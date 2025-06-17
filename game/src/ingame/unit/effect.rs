use std::collections::VecDeque;

use bevy::{
    asset::uuid::Uuid,
    ecs::entity::MapEntities,
    platform::collections::{HashMap, HashSet},
    prelude::*,
};
use derive_more::From;
use egui_dock::egui::UserData;
use lightyear::{channel::builder::EntityActionsChannel, prelude::*};
use mlua::prelude::*;
use serde::{Deserialize, Serialize};

use crate::{
    AppExt, GameState,
    ingame::{
        lua::{AppLuaExt, LuaCtx, LuaExt, Protos},
        map::MessageChannel,
        unit::{UnitId, UnitMap, UnitProxy},
    },
};

pub fn plugin(app: &mut App) {
    app.init_resource::<Protos<EffectProto>>();

    app.add_systems(
        FixedUpdate,
        update_effects.run_if(in_state(GameState::InGame)),
    );

    app.register_trigger::<ApplyEffect>(ChannelDirection::ServerToClient);
    app.register_trigger::<UpdateEffectCustomData>(ChannelDirection::ServerToClient);
    app.register_trigger::<RemoveEffect>(ChannelDirection::ServerToClient);

    app.register_component::<CustomData>(ChannelDirection::ServerToClient);

    if app.is_client() {
        app.add_observer(on_effect_applied);
        app.add_observer(on_effect_custom_data_uppdated);
        app.add_observer(on_effect_removed);

        app.init_resource::<DeferredUnitCommands>();

        app.add_systems(
            Update,
            apply_deferred_unit_command.run_if(in_state(GameState::InGame)),
        );
    }

    app.setup_lua(setup_lua);
}

fn setup_lua(lua: &Lua) -> LuaResult<()> {
    let game = lua.table("game")?;

    game.set(
        "register_effect",
        lua.create_function(|lua, proto: LuaValue| {
            lua.world()
                .resource_mut::<Protos<EffectProto>>()
                .insert(proto, lua.current_path())
        })?,
    )?;

    Ok(())
}

// We don't replicate this component, as we don't want to send the entire
// struct everytime an effect updates
// We instead send individual messages for specific changes
// This increases the risk of synchronization bugs, so we need to be careful
#[derive(Component, Default, Debug)]
pub struct EffectList {
    effect_list: HashMap<EffectId, Effect>,
    updatable: HashSet<EffectId>,
}

impl EffectList {
    pub fn add_effect(&mut self, proto: &EffectProto, effect: Effect) {
        let id = effect.id;
        if proto.on_update.is_some() {
            self.updatable.insert(id);
        }

        self.effect_list.insert(id, effect);
    }

    pub fn remove_effect(&mut self, effect_id: EffectId) {
        self.effect_list.remove(&effect_id);
        self.updatable.remove(&effect_id);
    }
}

#[derive(
    Default, Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Deref, DerefMut,
)]
pub struct EffectId(pub Uuid);

