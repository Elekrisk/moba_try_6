use bevy::prelude::*;
use bevy_enhanced_input::prelude::*;
use lightyear::prelude::*;
use lobby_common::Team;
use vleue_navigator::prelude::*;

use crate::AppExt;
use crate::ingame::targetable::Health;
use crate::ingame::unit::MyTeam;
use crate::ingame::unit::UnitId;
use crate::ingame::unit::attack::AutoAttackTarget;
use crate::ingame::unit::attack::SetAutoAttackTarget;
use crate::ingame::unit::stats::StatBlock;

use super::MovementTarget;

use super::Unit;

use super::ControlledByClient;

use super::SetUnitMovementTarget;

use super::super::map::MessageChannel;

use super::super::camera::MousePos;

pub fn plugin(app: &mut App) {
    app.register_component::<CurrentPath>(ChannelDirection::ServerToClient);

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
    unit: Query<(Entity, &ControlledByClient), With<Unit>>,
    mut commands: Commands,
) {
    // let client = event.from();

    // We need some way to get the currently controlled unit for this player.
    for (unit, client) in &unit {
        if client.0 == event.from {
            commands
                .entity(unit)
                .remove::<AutoAttackTarget>()
                .insert(MovementTarget(event.message.0));
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

        let local_end = navmesh.world_to_mesh().transform_point3(end).xy();

        // let Some(end) = navmesh.get().get_closest_point(local_end.xy()) else {
        //     return;
        // };

        let mut closest_point = vec2(f32::INFINITY, f32::INFINITY);
        let mut closest_dist = f32::INFINITY;

        if navmesh.is_in_mesh(local_end) {
            closest_point = local_end
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
        }

        if let Some(path) = navmesh.get().path(
            navmesh
                .world_to_mesh()
                .transform_point3(trans.translation)
                .xy(),
            closest_point,
        ) {
            let path = path
                .path
                .iter()
                .map(|vec2| navmesh.transform().transform_point(vec2.extend(0.0)))
                .collect();
            if let Some(mut cur_path) = cur_path {
                cur_path.0 = path;
            } else {
                commands.entity(e).insert(CurrentPath(path));
            }
        } else {
            warn!(
                "Pathfinding failed (from {} to {})",
                navmesh
                    .world_to_mesh()
                    .transform_point3(trans.translation)
                    .xy(),
                closest_point
            );
        }
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
