use std::{
    marker::PhantomData,
    path::{Path, PathBuf},
};

use anyhow::anyhow;
use bevy::{
    asset::{AssetLoader, AssetPath, AsyncReadExt},
    platform::collections::{HashMap, HashSet},
    prelude::*,
};
use mlua::{chunk, prelude::*};

use super::network::ServerOptions;

#[macro_export]
macro_rules! from_lua_table {
    (struct $name:ident {
        $($item:ident: $({$tt:tt})? $item_ty:ty),* $(,)?
    }) => {
        impl mlua::FromLua for $name {
            fn from_lua(value: mlua::prelude::LuaValue, lua: &mlua::Lua) -> mlua::Result<Self> {
                let ty = value.type_name();
                let table = value.as_table().ok_or_else(|| {
                    mlua::prelude::LuaError::FromLuaConversionError {
                        from: ty,
                        to: stringify!($name).to_string(),
                        message: Some("expected table".to_string())
                    }
                })?;

                Ok($name {
                    $(
                        $item: from_lua_table!(lua table $item $([$tt])? ($item_ty))
                    ),*
                })
            }
        }
    };
    (struct W<$name:ident> {
        $($item:ident: $({$tt:tt})? $item_ty:ty),* $(,)?
    }) => {
        impl FromLua for W<$name> {
            fn from_lua(value: LuaValue, lua: &Lua) -> mlua::Result<Self> {
                let ty = value.type_name();
                let table = value.as_table().ok_or_else(|| {
                    LuaError::FromLuaConversionError {
                        from: ty,
                        to: stringify!($name).to_string(),
                        message: Some("expected table".to_string())
                    }
                })?;

                Ok(W($name {
                    $(
                        $item: from_lua_table!(lua table $item $([$tt])? ($item_ty))
                    ),*
                }))
            }
        }
    };
    ($lua:ident $table:ident $item:ident [W] ($item_ty:ty)) => {
        <crate::ingame::lua::W<$item_ty>>::from_lua($table.get(stringify!($item))?, $lua)?.0
    };
    ($lua:ident $table:ident $item:ident ($item_ty:ty)) => {
        <$item_ty>::from_lua($table.get(stringify!($item))?, $lua)?
    };
}

macro_rules! into_lua_table {
    (struct $name:ident {
        $($item:ident: $({$tt:tt})? $item_ty:ty),* $(,)?
    }) => {
        impl mlua::IntoLua for $name {
            fn into_lua(self, lua: &mlua::Lua) -> mlua::prelude::LuaResult<mlua::prelude::LuaValue> {
                let table = lua.create_table()?;
                $(
                    table.set(stringify!($item), into_lua_table!(lua self $item $([$tt])?))?;
                )*
                Ok(mlua::Value::Table(table))
            }
        }
    };
    (struct W<$name:ident> {
        $($item:ident: $({$tt:tt})? $item_ty:ty),* $(,)?
    }) => {
        impl mlua::IntoLua for W<$name> {
            fn into_lua(self, lua: &mlua::Lua) -> mlua::prelude::LuaResult<mlua::prelude::LuaValue> {
                let table = lua.create_table()?;
                $(
                    table.set(stringify!($item), into_lua_table!(lua self $item $([$tt])?))?;
                )*
                Ok(mlua::Value::Table(table))
            }
        }
    };
    ($lua:ident $self:ident $item:ident [W]) => {
        crate::ingame::lua::W($self.$item).into_lua($lua)?
    };
    ($lua:ident $self:ident $item:ident) => {
        $self.$item.into_lua($lua)?
    };
}

macro_rules! from_into_lua_table {
    (struct $name:ident {
        $($item:ident: $({$tt:tt})? $item_ty:ty),* $(,)?
    }) => {
        from_lua_table!(struct $name { $($item: $({$tt})? $item_ty),* });
        into_lua_table!(struct $name { $($item: $({$tt})? $item_ty),* });
    };
    (struct W<$name:ident> {
        $($item:ident: $({$tt:tt})? $item_ty:ty),* $(,)?
    }) => {
        from_lua_table!(struct W<$name> { $($item: $({$tt})? $item_ty),* });
        into_lua_table!(struct W<$name> { $($item: $({$tt})? $item_ty),* });
    };
}