impl EffectId {
    pub fn new() -> Self {
        Self(Uuid::new_v4())
    }
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Effect {
    proto: String,
    id: EffectId,
    time_till_update: f32,
    data: CustomData,
}

#[derive(PartialEq)]
pub struct EffectProto {
    id: String,
    update_rate: f32,
    on_applied: Option<LuaFunction>,
    on_removed: Option<LuaFunction>,
    on_update: Option<LuaFunction>,
}

proto!(
    struct EffectProto {
        id: String,
        update_rate: f32,
        on_applied: Option<LuaFunction>,
        on_removed: Option<LuaFunction>,
        on_update: Option<LuaFunction>,
    }
);

pub fn apply_effect(lua: &Lua, proxy: &UnitProxy, args: ApplyEffectArgs) -> LuaResult<()> {
    let mut world = lua.world();

    let effect = Effect {
        proto: args.proto,
        id: EffectId::new(),
        time_till_update: 0.0,
        data: args.data,
    };

    ApplyEffectCommand {
        entity: proxy.entity,
        effect,
    }
    .apply(&mut world);

    Ok(())
}

pub struct ApplyEffectArgs {
    proto: String,
    data: CustomData,
}

from_into_lua_table!(
    struct ApplyEffectArgs {
        proto: String,
        data: CustomData,
    }
);

fn update_effects(
    q: Query<(Entity, &mut EffectList)>,
    effect_protos: Res<Protos<EffectProto>>,
    time: Res<Time>,
    mut commands: Commands,
) {
    for (e, effect_list) in q {
        let effect_list = effect_list.into_inner();
        for &id in &effect_list.updatable {
            let effect = effect_list.effect_list.get_mut(&id).unwrap();

            let Ok((proto, _path)) = effect_protos.get(&effect.proto) else {
                error!("Unregistered effect proto {}", effect.proto);
                continue;
            };

            effect.time_till_update -= time.delta_secs();
            if effect.time_till_update <= 0.0 {
                if let Some(update) = proto.on_update {
                    let unit = UnitProxy { entity: e };
                    let effect = EffectProxy {
                        entity: e,
                        effect_id: effect.id,
                    };
                    commands.queue(move |world: &mut World| {
                        let lua = world.resource::<LuaCtx>().0.clone();
                        lua.with_world(world, |_| match update.call::<()>((unit, effect)) {
                            Ok(()) => {}
                            Err(e) => {
                                error!("Lua error during effect {} update: {}", &proto.id, e);
                            }
                        });
                    });
                }
                effect.time_till_update += proto.update_rate;
            }
        }
    }
}

trait UnitCommandTrait {
    fn id(&self) -> UnitId;
}

#[derive(Event, Clone, Serialize, Deserialize)]
pub struct ApplyEffect {
    pub unit_id: UnitId,
    pub effect: Effect,
}

impl UnitCommandTrait for ApplyEffect {
    fn id(&self) -> UnitId {
        self.unit_id
    }
}

#[derive(Event, Clone, Serialize, Deserialize)]
pub struct UpdateEffectCustomData {
    pub unit_id: UnitId,
    pub effect_id: EffectId,
    pub custom_data: CustomData,
}

impl UnitCommandTrait for UpdateEffectCustomData {
    fn id(&self) -> UnitId {
        self.unit_id
    }
}

#[derive(Event, Clone, Serialize, Deserialize)]
pub struct RemoveEffect {
    pub unit_id: UnitId,
    pub effect_id: EffectId,
}

impl UnitCommandTrait for RemoveEffect {
    fn id(&self) -> UnitId {
        self.unit_id
    }
}

pub struct ApplyEffectCommand {
    pub entity: Entity,
    pub effect: Effect,
}

impl Command for ApplyEffectCommand {
    fn apply(self, world: &mut World) -> () {
        let (proto, _) = world
            .resource::<Protos<EffectProto>>()
            .get(&self.effect.proto)
            .unwrap();
        let mut entity = world.entity_mut(self.entity);
        entity
            .entry::<EffectList>()
            .or_default()
            .get_mut()
            .add_effect(&proto, self.effect.clone());
        let unit_id = *entity.get::<UnitId>().unwrap();
        world.server_trigger::<MessageChannel>(
            ApplyEffect {
                unit_id,
                effect: self.effect,
            },
            NetworkTarget::All,
        );
    }
}

pub struct EffectProxy {
    entity: Entity,
    effect_id: EffectId,
}

impl LuaUserData for EffectProxy {
    fn add_fields<F: LuaUserDataFields<Self>>(fields: &mut F) {}

