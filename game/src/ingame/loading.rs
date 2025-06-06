use std::path::PathBuf;

use bevy::{
    asset::{AssetLoader, AssetPath, LoadedUntypedAsset}, color::palettes, platform::collections::{HashMap, HashSet}, prelude::*
};
use engine_common::{ChampionDef, ChampionId, MapId};
use lightyear::prelude::*;

use crate::{
    ingame::{
        lua::{AppLuaExt, AssetPathExt, LuaCtx, LuaExt, LuaScript, ScriptCompleted, W},
        map::{LoadMap, MapDefAsset, MessageChannel},
    }, new_ui::{
        container::ContainerView, image::ImageView, list::ListView, tree::UiTree, View, ViewExt
    }, AppExt, GameState, InGamePlayerInfo, Players
};

pub fn plugin(app: &mut App) {
    app.register_resource::<WhatToLoad>(ChannelDirection::ServerToClient);
    app.register_resource::<ClientLoadStates>(ChannelDirection::ServerToClient);
    app.register_trigger::<UpdateClientLoadState>(ChannelDirection::ClientToServer);

    app.add_sub_state::<LoadingState>();

    app.init_asset::<DummyAsset>();
    app.register_asset_loader(DummyAssetLoader);

    if app.is_client() {
        // We need to know what to load, i.e. map and champs
        app.add_systems(
            Update,
            (|mut commands: Commands| {
                commands.set_state(LoadingState::LoadingDefs);
            })
            .run_if(resource_added::<WhatToLoad>),
        );
        app.add_systems(OnEnter(GameState::Loading), spawn_loading_screen);
        app.add_systems(
            Update,
            update_load_status.run_if(
                in_state(GameState::Loading).and(resource_exists_and_changed::<UntypedAssetHolder>),
            ),
        );
    } else {
        // We already know what to load, as we were given that information from
        // the lobby server
        app.add_systems(
            OnEnter(LoadingState::WaitingForWhatToLoad),
            |mut commands: Commands| {
                commands.set_state(LoadingState::LoadingDefs);
            },
        );
        app.insert_resource(WhatToLoad {
            // Hard code default map currently
            map: MapId("default".into()),
            champs: app
                .world()
                .resource::<Players>()
                .players
                .values()
                .map(|p| p.champion.clone())
                .collect(),
        });
        app.insert_resource(ClientLoadStates {
            load_states: app
                .world()
                .resource::<Players>()
                .players
                .values()
                .map(|p| (p.client_id, ClientLoadState::Loading(0.0)))
                .collect(),
        });

        app.add_observer(on_update_client_load_state);
    }

    app.add_systems(
        OnEnter(LoadingState::LoadingDefs),
        start_loading_what_to_load,
    );
    app.add_systems(
        Update,
        wait_until_loaded.run_if(in_state(LoadingState::LoadingDefs)),
    );
    app.add_systems(
        Update,
        execute_lua.run_if(in_state(LoadingState::RunningScripts)),
    );
    app.add_systems(
        Update,
        wait_for_assets.run_if(in_state(LoadingState::LoadingAssets)),
    );
    app.add_systems(
        Update,
        wait_until_all_clients_loaded.run_if(in_state(LoadingState::WaitingForOthers)),
    );
    app.setup_lua(setup_lua);

    app.add_systems(OnEnter(LoadingState::LoadCompleted), start_map_load);
}

#[derive(Default, Debug, Clone, Copy, PartialEq, Eq, Hash, SubStates)]
#[source(GameState = GameState::Loading)]
pub enum LoadingState {
    #[default]
    WaitingForWhatToLoad,
    LoadingDefs,
    RunningScripts,
    LoadingAssets,
    WaitingForOthers,
    LoadCompleted,
}

#[derive(Debug, Resource, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WhatToLoad {
    map: MapId,
    champs: Vec<ChampionId>,
}

#[derive(Resource)]
pub struct LoadHolder {
    map: Handle<MapDefAsset>,
    champs: Vec<Handle<ChampionDef>>,
}