macro_rules! proto {
    (struct $name:ident {
        $($item:ident: $({$tt:tt})? $item_ty:ty),* $(,)?
    }) => {
        from_into_lua_table!(struct $name { $($item: $({$tt})? $item_ty),* });

        impl crate::ingame::lua::Proto for $name {
            fn id(&self) -> &str {
                &self.id
            }
        }
    };
    (struct W<$name:ident> {
        $($item:ident: $({$tt:tt})? $item_ty:ty),* $(,)?
    }) => {
        from_into_lua_table!(struct W<$name> { $($item: $({$tt})? $item_ty),* });

        impl crate::ingame::lua::Proto for $name {
            fn id(&self) -> &str {
                &self.id
            }
        }
    };
}

#[derive(Resource, Deref)]
pub struct LuaCtx(pub Lua);

/// Contains data needed for the lua script running system to work.
///
/// Only supports running scripts once; trying to run one multiple times
/// may not work correctly. The scripts themselves are ment to be "registration scripts",
/// which registers functions to be ran on engine events.
#[derive(Default, Resource)]
pub struct LuaScriptRunningContext {
    /// Scripts that are yet to be loaded
    waiting_for_load: HashSet<Handle<LuaScript>>,
    /// All scripts that have been loaded and executed.
    loaded_and_executed: HashSet<Handle<LuaScript>>,
    /// Scripts that are waiting for another script to complete.
    waiting_for_others: Vec<ThreadInfo>,
    /// Scripts that are ready to be ran.
    currently_running: Vec<ThreadInfo>,
}

struct ThreadInfo {
    handle: Handle<LuaScript>,
    thread: LuaThread,
    /// A list of the scripts that need to complete before this script can run.
    ///
    /// Needed so that scripts may assume that other scripts have already registered their stuff.
    dependencies: Vec<Handle<LuaScript>>,
}

#[derive(Event)]
pub struct ScriptCompleted(pub Handle<LuaScript>);

pub fn execute_lua(world: &mut World) {
    let lua = world.resource::<LuaCtx>().0.clone();

    let mut ctx = world.resource_mut::<LuaScriptRunningContext>();
    // world.resource_scope(|world, mut ctx: Mut<'_, LuaScriptRunningContext>| {
    let mut i = 0;

    let mut newly_completed = vec![];

    // We should loop and run until we reach a stale state;
    // i.e. we have run out of currently running threads and none completed last loop
    loop {
        // First, check if any waiting threads can be started
        while i < ctx.waiting_for_others.len() {
            let thread_info = &ctx.waiting_for_others[i];
            if thread_info
                .dependencies
                .iter()
                .all(|h| ctx.loaded_and_executed.contains(h))
            {
                // Move to running queue
                let thread_info = ctx.waiting_for_others.remove(i);
                ctx.currently_running.push(thread_info);
            } else {
                i += 1;
            }
        }

        if ctx.currently_running.is_empty() {
            // No threads to run, and as such, no way for any dependencies to complete
            break;
        }

        // Loop through all running threads and resume them
        while !ctx.currently_running.is_empty() {
            i = 0;
            while i < ctx.currently_running.len() {
                let thread_info = &ctx.currently_running[i];

                let thread = thread_info.thread.clone();
                info!("Running thread {:?}", thread_info.handle.path());

                lua.set_path(thread_info.handle.path().unwrap().path().into());
                let resume_result = lua.with_world(world, |_| thread.resume::<LuaValue>(()));
                lua.reset_path();

                ctx = world.resource_mut::<LuaScriptRunningContext>();
                let thread_info = &ctx.currently_running[i];
                match resume_result {
                    Ok(value) => match thread_info.thread.status() {
                        LuaThreadStatus::Resumable => {
                            // The thread isn't finished yet, which means that it
                            // returned an object which tells us when to resume the thread.
                            match ResumeCondition::from_lua(value, &lua) {
                                Ok(cond) => match cond {
                                    ResumeCondition::AfterDependencyLoaded(handle) => {
                                        info!(" Thread reports dependency {:?}", handle.path());
                                        // Is this dependency already loaded?
                                        if ctx.loaded_and_executed.contains(&handle) {
                                            // We don't need to even add the dependency to the thread info; just continue
                                            // By not incrementing the index, this thread will be ran again
                                            info!(
                                                "  That dependency is already fulfilled; continuing"
                                            );
                                        } else {
                                            // We need to load and execute the dependency
                                            info!("  Waiting for dependency; going to next");
                                            let mut thread_info = ctx.currently_running.remove(i);
                                            thread_info.dependencies.push(handle);
                                            ctx.waiting_for_others.push(thread_info);
                                        }
                                    }
                                    ResumeCondition::Immediately => {
                                        info!(" Thread reports no additional dependencies");
                                    }
                                },
                                Err(_e) => todo!(),
                            }
                        }
                        LuaThreadStatus::Running => unreachable!(),
                        LuaThreadStatus::Finished => {
                            info!(" Thread finished");
                            let thread_info = ctx.currently_running.remove(i);
                            newly_completed.push(thread_info.handle.clone());
                            ctx.loaded_and_executed.insert(thread_info.handle);
                        }
                        LuaThreadStatus::Error => unreachable!(),
                    },
                    Err(e) => {
                        error!("Lua error: {e}");
                        ctx.currently_running.remove(i);
                    }
                }
            }
        }
    }

    for handle in newly_completed {
        world.trigger(ScriptCompleted(handle));
    }
    // });
}

