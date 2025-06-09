use bevy::{ecs::entity::MapEntities, prelude::*};
use lightyear::prelude::{AppComponentExt, AppIdentityExt, AppTriggerExt, ChannelDirection, FromClients};
use lobby_common::Team;
use serde::{Deserialize, Serialize};

use crate::{ingame::{structure::Model, unit::{animation::{AnimationPlayerProxy, GltfAnimations}, movement::CurrentPath, stats::StatBlock, ControlledByClient, MovementTarget, UnitId, UnitMap}, vision::VisibleBy}, AppExt};

pub fn plugin(app: &mut App) {
    app.add_systems(FixedUpdate, decrement_auto_attack_timer);
    app.add_systems(FixedUpdate, auto_attack);

    app.register_component::<AutoAttackTarget>(ChannelDirection::ServerToClient).add_map_entities();
    app.register_trigger::<SetAutoAttackTarget>(ChannelDirection::ClientToServer);

    if app.is_server() {
        app.add_observer(on_set_auto_attack_target);
    }
}

#[derive(Component, Default)]
pub struct AutoAttackTimer(pub f32);

fn decrement_auto_attack_timer(q: Query<&mut AutoAttackTimer>, time: Res<Time>) {
    for mut timer in q {
        if timer.0 > 0.0 {
            timer.0 -= time.delta_secs();
        }
    }
}

#[derive(Component, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct AutoAttackTarget(Entity);

impl MapEntities for AutoAttackTarget {
    fn map_entities<E: EntityMapper>(&mut self, entity_mapper: &mut E) {
        self.0 = entity_mapper.get_mapped(self.0);
    }
}

fn auto_attack(q: Query<(Entity, &AutoAttackTarget, &StatBlock, &Team, &Transform, &mut AutoAttackTimer)>, target: Query<(&VisibleBy, &Transform)>, mut commands: Commands) {
    for (e, aa_target, stats, team, trans, mut aa_timer) in q {
        let (vis, target_trans) = target.get(aa_target.0).unwrap();
        if vis.0.contains(team) {
            // is in range?
            if target_trans.translation.xz().distance(trans.translation.xz()) <= stats.range.base {
                commands.entity(e).remove::<MovementTarget>();
                commands.entity(e).remove::<CurrentPath>();
                if aa_timer.0 <= 0.0 {
                    // we perform auto attack
                    // TODO: auto attack
                    info!("DO AUTO ATTACK");
                    commands.entity(e).queue(DoAutoAttack {
                        target: aa_target.0
                    });
                    aa_timer.0 = stats.attack_speed.base;
                } else {
                    // we stand and wait
                }
            } else {
                // Move towards unit
                info!("MOVE TOWARDS UNIT");
                commands.entity(e).insert(MovementTarget(target_trans.translation.xz()));
            }
        } else {
            // Cannot see; abort attack
            info!("CANNOT SEE; ABORT");
            commands.entity(e).remove::<(AutoAttackTarget, MovementTarget, CurrentPath)>();
        }
    }
}

struct DoAutoAttack {
    target: Entity
}

impl EntityCommand for DoAutoAttack {
    fn apply(self, entity: EntityWorldMut) -> () {
        if entity.world().is_client() {
            // Play auto attack animation once
            let proxy = entity.get::<AnimationPlayerProxy>().unwrap().0;
            let model = &entity.get::<Model>().unwrap().0;
            let gltf_handle = entity.resource::<AssetServer>().get_handle(model.path()).unwrap();
            let world = entity.into_world_mut();
            world.resource_scope::<GltfAnimations, _>(|world, anims| {
                let (_, anims) = anims.map.get(&gltf_handle).unwrap();
                let mut binding = world.entity_mut(proxy);
                let mut animation_player = binding.get_mut::<AnimationPlayer>().unwrap();
                let anim = *anims.get("AutoAttack").or_else(|| anims.get("Idle")).unwrap();
                animation_player.stop_all();
                animation_player.play(anim);
            });
        }
    }
}

#[derive(Event, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SetAutoAttackTarget(pub UnitId);

fn on_set_auto_attack_target(trigger: Trigger<FromClients<SetAutoAttackTarget>>, q: Query<(Entity, &ControlledByClient)>, unit_map: Res<UnitMap>, mut commands: Commands) {
    info!("Received set auto attack");
    for (e, control) in q {
        if control.0 == trigger.from {
            info!("Setting auto attack target");
            commands.entity(e).insert(AutoAttackTarget(*unit_map.map.get(&trigger.message.0).unwrap()));
        }
    }
}