#[derive(Default, Resource)]
pub struct UntypedAssetHolder {
    total_asset_count: usize,
    waiting_assets: Vec<Handle<LoadedUntypedAsset>>,
    done_assets: Vec<UntypedHandle>,
}

impl UntypedAssetHolder {
    fn add_asset(&mut self, handle: Handle<LoadedUntypedAsset>) {
        self.total_asset_count += 1;
        self.waiting_assets.push(handle)
    }
}

fn start_loading_what_to_load(
    what_to_load: Res<WhatToLoad>,
    asset_server: Res<AssetServer>,
    mut commands: Commands,
) {
    commands.replicate_resource::<WhatToLoad, MessageChannel>(NetworkTarget::All);
    commands.replicate_resource::<ClientLoadStates, MessageChannel>(NetworkTarget::All);

    info!(
        "We are starting to load the following deps: {:#?}",
        *what_to_load
    );
    let map = asset_server.load::<MapDefAsset>(format!("maps/{}/def.ron", what_to_load.map.0));
    let mut champs = vec![];
    for champ in &what_to_load.champs {
        champs.push(asset_server.load::<ChampionDef>(format!("champs/{}/def.ron", champ.0)));
    }
    let load_holder = LoadHolder { map, champs };

    commands.insert_resource(load_holder);
}

fn wait_until_loaded(
    holder: Res<LoadHolder>,
    asset_server: Res<AssetServer>,
    map_def_assets: Res<Assets<MapDefAsset>>,
    mut commands: Commands,
) {
    if asset_server.is_loaded_with_dependencies(&holder.map)
        && holder
            .champs
            .iter()
            .all(|h| asset_server.is_loaded_with_dependencies(h))
    {
        info!("We have loaded all defs!");
        // We have loaded the defs and their scripts, but the scripts themselves have dependencies
        // that are only registered upon running them.
        // As such, we need to run the scripts.
        // Note that they should be made in such a way that running them only registeres dependencies
        // and registering protos. They should not start running game logic.

        info!("We start running the following scripts:");

        let map_def = map_def_assets.get(&holder.map).unwrap();
        info!(" {}", map_def.script.path().unwrap());

        commands.insert_resource(LuaRegistrationScriptRunningContext {
            waiting_for_load: [map_def.script.clone()].into_iter().collect(),
            ..default()
        });

        commands.insert_resource(UntypedAssetHolder::default());
        commands.set_state(LoadingState::RunningScripts);
    }
}

use mlua::prelude::*;

/// Contains data needed for the lua script running system to work.
///
/// Only supports running scripts once; trying to run one multiple times
/// may not work correctly. The scripts themselves are ment to be "registration scripts",
/// which registers functions to be ran on engine events.
#[derive(Default, Resource)]
pub struct LuaRegistrationScriptRunningContext {
    /// Scripts that are yet to be loaded.
    /// When they are, they are removed from this set.
    waiting_for_load: HashSet<Handle<LuaScript>>,
    /// Scripts that have been started, but not yet completed
    /// When they are, they are removed from this set.
    started: HashSet<Handle<LuaScript>>,
    /// All scripts that have been loaded and executed.
    loaded_and_executed: HashSet<Handle<LuaScript>>,
    /// Scripts that are waiting for another script to complete.
    waiting_for_others: Vec<ThreadInfo>,
    /// Scripts that are ready to be ran.
    currently_running: Vec<ThreadInfo>,
}

impl LuaRegistrationScriptRunningContext {
    fn is_complete(&self) -> bool {
        self.waiting_for_load.is_empty() && self.started.is_empty()
    }
}

struct ThreadInfo {
    handle: Handle<LuaScript>,
    thread: LuaThread,
    /// A list of the scripts that need to complete before this script can run.
    ///
    /// Needed so that scripts may assume that other scripts have already registered their stuff.
    dependencies: Vec<Handle<LuaScript>>,
}