#[derive(Debug, Clone, PartialEq, Eq, FromLua)]
enum ResumeCondition {
    AfterDependencyLoaded(Handle<LuaScript>),
    Immediately,
}

impl LuaUserData for ResumeCondition {}

struct IsServer(bool);

pub fn common(app: &mut App) {
    let lua = Lua::new();

    let is_server = IsServer(app.world().contains_resource::<ServerOptions>());
    lua.set_app_data(is_server);

    info!("I am server: {}", lua.is_server());

    app.insert_resource(LuaCtx(lua.clone()))
        .init_resource::<ScriptInfoMap>()
        .init_resource::<LuaScriptRunningContext>()
        .register_asset_loader(LuaScriptLoader { lua })
        .init_asset::<LuaScript>()
        .setup_lua(setup_lua)
        .add_systems(Update, (wait_for_script_load, execute_lua).chain());
}

#[derive(Resource, Default, Debug, Reflect)]
pub struct ScriptInfoMap {
    map: HashMap<Handle<LuaScript>, ScriptInfo>,
}

#[derive(Debug, Default, Reflect)]
pub struct ScriptInfo {
    executed: bool,
}

fn setup_lua(lua: &Lua) -> LuaResult<()> {
    let ensure_loaded_rust = lua.create_function(|lua: &Lua, path: PathBuf| {
        let path = lua.path(&path);
        // Check if path is loaded
        let mut world = lua.world();
        let handle =
            if let Some(handle) = world.resource::<AssetServer>().get_handle(path.as_path()) {
                handle
            } else {
                world.resource::<AssetServer>().load(path)
            };

        let ctx = world.resource::<LuaScriptRunningContext>();
        // Has this already been executed?
        if ctx.loaded_and_executed.contains(&handle) {
            // Then we don't need to yield, but it is easiest to it anyway
            return lua.create_userdata(ResumeCondition::Immediately);
        }
        // Is this script running, or is waiting to run?
        if ctx
            .currently_running
            .iter()
            .chain(&ctx.waiting_for_others)
            .any(|ti| ti.handle == handle)
        {
            // Then we need to wait for it to be completed
            return lua.create_userdata(ResumeCondition::AfterDependencyLoaded(handle));
        }
        // The script is not running, we need to start it
        // The ExecuteLuaScript command will handle if the script is loaded or not
        ExecuteLuaScript::new(handle.clone())
            // .immediately()
            .apply(&mut world);

        lua.create_userdata(ResumeCondition::AfterDependencyLoaded(handle))
    })?;

    let ensure_loaded = lua
        .load(chunk! {
            function(path)
                local cont = $ensure_loaded_rust (path)
                coroutine.yield(cont)
            end
        })
        .eval::<LuaFunction>()?;

    // let ensure_loaded = lua.create_function(|lua, path: PathBuf| {
    //     info!("Running ENSURE_LOADED");
    //     let path = lua.path(&path);
    //     let handle = lua.world().resource::<AssetServer>().load(path.as_ref());
    //     lua.world().commands().queue(ExecuteLuaScript(handle));
    //     lua.world().flush();
    //     Ok(())
    // })?;

    lua.table("game")?.set("ensure_loaded", ensure_loaded)?;
    Ok(())
}

fn wait_for_script_load(
    mut events: EventReader<AssetEvent<LuaScript>>,
    ctx: Res<LuaScriptRunningContext>,
    asset_server: Res<AssetServer>,
    mut commands: Commands,
) {
    for e in events.read() {
        match e {
            AssetEvent::LoadedWithDependencies { id } => {
                if let Some(handle) = asset_server.get_id_handle(*id)
                    && ctx.waiting_for_load.contains(&handle)
                {
                    // We are waiting for this script and should execute it
                    commands.queue(ExecuteLuaScript::new(handle.clone()).immediately());
                }
            }
            _ => {}
        }
    }
}

