use bevy::prelude::*;
use bevy_enhanced_input::prelude::*;
use lightyear::prelude::*;
use lobby_common::Team;
use vleue_navigator::prelude::*;

use crate::ingame::unit::attack::CurrentlyAutoAttacking;
use crate::AppExt;
use crate::ingame::lua::LuaCtx;
use crate::ingame::lua::LuaExt;
use crate::ingame::lua::Protos;
use crate::ingame::targetable::Health;
use crate::ingame::unit::MyTeam;
use crate::ingame::unit::UnitId;
use crate::ingame::unit::UnitProxy;
use crate::ingame::unit::attack::AutoAttackTarget;
use crate::ingame::unit::attack::SetAutoAttackTarget;
use crate::ingame::unit::state::State;
use crate::ingame::unit::state::StateList;
use crate::ingame::unit::state::StateProto;
use crate::ingame::unit::stats::StatBlock;

use super::MovementTarget;

use super::Unit;

use super::ControlledByClient;

use super::SetUnitMovementTarget;

use super::super::map::MessageChannel;

use super::super::camera::MousePos;

pub fn plugin(app: &mut App) {
    app.register_component::<CurrentPath>(ChannelDirection::ServerToClient);

    app.add_observer(on_movement_start);
    app.add_observer(on_movement_end);

    if app.is_client() {
        app.add_input_context::<UnitControlContext>()
            .add_observer(bind_input)
            .add_observer(on_move_click)
            .add_systems(Startup, |mut commands: Commands| {
                commands.spawn(Actions::<UnitControlContext>::default());
            })
            // .add_systems(Update, draw_current_path)
            ;
        app.add_systems(Update, move_unit_along_path);
    } else {
        app.add_systems(FixedUpdate, unit_pathfinding);
        app.add_systems(FixedUpdate, refresh_movement_target_on_navmesh_reload);
        app.add_observer(on_set_unit_movement_target);
        app.add_systems(FixedUpdate, move_unit_along_path);
    }
}

#[derive(InputContext)]
pub struct UnitControlContext;

#[derive(Debug, InputAction)]
#[input_action(output = bool)]
pub struct MoveClick;

pub(crate) fn bind_input(
    trigger: Trigger<Binding<UnitControlContext>>,
    mut actions: Query<&mut Actions<UnitControlContext>>,
) {
    let mut actions = actions.get_mut(trigger.target()).unwrap();
    actions
        .bind::<MoveClick>()
        .to(MouseButton::Right)
        .with_conditions(Pulse::new(0.25));
}

pub(crate) fn on_move_click(
    _trigger: Trigger<Fired<MoveClick>>,
    mouse_pos: Res<MousePos>,
    q: Query<(&UnitId, &GlobalTransform, &Team), With<Health>>,
    my_team: Option<Res<MyTeam>>,
    mut commands: Commands,
) {
    let Some(my_team) = my_team else { return };
    // Check if we clicked on enemy
    for (unit_id, trans, team) in q {
        if *team != my_team.0 && trans.translation().xz().distance(mouse_pos.plane_pos) <= 0.5 {
            commands.client_trigger::<MessageChannel>(SetAutoAttackTarget(*unit_id));
            return;
        }
    }
    commands.client_trigger::<MessageChannel>(SetUnitMovementTarget(mouse_pos.plane_pos));
}

pub(crate) fn on_set_unit_movement_target(
    event: Trigger<FromClients<SetUnitMovementTarget>>,
    // mut unit: Single<&mut MovementTarget>,
    unit: Query<(Entity, &ControlledByClient, &StateList), With<Unit>>,
    state_protos: Res<Protos<StateProto>>,
    mut commands: Commands,
) {
    // let client = event.from();

    // We need some way to get the currently controlled unit for this player.
    for (unit, client, state) in unit {
        if client.0 == event.from {
            let (proto, _) = state_protos.get(&state.current_state().proto).unwrap();
            if proto.move_cancellable {
                commands
                    .entity(unit)
                    .remove::<(AutoAttackTarget, CurrentlyAutoAttacking)>()
                    .insert(MovementTarget(event.message.0));

                if let Some(on_cancel) = proto.on_move_cancel.clone() {
                    commands.queue(move |world: &mut World| {
                        let lua = world.resource::<LuaCtx>().0.clone();
                        lua.with_world(world, |_| {
                            on_cancel.call::<()>(UnitProxy { entity: unit }).unwrap();
                        });
                    });
                }
            }
        }
    }
}

pub(crate) fn refresh_movement_target_on_navmesh_reload(
    units: Query<&mut MovementTarget>,
    navmesh: Single<(&ManagedNavMesh, Ref<NavMeshStatus>)>,
) {
    if navmesh.1.is_changed() {
        for mut unit in units {
            unit.set_changed();
        }
    }
}

pub(crate) fn unit_pathfinding(
    mut units: Query<
        (
            Entity,
            &Transform,
            &MovementTarget,
            Option<&mut CurrentPath>,
        ),
        Changed<MovementTarget>,
    >,
    navmesh: Single<(&ManagedNavMesh, Ref<NavMeshStatus>)>,
    assets: Res<Assets<NavMesh>>,
    // mut gizmos: Gizmos,
    mut commands: Commands,
) {
    let navmesh = navmesh.0;
    let Some(navmesh) = assets.get(navmesh) else {
        return;
    };

    for (e, trans, target, cur_path) in &mut units {
        let end = vec3(target.0.x, 0.0, target.0.y);

        if let Some(path) = get_path(&mut commands, navmesh, e, trans, cur_path, end) {
            commands.entity(e).insert(CurrentPath(path));
        }
    }
}