pub fn execute_lua(world: &mut World) {
    let lua = world.resource::<LuaCtx>().0.clone();

    let asset_server = world.resource::<AssetServer>().clone();

    // Keep track of the scripts that complete this frame, to be able to trigger ScriptCompleted
    // for them.
    // This would be a bit more ergonomic if we had Assets as Entities, as the observers
    // could be listening on the asset entities themselves.
    let mut newly_completed = vec![];

    world.resource_scope(|world, mut ctx: Mut<LuaRegistrationScriptRunningContext>| {
        // We should loop and run until we reach a stale state;
        // i.e. we have run out of currently running threads and none completed last loop
        loop {
            let script_assets = world.resource::<Assets<LuaScript>>();

            // Check if any scripts that are waiting to be loaded have been
            let mut to_remove = vec![];
            // We get a non-change-detecting reference so that we can partially
            // borrow the context (.waiting_for_load and .currently_running)
            let ctx_ref = ctx.bypass_change_detection();
            let mut changed = false;
            for script in &ctx_ref.waiting_for_load {
                if asset_server.is_loaded_with_dependencies(script) {
                    info!(
                        "{} is loaded! Moving to running list",
                        script.path().unwrap()
                    );

                    // Add the script to the running list and remove it from the waiting list
                    // The removal needs to be deferred due to it being borrowed
                    let lua_script = script_assets.get(script).unwrap();
                    let thread_info = ThreadInfo {
                        handle: script.clone(),
                        thread: lua.create_thread(lua_script.function.clone()).unwrap(),
                        dependencies: vec![],
                    };
                    ctx_ref.currently_running.push(thread_info);
                    ctx_ref.started.insert(script.clone());
                    to_remove.push(script.clone());
                    changed = true;
                }
            }

            // We don't actually check if the context has changed anywhere,
            // but just to be safe
            if changed {
                ctx.set_changed();
            }

            for script in to_remove {
                ctx.waiting_for_load.remove(&script);
            }

            // Check if any waiting threads can be started
            {
                let mut i = 0;
                while i < ctx.waiting_for_others.len() {
                    let thread_info = &ctx.waiting_for_others[i];
                    if thread_info
                        .dependencies
                        .iter()
                        .all(|h| ctx.loaded_and_executed.contains(h))
                    {
                        info!(
                            "{} can now run! Moving to running list",
                            ctx.waiting_for_others[i].handle.path().unwrap()
                        );
                        // Move to running queue
                        let thread_info = ctx.waiting_for_others.remove(i);
                        ctx.currently_running.push(thread_info);
                    } else {
                        i += 1;
                    }
                }
            }

            if ctx.currently_running.is_empty() {
                // No threads to run, and as such, no way for any dependencies to complete
                break;
            }

            // Loop through all running threads and resume them
            while !ctx.currently_running.is_empty() {
                // We always try to run the first script in the list
                // When the script needs to wait for some reason (or completes),
                // it is removed from the currently running list, allowing other scripts to run
                while let Some(thread_info) = ctx.currently_running.first() {
                    let thread = thread_info.thread.clone();
                    info!("Running thread {:?}", thread_info.handle.path());

                    // We give the path of the script to run to lua, so that we
                    // can resolve relative asset paths.
                    lua.set_path(thread_info.handle.path().unwrap().path().into());
                    let resume_result = lua.with_world(world, |_| thread.resume::<LuaValue>(()));
                    lua.reset_path();

                    match resume_result {
                        Ok(value) => match thread_info.thread.status() {
                            LuaThreadStatus::Resumable => {
                                // The thread isn't finished yet, which means that it
                                // returned an object which tells us when to resume the thread.
                                handle_resume_condition(&lua, world, &mut ctx, value);
                            }
                            LuaThreadStatus::Running => unreachable!(),
                            LuaThreadStatus::Finished => {
                                info!(" Thread finished");
                                let thread_info = ctx.currently_running.remove(0);
                                newly_completed.push(thread_info.handle.clone());
                                ctx.started.remove(&thread_info.handle);
                                ctx.loaded_and_executed.insert(thread_info.handle);
                            }
                            LuaThreadStatus::Error => unreachable!(),
                        },
                        Err(e) => {
                            error!("Lua error: {e}");
                            let handle = thread_info.handle.clone();
                            ctx.currently_running.remove(0);
                            ctx.started.remove(&handle);
                        }
                    }
                }
            }
        }

        // Are we done?
        if ctx.is_complete() {
            info!("Script running completed!");
            world.commands().set_state(LoadingState::LoadingAssets);
        }
    });

    for handle in newly_completed {
        world.trigger(ScriptCompleted(handle));
    }
}