    fn add_methods<M: LuaUserDataMethods<Self>>(methods: &mut M) {
        methods.add_method("get_custom_data", |lua, s, ()| {
            let world = lua.world();

            let custom_data = world
                .entity(s.entity)
                .get::<EffectList>()
                .unwrap()
                .effect_list
                .get(&s.effect_id)
                .unwrap()
                .data
                .clone()
                .into_lua(lua);

            Ok(custom_data)
        });

        methods.add_method("set_custom_data", |lua, s, data: CustomData| {
            let mut world = lua.world();

            world
                .entity_mut(s.entity)
                .get_mut::<EffectList>()
                .unwrap()
                .effect_list
                .get_mut(&s.effect_id)
                .unwrap()
                .data = data;

            Ok(())
        });
    }
}

// --- CLIENT ---

#[derive(Resource, Default)]
struct DeferredUnitCommands {
    queue: Vec<UnitCommand>,
}

#[derive(From)]
enum UnitCommandKind {
    ApplyEffect(ApplyEffect),
    UpdateEffectCustomData(UpdateEffectCustomData),
    RemoveEffect(RemoveEffect),
}

struct UnitCommand {
    unit_id: UnitId,
    kind: UnitCommandKind,
}

impl<T: UnitCommandTrait + Into<UnitCommandKind>> From<T> for UnitCommand {
    fn from(value: T) -> Self {
        Self {
            unit_id: value.id(),
            kind: value.into(),
        }
    }
}

fn apply_deferred_unit_command(
    mut queue: ResMut<DeferredUnitCommands>,
    protos: Res<Protos<EffectProto>>,
    mut q: Query<&mut EffectList>,
    unit_map: Res<UnitMap>,
) {
    let mut to_remove = vec![];

    for (i, command) in queue.queue.iter_mut().enumerate() {
        if let Some(&e) = unit_map.map.get(&command.unit_id) {
            let x: Result<(), BevyError> = try {
                match &mut command.kind {
                    UnitCommandKind::ApplyEffect(apply_effect) => {
                        let effect = std::mem::take(&mut apply_effect.effect);
                        let (proto, _) = protos.get(&effect.proto).unwrap();
                        q.get_mut(e)?.add_effect(&proto, effect);
                    }
                    UnitCommandKind::UpdateEffectCustomData(update_effect_custom_data) => {
                        let effect_id = update_effect_custom_data.effect_id;
                        let data = std::mem::take(&mut update_effect_custom_data.custom_data);
                        q.get_mut(e)
                            .unwrap()
                            .effect_list
                            .get_mut(&effect_id)
                            .unwrap()
                            .data = data;
                    }
                    UnitCommandKind::RemoveEffect(remove_effect) => {
                        let effect_id = remove_effect.effect_id;
                        q.get_mut(e).unwrap().remove_effect(effect_id);
                    }
                }
            };

            if let Err(e) = x {
                error!("Error while executing unit command: {e}");
            }

            to_remove.push(i);
        }
    }

    // Iterate in reverse to avoid items being removed messing up the indices
    // This works as the indices in to_remove is strictly increasing
    for to_remove in to_remove.into_iter().rev() {
        queue.queue.swap_remove(to_remove);
    }
}

fn on_effect_applied(
    trigger: Trigger<FromServer<ApplyEffect>>,
    protos: Res<Protos<EffectProto>>,
    mut q: Query<&mut EffectList>,
    unit_map: Res<UnitMap>,
    mut queue: ResMut<DeferredUnitCommands>,
) {
    if let Some(&e) = unit_map.map.get(&trigger.message.unit_id) {
        let effect = trigger.message.effect.clone();
        q.get_mut(e)
            .unwrap()
            .add_effect(&protos.get(&effect.proto).unwrap().0, effect);
    } else {
        queue.queue.push(trigger.message.clone().into());
    }
}

fn on_effect_custom_data_uppdated(
    trigger: Trigger<FromServer<UpdateEffectCustomData>>,
    mut q: Query<&mut EffectList>,
    unit_map: Res<UnitMap>,
    mut queue: ResMut<DeferredUnitCommands>,
) {
    if let Some(&e) = unit_map.map.get(&trigger.message.unit_id) {
        q.get_mut(e)
            .unwrap()
            .effect_list
            .get_mut(&trigger.message.effect_id)
            .unwrap()
            .data = trigger.message.custom_data.clone();
    } else {
        queue.queue.push(trigger.message.clone().into());
    }
}

fn on_effect_removed(
    trigger: Trigger<FromServer<RemoveEffect>>,
    mut q: Query<&mut EffectList>,

    unit_map: Res<UnitMap>,
    mut queue: ResMut<DeferredUnitCommands>,
) {
    if let Some(&e) = unit_map.map.get(&trigger.message.unit_id) {
        q.get_mut(e)
            .unwrap()
            .remove_effect(trigger.message.effect_id);
    } else {
        queue.queue.push(trigger.message.clone().into());
    }
}

#[derive(Default, Debug, Component, Clone, PartialEq, Serialize, Deserialize)]
pub enum CustomData {
    #[default]
    Nil,
    Boolean(bool),
    Integer(i32),
    Number(f64),
    String(String),
    Table(HashMap<String, CustomData>, Vec<CustomData>),
}

impl IntoLua for CustomData {
    fn into_lua(self, lua: &Lua) -> LuaResult<LuaValue> {
        match self {
            CustomData::Nil => Ok(LuaValue::Nil),
            CustomData::Boolean(val) => Ok(LuaValue::Boolean(val)),
            CustomData::Integer(val) => Ok(LuaValue::Integer(val)),
            CustomData::Number(val) => Ok(LuaValue::Number(val)),
            CustomData::String(val) => val.into_lua(lua),
            CustomData::Table(hash_map, vec) => {
                let table = lua.create_table()?;
                for (key, val) in hash_map {
                    table.raw_set(key, val)?;
                }
                for val in vec {
                    table.raw_push(val)?;
                }
                Ok(LuaValue::Table(table))
            }
        }
    }
}

impl FromLua for CustomData {
    fn from_lua(value: LuaValue, lua: &Lua) -> LuaResult<Self> {
        match value {
            LuaNil => Ok(Self::Nil),
            LuaValue::Boolean(val) => Ok(Self::Boolean(val)),
            LuaValue::Integer(val) => Ok(Self::Integer(val)),
            LuaValue::Number(val) => Ok(Self::Number(val)),
            LuaValue::String(val) => Ok(Self::String(val.to_string_lossy())),
            LuaValue::Table(table) => {
                let mut map = HashMap::new();
                let mut vec = vec![];
                for pair in table.pairs::<String, CustomData>() {
                    let (key, val) = pair?;
                    map.insert(key, val);
                }
                for val in table.sequence_values::<CustomData>() {
                    vec.push(val?);
                }
                Ok(Self::Table(map, vec))
            }
            _ => Err(LuaError::external(anyhow::anyhow!(
                "Invalid value {:?} in CustomData",
                value
            ))),
        }
    }
}
