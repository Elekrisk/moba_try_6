use bevy::{ecs::entity::MapEntities, prelude::*};
use lightyear::prelude::{
    AppComponentExt, AppIdentityExt, AppTriggerExt, ChannelDirection, FromClients,
};
use lobby_common::Team;
use mlua::{FromLua, IntoLua};
use serde::{Deserialize, Serialize};

use crate::{
    ingame::{
        lua::{LuaCtx, LuaExt, Protos}, projectile::{SpawnProjectile, SpawnProjectileArgs}, structure::Model, targetable::{Health, Position}, unit::{
            animation::{AnimationPlayerProxy, GltfAnimations}, movement::CurrentPath, state::{State, StateList, StateProto}, stats::StatBlock, ControlledByClient, MovementTarget, UnitId, UnitMap, UnitProxy
        }, vision::VisibleBy
    }, AppExt, Options
};

pub fn plugin(app: &mut App) {
    app.register_component::<AutoAttackTarget>(ChannelDirection::ServerToClient)
        .add_map_entities();
    app.register_component::<AutoAttackTimer>(ChannelDirection::ServerToClient);
    app.register_component::<AutoAttackType>(ChannelDirection::ServerToClient);
    app.register_component::<CurrentlyAutoAttacking>(ChannelDirection::ServerToClient)
        .add_map_entities();
    app.register_trigger::<SetAutoAttackTarget>(ChannelDirection::ClientToServer);

    if app.is_server() {
        app.add_observer(on_set_auto_attack_target);
        app.add_observer(on_start_aa);
        app.add_observer(on_stop_aa);
        // app.add_observer(on_aa_stopped);
        app.add_systems(FixedUpdate, perform_auto_attack);
        app.add_systems(FixedUpdate, auto_attack);
    }
}

#[derive(Component, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct AutoAttackTimer {
    /// Timer ticking down until next auto attack can be performed
    pub timer: f32,
    /// How long the auto attack is from start to finish
    pub total_time: f32,
    /// How long into the auto attack the attack occurs,
    pub threshold: f32,
    /// If the auto attack has occurred this cycle
    pub has_occurred: bool,
}

impl Default for AutoAttackTimer {
    fn default() -> Self {
        Self {
            timer: 0.0,
            total_time: 1.0,
            threshold: 0.3,
            has_occurred: false,
        }
    }
}

impl AutoAttackTimer {
    fn set(&mut self, time: f32) {
        // info!("SET");
        self.timer = time;
        self.total_time = time;
        // TODO: per champion stat?
        self.threshold = time * 0.3;
        self.has_occurred = false;
    }

    fn tick(&mut self, delta: f32) -> bool {
        self.timer -= delta;
        if !self.has_occurred && self.timer < (self.total_time - self.threshold) {
            // info!(
            //     "Attack has reached threshold: occured? {}; self.timer? {}; self.total_time? {}; self.threshold? {}",
            //     self.has_occurred, self.timer, self.total_time, self.threshold
            // );
            self.has_occurred = true;
            true
        } else {
            false
        }
    }
}

fn perform_auto_attack(
    q: Query<(
        Entity,
        &mut AutoAttackTimer,
        &mut StateList,
        Option<&AutoAttackTarget>,
        Option<&CurrentlyAutoAttacking>,
    )>,
    time: Res<Time>,
    mut commands: Commands,
) {
    for (e, mut timer, mut state_list, target, is_autoing) in q {
        if timer.timer > 0.0 {
            // info!("Timer: {}", timer.timer);
            if timer.tick(time.delta_secs())
                && let Some(cur_target) = is_autoing
                && let Some(target) = target
                && cur_target.target == target.0
            {
                // info!("Do the attack!");
                // Do the attack
                // TODO: on-attack & on-hit effects? resistances? attack scaling?
                commands.queue(PerformAutoAttack {
                    source: e,
                    target: target.0,
                });
            }
        } else {
            commands.entity(e).remove::<CurrentlyAutoAttacking>();
        }
    }
}