fn handle_resume_condition(
    lua: &Lua,
    world: &mut World,
    ctx: &mut Mut<'_, LuaRegistrationScriptRunningContext>,
    value: LuaValue,
) {
    match ResumeCondition::from_lua(value, lua) {
        Ok(cond) => match cond {
            ResumeCondition::AfterDependencyLoaded(handle) => {
                info!(" Thread reports dependency {:?}", handle.path());
                // The dependency can be in the following states:
                // - Not yet loaded
                // - Loaded, but not yet started
                // - Started, but not yet completed
                // - Completed

                // The first two cases are treated the same;
                // the first part of execute_lua will simply immediately notice that
                // the asset is loaded.

                if ctx.loaded_and_executed.contains(&handle) {
                    // We don't need to even add the dependency to the thread info; just continue
                    // By not incrementing the index, this thread will be ran again
                    info!("  That dependency is already fulfilled; continuing");
                } else {
                    if !ctx.started.contains(&handle) {
                        // We haven't started this script yet, so we add it to the list
                        // of scripts we are waiting for to load
                        // If it already is loaded, it will be noticed immediately
                        ctx.waiting_for_load.insert(handle.clone());
                    }

                    // We add the dependency to this script's dependency list
                    // and move it to the waiting list
                    info!("  Waiting for dependency; going to next");
                    let mut thread_info = ctx.currently_running.remove(0);
                    thread_info.dependencies.push(handle);
                    ctx.waiting_for_others.push(thread_info);
                }
            }
            ResumeCondition::RegisterDependency(handle) => {
                info!(" Thread registers asset {}", handle.path().unwrap());
                // This isn't really a "resume condition"
                // We add this handle to the UntypedAssetHolder, so that we can wait on it later
                world.resource_mut::<UntypedAssetHolder>().add_asset(handle);
                // We resume the thread immediately
            }
            ResumeCondition::Immediately => {
                // Resume immediately
            }
        },
        Err(_e) => {
            // The object we received wasn't a ResumeCondition;
            // this may be caused by a script running coroutine.yield()
            // with another value
            // We log this as a warning, but simply rerun the thread
            warn!(" Lua script yielded non-ResumeCondition value");
        }
    }
}