#[derive(Component)]
pub struct LuaObject(pub LuaTable);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Reflect, Deref, DerefMut)]
pub struct W<T>(pub T);

from_into_lua_table!(
    struct W<Vec2> {
        x: f32,
        y: f32,
    }
);

impl FromLua for W<AssetPath<'static>> {
    fn from_lua(value: LuaValue, lua: &Lua) -> LuaResult<Self> {
        let string = lua
            .coerce_string(value)?
            .ok_or_else(|| LuaError::external(anyhow!("Cannot convert nil to AssetPath")))?
            .to_string_lossy();
        Ok(W(AssetPath::from_static(string)))
    }
}

impl IntoLua for W<AssetPath<'_>> {
    fn into_lua(self, lua: &Lua) -> LuaResult<LuaValue> {
        self.to_string().into_lua(lua)
    }
}

pub trait AppLuaExt {
    fn setup_lua(&mut self, func: impl FnOnce(&Lua) -> LuaResult<()>) -> &mut Self;
}

impl AppLuaExt for App {
    fn setup_lua(&mut self, func: impl FnOnce(&Lua) -> LuaResult<()>) -> &mut Self {
        let lua = &self.world().resource::<LuaCtx>().0;

        match func(lua) {
            Ok(()) => {}
            Err(e) => {
                error!("Error setting up lua: {e}");
                self.world_mut()
                    .send_event(AppExit::Error(2.try_into().unwrap()));
            }
        }
        self
    }
}

pub trait LuaExt {
    fn world(&self) -> mlua::AppDataRefMut<'_, World>;
    fn path<'a>(&self, path: &'a Path) -> PathBuf;
    fn path_rel<'a>(&self, path: &'a Path, rel: PathBuf) -> PathBuf;
    fn current_path(&self) -> PathBuf;
    fn set_path(&self, path: PathBuf);
    fn reset_path(&self);
    fn with_world<T>(&self, world: &mut World, func: impl FnOnce(&Self) -> T) -> T;
    fn table(&self, key: &str) -> LuaResult<LuaTable>;
    fn is_server(&self) -> bool;
    fn is_client(&self) -> bool;
}

impl LuaExt for Lua {
    fn world(&self) -> mlua::AppDataRefMut<'_, World> {
        self.app_data_mut().unwrap()
    }

    fn path<'a>(&self, path: &'a Path) -> PathBuf {
        self.path_rel(
            path,
            self.app_data_ref::<CurrentScriptPath>().unwrap().0.clone(),
        )
    }

    fn path_rel<'a>(&self, path: &'a Path, rel: PathBuf) -> PathBuf {
        if true && (path.starts_with(".") || path.starts_with("..")) {
            rel.parent().unwrap().join(path).into()
        } else {
            path.into()
        }
    }

    fn current_path(&self) -> PathBuf {
        self.app_data_ref::<CurrentScriptPath>().unwrap().0.clone()
    }

    fn set_path(&self, path: PathBuf) {
        self.set_app_data(CurrentScriptPath(path));
    }

    fn reset_path(&self) {
        self.remove_app_data::<CurrentScriptPath>();
    }

    fn with_world<T>(&self, world: &mut World, func: impl FnOnce(&Self) -> T) -> T {
        self.set_app_data(std::mem::take(world));
        let x = func(self);
        *world = self.remove_app_data().unwrap();
        x
    }

    fn table(&self, key: &str) -> LuaResult<LuaTable> {
        match self.globals().get::<LuaTable>(key) {
            Ok(table) => Ok(table),
            Err(LuaError::FromLuaConversionError { from: "nil", .. }) => {
                let table = self.create_table()?;
                self.globals().set(key, table.clone())?;
                Ok(table)
            }
            Err(e) => Err(e),
        }
    }

    fn is_server(&self) -> bool {
        self.app_data_ref::<IsServer>().unwrap().0
    }

    fn is_client(&self) -> bool {
        !self.is_server()
    }
}

pub trait AssetPathExt {
    fn relative(self, origin: &Path) -> AssetPath<'static>;
}

