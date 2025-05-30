use std::{f32::consts::FRAC_PI_2, ops::Deref};

use bevy::{asset::uuid::Uuid, platform::collections::HashMap, prelude::*, window::WindowResized};
use lightyear::prelude::{AppComponentExt, ChannelDirection, ServerReplicate};
use serde::{Deserialize, Serialize};
use vleue_navigator::{
    prelude::{
        CachedObstacle, ManagedNavMesh, NavMeshSettings, NavMeshStatus, NavMeshUpdateMode, NavmeshUpdaterPlugin, ObstacleSource
    }, NavMesh, Triangulation, VleueNavigatorPlugin
};

use crate::AppExt;

use super::terrain::Terrain;

pub fn common(app: &mut App) {
    app.add_plugins((
        VleueNavigatorPlugin,
        NavmeshUpdaterPlugin::<TerrainData>::default(),
    ));

    app.register_component::<TerrainData>(ChannelDirection::ServerToClient);

    // Temporary setup
    app.add_systems(Startup, setup);
    if app.is_server() {
        app.add_systems(Update, update_terrain.run_if(resource_changed::<Terrain>).after(super::lua::execute_lua));
    }

    if app.is_client() {
        info!("ADDING DANGEROUS STUFF");
        app.init_resource::<CurrentMeshEntity>()
            .register_type::<DrawNavmesh>()
            .insert_resource(DrawNavmesh(true))
            .add_systems(
                Update,
                (
                    despawn_navmesh_display_on_navmesh.run_if(
                        resource_changed::<DrawNavmesh>.and(resource_equals(DrawNavmesh(false))),
                    ),
                    display_mesh.run_if(resource_equals(DrawNavmesh(true))),
                ),
            );
    }
}

fn setup(mut commands: Commands) {
    commands.spawn((
        NavMeshSettings {
            fixed: Triangulation::from_outer_edges(&[
                vec2(0.0, 0.0),
                vec2(100.0, 0.0),
                vec2(100.0, 100.0),
                vec2(0.0, 100.0),
            ]),
            agent_radius: 0.5,
            merge_steps: 3,
            simplify: 0.01,
            // default_search_delta: 0.1,
            // default_search_steps: 100,
            ..default()
        },
        NavMeshUpdateMode::Direct,
        Transform::from_translation(vec3(-50.0, 0.05, 50.0))
            .with_rotation(Quat::from_rotation_x(-FRAC_PI_2)),
    ));
}

fn update_terrain(
    terrain: Res<Terrain>,
    entity_map: Local<HashMap<Uuid, Entity>>,
    terrain_data: Query<(Entity, &mut TerrainData), With<SpawnedTerrainEntity>>,
    mut commands: Commands,
) {
    info!("Updating terrain! ({} objects)", terrain.objects.len());
    for (e, _) in &terrain_data {
        commands.entity(e).despawn();
    }
    info!("All terrain objects have been despawned");

    for object in &terrain.objects {
        info!("Spawning TerrainData entity");
        commands.spawn((
            Transform::default(),
            TerrainData {
                // uuid: object.uuid,
                vertices: object.vertices.clone(),
            },
            ServerReplicate::default(),
            SpawnedTerrainEntity,
        ));
    }
}

#[derive(Component)]
pub struct SpawnedTerrainEntity;

#[derive(Resource, Default, Deref, DerefMut)]
struct CurrentMeshEntity(Option<Entity>);

#[derive(Resource, Clone, Copy, PartialEq, Eq, Reflect)]
#[reflect(Resource)]
struct DrawNavmesh(bool);

fn despawn_navmesh_display_on_navmesh(
    mut current_mesh_entity: ResMut<CurrentMeshEntity>,
    mut commands: Commands,
) {
    if let Some(entity) = current_mesh_entity.0 {
        commands.entity(entity).despawn();
        current_mesh_entity.0 = None;
    }
}

fn display_mesh(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    navmeshes: Res<Assets<NavMesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut current_mesh_entity: ResMut<CurrentMeshEntity>,
    window_resized: EventReader<WindowResized>,
    navmesh: Single<(&ManagedNavMesh, Ref<NavMeshStatus>)>,
) {
    let (navmesh_handle, status) = navmesh.deref();
    if (!status.is_changed() || **status != NavMeshStatus::Built) && window_resized.is_empty() {
        if current_mesh_entity.is_some() {
            return;
        }
    }

    info!("Rebuilding navmesh display");

    let Some(navmesh) = navmeshes.get(*navmesh_handle) else {
        return;
    };
    if let Some(entity) = current_mesh_entity.0 {
        commands.entity(entity).despawn();
    }

    current_mesh_entity.0 = Some(
        commands
            .spawn((
                Mesh3d(meshes.add(navmesh.to_mesh())),
                MeshMaterial3d(
                    materials.add(StandardMaterial::from(Color::oklch(0.424, 0.199, 265.638))),
                ),
            ))
            .with_children(|main_mesh| {
                main_mesh.spawn((
                    Mesh3d(meshes.add(navmesh.to_wireframe_mesh())),
                    Transform::from_translation(Vec3::new(0.0, 0.0, 0.1)),
                    MeshMaterial3d(
                        materials.add(StandardMaterial::from(Color::oklch(0.855, 0.138, 181.071))),
                    ),
                ));
            })
            .id(),
    );
}

#[derive(Component, Clone, PartialEq, Serialize, Deserialize)]
pub struct TerrainData {
    // uuid: Uuid,
    pub vertices: Vec<Vec2>,
}

impl ObstacleSource for TerrainData {
    fn get_polygons(
        &self,
        obstacle_transform: &GlobalTransform,
        navmesh_transform: &Transform,
        up: (Dir3, f32),
    ) -> Vec<Vec<Vec2>> {
        info!("GETTING POLYGONS");
        let inverse = navmesh_transform.compute_affine().inverse();

        let transform = obstacle_transform.compute_transform();
        let shift = transform.translation.xz();

        let to_navmesh = |v: Vec2| inverse.transform_point3(vec3(v.x + shift.x, 0.0, v.y + shift.y)).xy();

        let vertices: Vec<Vec2> = self
            .vertices
            .iter()
            .copied()
            .map(|vertex| to_navmesh(vertex))
            .collect();

        vec![vertices]
    }
}