// fn on_aa_stopped(trigger: Trigger<OnRemove, CurrentlyAutoAttacking>, mut q: Query<&mut StateList>) {
//     if let Ok(mut state_list) = q.get_mut(trigger.target()) {
//         state_list.remove_state(&State::new("attacking"));
//     }
// }

#[derive(Component, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct AutoAttackTarget(pub Entity);

impl MapEntities for AutoAttackTarget {
    fn map_entities<E: EntityMapper>(&mut self, entity_mapper: &mut E) {
        self.0 = entity_mapper.get_mapped(self.0);
    }
}

#[derive(Component, Clone, Copy, PartialEq, Serialize, Deserialize, MapEntities)]
pub struct CurrentlyAutoAttacking {
    target: Entity,
    speed: f32,
}

fn auto_attack(
    q: Query<(
        Entity,
        &Position,
        &AutoAttackTarget,
        &StatBlock,
        &Team,
        &mut AutoAttackTimer,
        Option<&CurrentlyAutoAttacking>,
    )>,
    target: Query<(&Position, &VisibleBy)>,
    opt: Option<Res<Options>>,
    mut commands: Commands,
) {
    let is_client = opt.is_some();
    for (e, pos, aa_target, stats, team, mut aa_timer, cur_target) in q {
        let Ok((target_pos, vis)) = target.get(aa_target.0) else {
            // Target entity has been despawned
            let mut ec = commands.entity(e);
            ec.remove::<AutoAttackTarget>();
            if !aa_timer.has_occurred {
                ec.remove::<CurrentlyAutoAttacking>();
            }
            continue;
        };
        if vis.0.contains(team) {
            // is in range?
            if target_pos
                .distance(**pos)
                <= stats.range.base
            {
                // let mut trans = trans_q.get_mut(e).unwrap();
                // trans.look_at(target1, Vec3::Y);
                if !is_client {
                    commands.entity(e).remove::<MovementTarget>();
                    commands.entity(e).remove::<CurrentPath>();
                }
                if aa_timer.timer <= 0.0 {
                    // we perform auto attack
                    commands.entity(e).insert(CurrentlyAutoAttacking {
                        target: aa_target.0,
                        speed: stats.attack_speed.base,
                    });
                    aa_timer.set(stats.attack_speed.base.recip());
                } else {
                    // we stand and wait
                }
            } else if cur_target.is_none() || cur_target.unwrap().target != aa_target.0 {
                // Move towards unit
                // info!("MOVE TOWARDS UNIT");
                commands
                    .entity(e)
                    .remove::<CurrentlyAutoAttacking>()
                    .insert(MovementTarget(*target_pos));
            }
        } else {
            // Cannot see; abort attack
            // We only remove CurrentlyAutoAttacking if the auto attack hasn't gone through
            // This prevents the attack animation immediately stopping upon target death
            let mut ec = commands.entity(e);
            ec.remove::<(AutoAttackTarget, MovementTarget, CurrentPath)>();
            if aa_timer.has_occurred {
                ec.remove::<CurrentlyAutoAttacking>();
            }
        }
    }
}

struct PerformAutoAttack {
    source: Entity,
    target: Entity,
}

impl Command for PerformAutoAttack {
    fn apply(self, world: &mut World) -> () {
        // What type of auto attack? Melee? Ranged?
        let attack_type = world.entity(self.source).get::<AutoAttackType>().unwrap();
        match attack_type {
            AutoAttackType::Melee => {
                let amount = world
                    .entity(self.source)
                    .get::<StatBlock>()
                    .unwrap()
                    .attack_a
                    .base;
                if let Ok(e) = world.get_entity_mut(self.target) {
                    // We instantly apply damage
                    ApplyDamage {
                        source: self.source,
                        // TODO: figure out auto attack damage scaling
                        // For now, use base attack_a
                        amount,
                    }
                    .apply(e);
                }
            }
            AutoAttackType::Projectile(proto) => {
                // We first create a projectile that then deals damage
                let proto = proto.clone().unwrap();

                let pos = **world.entity(self.source).get::<Position>().unwrap();

                SpawnProjectile(SpawnProjectileArgs {
                    proto,
                    position: pos,
                    source_unit: UnitProxy { entity: self.source },
                    target: crate::ingame::projectile::ProjectileTarget::Entity(self.target),
                    speed: 5.0,
                }).apply(world);
            }
        }
    }
}