fn get_path(
    commands: &mut Commands<'_, '_>,
    navmesh: &NavMesh,
    e: Entity,
    trans: &Transform,
    cur_path: Option<Mut<'_, CurrentPath>>,
    end: Vec3,
) -> Option<Vec<Vec3>> {
    let (closest_start, start_on_navmesh) = get_closest_point(navmesh, trans.translation);
    let (closest_end, _end_on_navmesh) = get_closest_point(navmesh, end);

    if let Some(path) = navmesh.get().path(closest_start, closest_end) {
        let mut path: Vec<Vec3> = path
            .path
            .iter()
            .map(|vec2| navmesh.transform().transform_point(vec2.extend(0.0)))
            .collect();
        // if let Some(mut cur_path) = cur_path {
        //     cur_path.0 = path;
        // } else {
        //     commands.entity(e).insert(CurrentPath(path));
        // }

        if !start_on_navmesh {
            path.insert(
                0,
                navmesh
                    .transform()
                    .transform_point(closest_start.extend(0.0)),
            );
        }

        Some(path)
    } else {
        warn!(
            "Pathfinding failed (from {} to {})",
            navmesh
                .world_to_mesh()
                .transform_point3(trans.translation)
                .xy(),
            closest_end
        );
        None
    }
}

fn get_closest_point(navmesh: &NavMesh, end: Vec3) -> (Vec2, bool) {
    let local_end = navmesh.world_to_mesh().transform_point3(end).xy();

    let mut closest_point = vec2(f32::INFINITY, f32::INFINITY);
    let mut closest_dist = f32::INFINITY;

    if navmesh.is_in_mesh(local_end) {
        (local_end, true)
    } else {
        let layer = &navmesh.get().layers[0];
        for polygon in &layer.polygons {
            for [a, b] in polygon
                .vertices
                .array_windows()
                .copied()
                .chain(std::iter::once([
                    polygon.vertices[0],
                    *polygon.vertices.last().unwrap(),
                ]))
            {
                let a_vert = &layer.vertices[a as usize];
                let b_vert = &layer.vertices[b as usize];

                let segment = Segment2d::new(a_vert.coords, b_vert.coords);
                let segment_relative = local_end - segment.point1();
                let scalar_proj = segment_relative.dot(segment.direction().as_vec2());

                let scalar_proj = scalar_proj.clamp(0.0, segment.length());

                let point = segment.point1() + segment.direction().as_vec2() * scalar_proj;

                let dist = point.distance_squared(local_end);
                if dist < closest_dist {
                    closest_point = point;
                    closest_dist = dist;
                }
            }
        }
        (closest_point, false)
    }
}

#[derive(Component, Clone, PartialEq, Serialize, Deserialize)]
pub(crate) struct CurrentPath(Vec<Vec3>);

pub(crate) fn draw_current_path(
    units: Query<(&Transform, &CurrentPath, &Visibility)>,
    mut gizmos: Gizmos,
) {
    for (trans, path, visible) in &units {
        if path.0.is_empty() || *visible == Visibility::Hidden {
            return;
        }
        let mut start = trans.translation;
        start.y = 0.06;
        for [a, b] in std::iter::once([start, *path.0.first().unwrap()])
            .chain(path.0.array_windows().copied())
        {
            gizmos.line(a, b, Color::srgb(1.0, 0.0, 0.0));
        }
    }
}

pub(crate) fn move_unit_along_path(
    mut units: Query<(Entity, &mut Transform, &mut CurrentPath, &StatBlock)>,
    time: Res<Time>,
    mut commands: Commands,
) {
    for (e, mut trans, mut path, stats) in &mut units {
        let speed = stats.move_speed.base;
        let mut travel_dist = time.delta_secs() * speed;

        while travel_dist > 0.0001 {
            if let Some(next_step) = path.0.first() {
                trans.look_at(*next_step, Vec3::Y);

                let pos = trans.translation;
                let newpos = pos.move_towards(*next_step, travel_dist);
                trans.translation = newpos;

                let travelled_dist = pos.distance(newpos);
                if trans.translation == *next_step {
                    path.0.remove(0);
                }

                travel_dist -= travelled_dist;
            } else {
                commands.entity(e).remove::<CurrentPath>();
                break;
            }
        }
    }
}

fn on_movement_start(
    trigger: Trigger<OnAdd, CurrentPath>,
    mut q: Query<&mut StateList>,
    states: Res<Protos<StateProto>>,
) {
    if let Ok(mut state_list) = q.get_mut(trigger.target()) {
        state_list.add_state(State::moving(), &states);
    }
}

fn on_movement_end(trigger: Trigger<OnRemove, CurrentPath>, mut q: Query<&mut StateList>) {
    if let Ok(mut state_list) = q.get_mut(trigger.target()) {
        state_list.remove_state(&State::moving());
    }
}