fn setup_lua(lua: &Lua) -> LuaResult<()> {
    // This loads and executes a script.
    let ensure_loaded_rust = lua.create_function(|lua: &Lua, path: PathBuf| {
        let path = lua.path(&path);
        // Check if path is loaded
        let world = lua.world();
        let handle =
            if let Some(handle) = world.resource::<AssetServer>().get_handle(path.as_path()) {
                handle
            } else {
                world.resource::<AssetServer>().load(path)
            };

        // We yield the AfterDependencyLoaded for the script; it is execute_lua's responsibility
        // to check if the dependency is loaded and executed, and if not, do that
        lua.create_userdata(ResumeCondition::AfterDependencyLoaded(handle))
    })?;

    // This registers an asset dependency.
    let register_asset = lua.create_function(|lua: &Lua, path: W<AssetPath<'static>>| {
        if lua.is_server() {
            // Graphical assets shouldn't be loaded on the server.
            // Right now, those are .glbs and .pngs.
            if let Some(ext) = path.0.path().extension()
                && ["glb", "png"].contains(&ext.to_str().unwrap_or(""))
            {
                info!("Skipping registering graphical asset {} on server", path.0);
                return lua.create_userdata(ResumeCondition::Immediately);
            }
        }

        let origin = lua.current_path();
        let path = path.0.relative(&origin);
        let world = lua.world();
        let handle = world.resource::<AssetServer>().load_untyped(path);

        // We yield the AfterDependencyLoaded for the script; it is execute_lua's responsibility
        // to check if the dependency is loaded and executed, and if not, do that
        lua.create_userdata(ResumeCondition::RegisterDependency(handle))
    })?;

    // Lua thunk to call coroutine.yield()
    let coroutine_yield = |func: LuaFunction| {
        lua.load(mlua::chunk! {
            function(path)
                local cont = $func (path)
                coroutine.yield(cont)
            end
        })
        .eval::<LuaFunction>()
    };

    lua.table("game")?
        .set("ensure_loaded", coroutine_yield(ensure_loaded_rust)?)?;
    lua.table("game")?
        .set("register_asset", coroutine_yield(register_asset)?)?;

    Ok(())
}

#[derive(Debug, Clone, PartialEq, Eq, FromLua)]
enum ResumeCondition {
    AfterDependencyLoaded(Handle<LuaScript>),
    RegisterDependency(Handle<LoadedUntypedAsset>),
    Immediately,
}

impl LuaUserData for ResumeCondition {}

fn wait_for_assets(
    mut holder: ResMut<UntypedAssetHolder>,
    untyped_assets: Res<Assets<LoadedUntypedAsset>>,
    asset_server: Res<AssetServer>,
    mut commands: Commands,
) {
    let mut i = 0;
    while i < holder.waiting_assets.len() {
        if asset_server.is_loaded_with_dependencies(&holder.waiting_assets[i]) {
            info!("Asset {} loaded!", holder.waiting_assets[i].path().unwrap());
            let handle = holder.waiting_assets.remove(i);
            let asset = untyped_assets.get(&handle).unwrap();
            holder.done_assets.push(asset.handle.clone());
        } else {
            i += 1;
        }
    }

    if holder.waiting_assets.is_empty() {
        info!("All assets loaded!");
        commands.set_state(LoadingState::WaitingForOthers);
    }
}