struct ApplyDamage {
    source: Entity,
    amount: f32,
}

impl EntityCommand for ApplyDamage {
    fn apply(self, mut entity: EntityWorldMut) -> () {
        // TODO: on-hit effects, resistances, lua callbacks
        let Some(mut health) = entity.get_mut::<Health>() else {
            return;
        };

        health.0 -= self.amount;
    }
}

#[derive(Debug, Component, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum AutoAttackType {
    Melee,
    Projectile(Option<String>),
}

impl FromLua for AutoAttackType {
    fn from_lua(value: mlua::Value, _lua: &mlua::Lua) -> mlua::Result<Self> {
        match value.to_string()?.as_str() {
            "melee" => Ok(Self::Melee),
            "projectile" => Ok(Self::Projectile(None)),
            e => Err(mlua::Error::external(anyhow::anyhow!(
                "{e} is not a valid auto attack type; must be one of melee or projectile"
            ))),
        }
    }
}

impl IntoLua for AutoAttackType {
    fn into_lua(self, lua: &mlua::Lua) -> mlua::Result<mlua::Value> {
        match self {
            AutoAttackType::Melee => "melee",
            AutoAttackType::Projectile(_) => "projectile",
        }
        .into_lua(lua)
    }
}

#[derive(Event, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SetAutoAttackTarget(pub UnitId);

fn on_set_auto_attack_target(
    trigger: Trigger<FromClients<SetAutoAttackTarget>>,
    q: Query<(
        Entity,
        &ControlledByClient,
        &mut StateList,
        Option<&CurrentlyAutoAttacking>,
    )>,
    unit_map: Res<UnitMap>,
    state_protos: Res<Protos<StateProto>>,
    mut commands: Commands,
) {
    info!("Received set auto attack");
    for (e, control, mut state_list, cur_target) in q {
        if control.0 == trigger.from {
            info!("Setting auto attack target");
            let new_target = *unit_map.map.get(&trigger.message.0).unwrap();
            if let Some(cur_target) = cur_target
                && cur_target.target != new_target
            {
                commands.entity(e).remove::<CurrentlyAutoAttacking>();
                // Queue on_move_cancel

                let (proto, _) = state_protos.get(&state_list.current_state().proto).unwrap();
                // if proto.move_cancellable {
                commands.entity(e).remove::<CurrentlyAutoAttacking>();

                if let Some(on_cancel) = proto.on_move_cancel.clone() {
                    commands.queue(move |world: &mut World| {
                        let lua = world.resource::<LuaCtx>().0.clone();
                        lua.with_world(world, |_| {
                            on_cancel.call::<()>(UnitProxy { entity: e }).unwrap();
                        });
                    });
                }
                // }
            }
            commands.entity(e).insert(AutoAttackTarget(
                *unit_map.map.get(&trigger.message.0).unwrap(),
            ));
        }
    }
}

fn on_start_aa(
    trigger: Trigger<OnInsert, CurrentlyAutoAttacking>,
    mut q: Query<(&mut StateList, &CurrentlyAutoAttacking)>,
    states: Res<Protos<StateProto>>,
) {
    let (mut state_list, aa) = q.get_mut(trigger.target()).unwrap();
    state_list.remove_state("attacking");
    state_list.add_state(
        State::new("attacking").with_animation_speed(aa.speed),
        &states,
    );
}

fn on_stop_aa(
    trigger: Trigger<OnRemove, CurrentlyAutoAttacking>,
    mut q: Query<(&mut AutoAttackTimer, &mut StateList)>,
) {
    let (mut aa, mut state_list) = q.get_mut(trigger.target()).unwrap();
    state_list.remove_state("attacking");
    if !aa.has_occurred {
        aa.timer = 0.0;
        aa.has_occurred = false;
    }
}