impl AssetPathExt for AssetPath<'static> {
    fn relative(mut self, origin: &Path) -> AssetPath<'static> {
        let path = self.path();
        if path.starts_with(".") || path.starts_with("..") {
            let parent = origin.parent().unwrap();
            let joined = parent.join(path);
            let res = AssetPath::from_static(joined);
            if let Some(label) = self.take_label() {
                res.with_label(label)
            } else {
                res
            }
        } else {
            self
        }
    }
}

pub struct ExecuteLuaScript(pub Handle<LuaScript>, pub bool);

impl ExecuteLuaScript {
    pub fn new(handle: Handle<LuaScript>) -> Self {
        Self(handle, false)
    }

    pub fn immediately(mut self) -> Self {
        self.1 = true;
        self
    }
}

pub struct CurrentScriptPath(pub PathBuf);

impl Command for ExecuteLuaScript {
    fn apply(self, world: &mut World) -> () {
        if let Some(script) = world.resource::<Assets<LuaScript>>().get(&self.0) {
            // let script = script.clone();
            let lua = &world.resource::<LuaCtx>().0;

            // world
            //     .resource_mut::<ScriptInfoMap>()
            //     .map
            //     .entry(self.0.clone())
            //     .or_default()
            //     .executed = true;

            // let world_val = std::mem::take(world);
            // lua.set_app_data(world_val);
            // lua.set_app_data(CurrentScriptPath(script.path));
            // match script.function.call::<()>(()) {
            //     Ok(_) => {}
            //     Err(e) => error!("Lua error: {e}"),
            // }
            // *world = lua.remove_app_data().unwrap();
            let thread_info = ThreadInfo {
                handle: self.0.clone(),
                thread: match lua.create_thread(script.function.clone()) {
                    Ok(val) => val,
                    Err(e) => {
                        error!("Lua error: {e}");
                        return;
                    }
                },
                dependencies: vec![],
            };
            let mut ctx = world.resource_mut::<LuaScriptRunningContext>();
            ctx.currently_running.push(thread_info);

            if self.1 {
                // Run scripts NOW also
                execute_lua(world);
            }
        } else {
            // error!("Script not loaded!");
            let mut ctx = world.resource_mut::<LuaScriptRunningContext>();
            ctx.waiting_for_load.insert(self.0.clone());
        }
    }
}

#[derive(Clone, TypePath, Asset)]
pub struct LuaScript {
    function: LuaFunction,
    // path: PathBuf,
}

struct LuaScriptLoader {
    lua: Lua,
}

impl AssetLoader for LuaScriptLoader {
    type Asset = LuaScript;

    type Settings = ();

    type Error = anyhow::Error;

    fn load(
        &self,
        reader: &mut dyn bevy::asset::io::Reader,
        _settings: &Self::Settings,
        load_context: &mut bevy::asset::LoadContext,
    ) -> impl bevy::tasks::ConditionalSendFuture<Output = std::result::Result<Self::Asset, Self::Error>>
    {
        async move {
            let mut buf = String::new();
            reader.read_to_string(&mut buf).await?;
            let script = self
                .lua
                .load(buf)
                .set_name(
                    Path::new("assets")
                        .join(load_context.path())
                        // .canonicalize()
                        // .unwrap()
                        .to_string_lossy(),
                )
                .into_function()?;
            Ok(LuaScript {
                function: script,
                // path: load_context.path().into(),
            })
        }
    }

    fn extensions(&self) -> &[&str] {
        &["ron"]
    }
}

#[derive(Resource)]
pub struct Protos<T> {
    map: HashMap<String, (LuaValue, PathBuf)>,
    lua: Lua,
    _phantom: PhantomData<T>,
}

impl<T> FromWorld for Protos<T> {
    fn from_world(world: &mut World) -> Self {
        Self {
            map: default(),
            lua: world.resource::<LuaCtx>().0.clone(),
            _phantom: default(),
        }
    }
}

impl<T: Proto> Protos<T> {
    pub fn insert(&mut self, value: LuaValue, origin: PathBuf) -> LuaResult<LuaValue> {
        let proto = T::from_lua(value.clone(), &self.lua)?;
        self.map.insert(proto.id().into(), (value.clone(), origin));
        Ok(value)
    }

    pub fn get(&self, id: &str) -> LuaResult<(T, PathBuf)> {
        let (value, origin) = self
            .map
            .get(id)
            .ok_or_else(|| LuaError::external(anyhow!("Missing proto")))?;
        let proto = T::from_lua(value.clone(), &self.lua)?;
        Ok((proto, origin.clone()))
    }
}

pub trait Proto: FromLua {
    fn id(&self) -> &str;
}