#[derive(Clone, Resource, PartialEq, Serialize, Deserialize)]
pub struct ClientLoadStates {
    load_states: HashMap<ClientId, ClientLoadState>,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub enum ClientLoadState {
    Loading(f32),
    Done,
}

#[derive(Debug, Event, Clone, PartialEq, Serialize, Deserialize)]
pub struct UpdateClientLoadState {
    state: ClientLoadState,
}

fn on_update_client_load_state(
    trigger: Trigger<FromClients<UpdateClientLoadState>>,
    mut states: ResMut<ClientLoadStates>,
) {
    states
        .load_states
        .insert(trigger.from(), trigger.message().state);
}

fn wait_until_all_clients_loaded(
    client_load_states: Res<ClientLoadStates>,
    mut commands: Commands,
) {
    if client_load_states
        .load_states
        .values()
        .all(|ls| *ls == ClientLoadState::Done)
    {
        info!("All clients loaded!");
        commands.set_state(LoadingState::LoadCompleted);
    }
}

fn start_map_load(what_to_load: Res<WhatToLoad>, mut commands: Commands) {
    commands.queue(LoadMap(what_to_load.map.clone()));
    commands.set_state(GameState::InGame);
}

#[derive(Component)]
struct LoadingScreenRoot;

fn spawn_loading_screen(mut commands: Commands) {
    commands.spawn((
        StateScoped(GameState::Loading),
        Node {
            width: Val::Percent(100.0),
            height: Val::Percent(100.0),
            flex_direction: FlexDirection::Column,
            ..default()
        },
        UiTree::new(loading_screen),
    ));
}

fn loading_screen(
    states: Option<Res<ClientLoadStates>>,
    players: Option<Res<Players>>,
) -> Option<impl View + use<>> {
    let (Some(states), Some(players)) = (states, players) else {
        return Some(
            ListView::new()
                .styled()
                .flex_direction(FlexDirection::Column)
                .flex_grow(1.0)
                .background_color(Color::BLACK),
        );
    };

    if !states.is_changed() {
        return None;
    }

    let mut list = ListView::new();

    let teams = players
        .players
        .values()
        .map(|pi| pi.team)
        .collect::<HashSet<_>>();

    let mut teams = Vec::from_iter(teams);
    teams.sort();

    for &team in &teams {
        let mut team_list = ListView::new();
        let players = players
            .players
            .values()
            .filter(|pi| pi.team == team)
            .collect::<Vec<_>>();
        for player in &players {
            let state = states.load_states.get(&player.client_id).unwrap();
            let player_thing = player_loading(&player, state);
            team_list.add(
                player_thing
                    .styled()
                    .padding(UiRect::all(Val::Px(5.0)))
                    .max_width(Val::Percent(100.0 / players.len() as f32))
                    .height(Val::Percent(100.0))
                    .flex_direction(FlexDirection::Column),
            );
        }
        list.add(
            team_list
                .styled()
                .flex_direction(FlexDirection::Row)
                .max_width(Val::Percent(100.0))
                .max_height(Val::Percent(100.0 / teams.len() as f32)),
        );
    }

    Some(
        list.styled()
            .flex_direction(FlexDirection::Column)
            .height(Val::Percent(100.0))
            .background_color(Color::BLACK)
            .align_items(AlignItems::Center)
            .justify_content(JustifyContent::Center),
    )
}

fn player_loading(
    player: &InGamePlayerInfo,
    state: &ClientLoadState,
) -> ListView {
    let player_name = player.name.clone();
    let mut player_thing = ListView::new();
    // TODO: show actual champ image
    player_thing.add(ImageView::new("champs/unknown.png"));
    player_thing.add(player_name);
    player_thing.add(
        ContainerView::new(
            ().styled()
                .height(Val::Px(5.0))
                .background_color(Color::WHITE)
                .width(Val::Percent(match state {
                    ClientLoadState::Loading(progress) => progress * 100.0,
                    ClientLoadState::Done => 100.0,
                })),
        )
        .styled()
        .width(Val::Percent(100.0))
        .background_color(Color::Srgba(palettes::tailwind::GRAY_700))
    );
    player_thing
}

fn update_load_status(holder: Res<UntypedAssetHolder>, mut commands: Commands) {
    let is_done = holder.done_assets.len() == holder.total_asset_count;
    let state = if is_done {
        ClientLoadState::Done
    } else {
        let progress = holder.done_assets.len() as f32 / holder.total_asset_count as f32;
        ClientLoadState::Loading(progress)
    };
    commands.client_trigger::<MessageChannel>(UpdateClientLoadState { state });
}

#[derive(Reflect, Asset)]
struct DummyAsset;

struct DummyAssetLoader;

impl AssetLoader for DummyAssetLoader {
    type Asset = DummyAsset;

    type Settings = ();

    type Error = !;

    fn load(
        &self,
        _reader: &mut dyn bevy::asset::io::Reader,
        _settings: &Self::Settings,
        _load_context: &mut bevy::asset::LoadContext,
    ) -> impl bevy::tasks::ConditionalSendFuture<Output = std::result::Result<Self::Asset, Self::Error>>
    {
        async move {
            let x: u8 = std::random::random();
            let _secs = (x as f32 / 255.0) * 10.0;
            // tokio::time::sleep(Duration::from_secs_f32(secs)).await;
            // smol::unblock(move || std::thread::sleep(Duration::from_secs_f32(secs))).await;
            Ok(DummyAsset)
        }
    }

    fn extensions(&self) -> &[&str] {
        &["dummy"]
    }
}
