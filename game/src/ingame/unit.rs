use core::f32;

use bevy::{asset::AssetPath, prelude::*};
use bevy_enhanced_input::prelude::*;
use lightyear::prelude::{client::{VisualInterpolateStatus, VisualInterpolationPlugin}, *};
use mlua::prelude::*;
use vleue_navigator::{TransformedPath, prelude::*};

use crate::AppExt;

use super::{
    camera::MousePos,
    lua::{AppLuaExt, AssetPathExt, LuaExt, Protos},
    map::{MapEntity, MessageChannel},
    structure::Model,
};

pub fn common(app: &mut App) {
    app.register_trigger::<SetUnitMovementTarget>(ChannelDirection::ClientToServer);
    app.register_component::<MovementTarget>(ChannelDirection::ServerToClient);
    app.register_component::<CurrentPath>(ChannelDirection::ServerToClient);

    app.init_resource::<Protos<UnitProto>>();

    if app.is_client() {
        app.add_input_context::<UnitControlContext>()
            .add_observer(bind_input)
            .add_observer(on_move_click)
            .add_systems(Startup, |mut commands: Commands| {
                info!("Spawning UnitControlContext actions");
                commands.spawn(Actions::<UnitControlContext>::default());
            })
            .add_systems(Update, draw_current_path);
        app.add_systems(Update, move_unit_along_path);
    } else {
        app.add_systems(FixedUpdate, unit_pathfinding);
        app.add_systems(FixedUpdate, refresh_movement_target_on_navmesh_reload);
        app.add_observer(on_set_unit_movement_target);
        app.add_systems(FixedUpdate, move_unit_along_path);
    }

    app.add_systems(FixedUpdate, |q: Query<Entity, Changed<CurrentPath>>| {
        for e in &q {
            info!("CURRENT PATH CHANGED FOR {e}");
        }
    });

    app.setup_lua(setup_lua);
}

fn setup_lua(lua: &Lua) -> LuaResult<()> {
    let game = lua.table("game")?;

    game.set(
        "register_unit",
        lua.create_function(|lua, proto: LuaValue| {
            lua.world()
                .resource_mut::<Protos<UnitProto>>()
                .insert(proto, lua.current_path())
        })?,
    )?;

    game.set(
        "spawn_unit",
        lua.create_function(|lua, args: SpawnUnitArgs| {
            if lua.is_client() {
                return Ok(());
            }

            let mut world = lua.world();

            let (proto, origin) = world.resource_mut::<Protos<UnitProto>>().get(&args.proto)?;

            world.spawn((
                Transform::from_xyz(args.position.x, 0.0, args.position.y),
                Model(proto.model.relative(&origin)),
                Unit,
                // MovementTarget(vec2(0.0, 0.0)),
                // VisualInterpolateStatus::<Transform>::default(),
                MapEntity,
                ServerReplicate::default(),
            ));

            Ok(())
        })?,
    )?;

    Ok(())
}

struct SpawnUnitArgs {
    proto: String,
    position: Vec2,
}

from_into_lua_table!(
    struct SpawnUnitArgs {
        proto: String,
        position: {W} Vec2,
    }
);

struct UnitProto {
    id: String,
    name: String,
    model: AssetPath<'static>,
}

proto!(
struct UnitProto {
    id: String,
    name: String,
    model: {W} AssetPath<'static>
});

/// Marker struct for units
#[derive(Component, Clone, PartialEq, Serialize, Deserialize)]
pub struct Unit;

/// Where this unit currently wants to go
#[derive(Component, Clone, PartialEq, Serialize, Deserialize)]
pub struct MovementTarget(pub Vec2);

/// Message for clients to set their controlled unit's movement target.
#[derive(Debug, Event, Clone, Serialize, Deserialize)]
pub struct SetUnitMovementTarget(Vec2);

// --- CLIENT ---

#[derive(InputContext)]
pub struct UnitControlContext;

#[derive(Debug, InputAction)]
#[input_action(output = bool)]
pub struct MoveClick;

fn bind_input(
    trigger: Trigger<Binding<UnitControlContext>>,
    mut actions: Query<&mut Actions<UnitControlContext>>,
) {
    let mut actions = actions.get_mut(trigger.target()).unwrap();
    info!("Binding move click!");
    actions
        .bind::<MoveClick>()
        .to(MouseButton::Right)
        .with_conditions(Pulse::new(0.25));
}

fn on_move_click(
    _trigger: Trigger<Fired<MoveClick>>,
    mouse_pos: Res<MousePos>,
    mut commands: Commands,
) {
    info!("Sending move click!");
    commands.client_trigger::<MessageChannel>(SetUnitMovementTarget(mouse_pos.plane_pos));
}

// --- SERVER ---

fn on_set_unit_movement_target(
    event: Trigger<FromClients<SetUnitMovementTarget>>,
    // mut unit: Single<&mut MovementTarget>,
    unit: Query<Entity, With<Unit>>,
    mut commands: Commands,
    time: Res<Time>,
) {
    let client = event.from();

    // We need some way to get the currently controlled unit for this player.
    // For now, we just assume it is ALL UNITS.
    for unit in &unit {
        commands
            .entity(unit)
            .insert(MovementTarget(event.message.0));
    }
}

// --- COMMON ---

fn refresh_movement_target_on_navmesh_reload(
    units: Query<&mut MovementTarget>,
    navmesh: Single<(&ManagedNavMesh, Ref<NavMeshStatus>)>,
) {
    if navmesh.1.is_changed() {
        for mut unit in units {
            unit.set_changed();
        }
    }
}

fn unit_pathfinding(
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

                    // gizmos.line(
                    //     vec3(a_vert.coords.x - 50.0, 0.6, 50.0 - a_vert.coords.y),
                    //     vec3(b_vert.coords.x - 50.0, 0.6, 50.0 - b_vert.coords.y),
                    //     Color::WHITE.with_alpha(0.25),
                    // );

                    let segment = Segment2d::new(a_vert.coords, b_vert.coords);
                    let segment_relative = local_end - segment.point1();
                    let scalar_proj = segment_relative.dot(segment.direction().as_vec2());
                    // info!("SCALAR PROJ: {}", scalar_proj);

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
            // info!("{path:#?}");
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
        }
    }
}

#[derive(Component, Clone, PartialEq, Serialize, Deserialize)]
struct CurrentPath(Vec<Vec3>);

fn draw_current_path(units: Query<(&Transform, &CurrentPath)>, mut gizmos: Gizmos) {
    for (trans, path) in &units {
        if path.0.is_empty() {
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

fn move_unit_along_path(
    mut units: Query<(Entity, &mut Transform, &mut CurrentPath)>,
    time: Res<Time>,
    mut commands: Commands,
) {
    let speed = 10.0;
    for (e, mut trans, mut path) in &mut units {
        let mut travel_dist = time.delta_secs() * speed;

        while travel_dist > 0.01 {
            if let Some(next_step) = path.0.first() {
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
