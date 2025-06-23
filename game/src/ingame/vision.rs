use std::{
    f32,
    sync::{mpmc, mpsc::TryRecvError},
};

use bevy::{
    asset::RenderAssetUsages, color::palettes, core_pipeline::bloom::Bloom, ecs::{component::Tick, system::SystemChangeTick}, image::ImageSampler, math::{
        bounding::{Aabb2d, Bounded2d, BoundingCircle, IntersectsVolume}, FloatPow
    }, pbr::{NotShadowCaster, NotShadowReceiver}, platform::collections::HashSet, prelude::*, render::{
        camera::{Exposure, ImageRenderTarget, RenderTarget},
        render_resource::{Extent3d, TextureDimension, TextureFormat, TextureUsages},
        view::RenderLayers,
    }
};
use lightyear::prelude::*;
use lobby_common::Team;
use serde::{Deserialize, Serialize};

use crate::{
    AppExt, GameState,
    ingame::{
        terrain::{C, Terrain, TerrainObject},
        unit::MyTeam,
    },
};

use super::targetable::Position;

pub fn plugin(app: &mut App) {
    app.register_component::<VisibleBy>(ChannelDirection::ServerToClient);
    app.register_component::<SightRange>(ChannelDirection::ServerToClient);

    if app.is_server() {
        app.add_systems(
            FixedUpdate,
            (update_visibility, update_team_visibility)
                .chain()
                .run_if(in_state(GameState::InGame)),
        );

        app.add_observer(on_sight_removed);
    } else {
        app.add_systems(OnEnter(GameState::InGame), setup_fow);

        app.add_systems(
            Update,
            change_visibility.run_if(in_state(GameState::InGame)),
        );
        // app.add_systems(
        //     Update,
        //     (render_fog_of_war, listen_for_fog_of_war_image)
        //         .chain()
        //         .run_if(in_state(GameState::InGame).and(resource_exists::<FogOfWarTexture>)),
        // );
        // app.insert_resource(FogOfWarBuffer(None));

        // app.add_systems(OnEnter(GameState::InGame), |mut commands: Commands| {
        //     let (sender1, receiver1) = mpmc::channel();
        //     let (sender2, receiver2) = mpmc::channel();

        //     // TODO: stop thread and remove resources on exit game state
        //     std::thread::spawn(|| work_thread(receiver1, sender2));

        //     commands.insert_resource(FogOfWarRenderSender(sender1));
        //     commands.insert_resource(FogOfWarRenderReceiver(receiver2));
        //     commands.insert_resource(FogOfWarThreadWorking(false));
        // });

        app.add_systems(
            Update,
            (render_fog_of_war, move_fow_mesh).chain().run_if(in_state(GameState::InGame)),
        );
    }
}

#[derive(Debug, Component, Clone, PartialEq, Serialize, Deserialize)]
pub struct SightRange(pub f32);

#[derive(Default, Debug, Component, Clone, PartialEq, Serialize, Deserialize)]
pub struct VisibleByEntities(pub HashSet<Entity>);

#[derive(Default, Debug, Component, Clone, PartialEq, Serialize, Deserialize)]
#[require(VisibleByEntities)]
pub struct VisibleBy(pub HashSet<Team>);

fn on_sight_removed(trigger: Trigger<OnRemove, SightRange>, q: Query<&mut VisibleByEntities>) {
    for mut vis in q {
        if vis.bypass_change_detection().0.remove(&trigger.target()) {
            vis.set_changed();
        }
    }
}

pub fn update_visibility(
    mut q: Query<(
        Entity,
        Ref<Position>,
        &Team,
        Option<&SightRange>,
        &mut VisibleByEntities,
    )>,
    terrain: Res<Terrain>,
) {
    let mut iter = q.iter_combinations_mut();
    while let Some(
        [
            (a, a_pos, a_team, a_range, mut a_vis_es),
            (b, b_pos, b_team, b_range, mut b_vis_es),
        ],
    ) = iter.fetch_next()
    {
        if (!a_pos.is_changed() && !b_pos.is_changed() && !terrain.is_changed())
            || (a_team == b_team)
        {
            continue;
        }

        let a_range = a_range.map(|x| x.0).unwrap_or(0.0);
        let b_range = b_range.map(|x| x.0).unwrap_or(0.0);

        // Whether the units are in sight range
        let a_reaches = a_pos.distance_squared(**b_pos) <= a_range.squared();
        let b_reaches = b_pos.distance_squared(**a_pos) <= b_range.squared();

        // If there is a free sightline between them
        // If neither part is in range of the other, don't actually do the check,
        // as they can't see each other either way
        let can_see = (a_reaches || b_reaches)
            && can_see(&terrain, **a_pos, **b_pos);

        // Whether the specific part sees the other
        let a_sees = a_reaches && can_see;
        let b_sees = b_reaches && can_see;

        if a_sees {
            // b_vis_es are the entities that can see b
            if b_vis_es.bypass_change_detection().0.insert(a) {
                b_vis_es.set_changed();
            }
        } else {
            if b_vis_es.bypass_change_detection().0.remove(&a) {
                b_vis_es.set_changed();
            }
        }

        if b_sees {
            if a_vis_es.bypass_change_detection().0.insert(b) {
                a_vis_es.set_changed();
            }
        } else {
            if a_vis_es.bypass_change_detection().0.remove(&b) {
                a_vis_es.set_changed();
            }
        }
    }
}

fn can_see(terrain: &Terrain, a: Vec2, b: Vec2) -> bool {
    // Convert to world space coords, as these are what the terrain are in
    let a = vec2(a.x, -a.y);
    let b = vec2(b.x, -b.y);

    let line = Segment2d::new(a, b);

    for object in &terrain.objects {
        if aabb_collide(object.aabb, line) {
            for [a_vert, b_vert] in
                object
                    .vertices
                    .array_windows::<2>()
                    .copied()
                    .chain(std::iter::once([
                        *object.vertices.first().unwrap(),
                        *object.vertices.last().unwrap(),
                    ]))
            {
                let segment = Segment2d::new(a_vert, b_vert);

                if segments_collide(line, segment) {
                    return false;
                }
            }
        }
    }

    true
}

fn update_team_visibility(
    q: Query<(&mut VisibleByEntities, &mut VisibleBy), Changed<VisibleByEntities>>,
    q2: Query<&Team>,
) {
    for (mut vis_by_e, mut vis_by) in q {
        vis_by.0.clear();
        let mut to_remove = vec![];
        for e in &vis_by_e.0 {
            match q2.get(*e) {
                Ok(team) => {
                    vis_by.0.insert(*team);
                }
                Err(_) => {
                    // Entity was prob despawned
                    info!("ENTITY DESPAWNED; REMOVE FROM VISIBLE LIST");
                    to_remove.push(*e);
                }
            }
        }
        for e in to_remove {
            vis_by_e.0.remove(&e);
        }
    }
}

fn aabb_collide(aabb: Aabb2d, segment: Segment2d) -> bool {
    if segment.aabb_2d(Vec2::ZERO).intersects(&aabb) {
        true
    } else {
        // let top = Segment2d::new(aabb.min, vec2(aabb.max.x, aabb.min.y));
        // let left = Segment2d::new(aabb.min, vec2(aabb.min.x, aabb.max.y));
        // let right = Segment2d::new(aabb.max, vec2(aabb.max.x, aabb.min.y));
        // let bot = Segment2d::new(aabb.max, vec2(aabb.min.x, aabb.max.y));
        // segments_collide(top, segment)
        //     || segments_collide(left, segment)
        //     || segments_collide(right, segment)
        //     || segments_collide(bot, segment)
        false
    }
}

fn segments_collide(a: Segment2d, b: Segment2d) -> bool {
    let (t, u) = segments_intersection(a, b);

    (0.0..=1.0).contains(&t) && (0.0..=1.0).contains(&u)
}

fn segments_intersection(a: Segment2d, b: Segment2d) -> (f32, f32) {
    let p1 = a.point1();
    let p2 = a.point2();
    let p3 = b.point1();
    let p4 = b.point2();

    let x1 = p1.x;
    let y1 = p1.y;
    let x2 = p2.x;
    let y2 = p2.y;
    let x3 = p3.x;
    let y3 = p3.y;
    let x4 = p4.x;
    let y4 = p4.y;

    let t = ((x1 - x3) * (y3 - y4) - (y1 - y3) * (x3 - x4))
        / ((x1 - x2) * (y3 - y4) - (y1 - y2) * (x3 - x4));

    let u = -((x1 - x2) * (y1 - y3) - (y1 - y2) * (x1 - x3))
        / ((x1 - x2) * (y3 - y4) - (y1 - y2) * (x3 - x4));

    (t, u)
}

// fn segments_collide2(s1: Segment2d, s2: Segment2d) -> bool {
//     fn ccw(a: Vec2, b: Vec2, c: Vec2) -> bool {
//         (c.y - a.y) * (b.x - a.x) > (b.y - a.y) * (c.x - a.x)
//     }

//     let a = s1.point1();
//     let b = s1.point2();
//     let c = s2.point1();
//     let d = s2.point2();

//     ccw(a, c, d) != ccw(b, c, d) && ccw(a, b, c) != ccw(a, b, d)
// }

pub fn change_visibility(q: Query<(&Team, &VisibleBy, &mut Visibility)>, me: Res<MyTeam>) {
    for (team, visible, mut visibility) in q {
        if *team == me.0 || visible.0.contains(&me.0) {
            *visibility = Visibility::Inherited;
        } else {
            *visibility = Visibility::Hidden;
        }
    }
}

// fn render_fog_of_war(
//     q: Query<(&Transform, &Team, &SightRange)>,
//     fow: Res<FogOfWarTexture>,
//     assets: Res<Assets<Image>>,
//     mut buffer: ResMut<FogOfWarBuffer>,
//     terrain: Res<Terrain>,
//     my_team: Res<MyTeam>,
//     sender: Res<FogOfWarRenderSender>,
//     mut fow_working: ResMut<FogOfWarThreadWorking>,
//     x: SystemChangeTick,
//     mut last_started: Local<Tick>,
// ) {
//     // How should we render fog of war? Actual fog? :)

//     if !assets.contains(&fow.0) {
//         return;
//     }

//     // Check if the working thread is currently working
//     if fow_working.0 {
//         return;
//     }

//     fow_working.0 = true;

//     // Check if the terrain has started since last we sent a terrain
//     let terrain = if terrain
//         .last_changed()
//         .is_newer_than(*last_started, x.this_run())
//     {
//         *last_started = x.this_run();
//         Some((*terrain).clone())
//     } else {
//         None
//     };

//     // We will write to the image buffer on another thread
//     let image = buffer
//         .0
//         .take()
//         .unwrap_or_else(|| assets.get(&fow.0).unwrap().clone());

//     // We collect the entities we use for fog of war rendering
//     let entities = q
//         .into_iter()
//         .filter_map(|(trans, team, range)| {
//             (*team == my_team.0).then_some((trans.translation.xz(), range.0))
//         })
//         .collect();

//     sender.0.send((entities, terrain, image)).unwrap();
// }

// fn listen_for_fog_of_war_image(
//     res: Res<FogOfWarRenderReceiver>,
//     fow: Res<FogOfWarTexture>,
//     mut assets: ResMut<Assets<Image>>,
//     mut buffer: ResMut<FogOfWarBuffer>,
//     mut working: ResMut<FogOfWarThreadWorking>,
// ) {
//     match res.0.try_recv() {
//         Ok(mut image) => {
//             std::mem::swap(assets.get_mut(&fow.0).unwrap(), &mut image);
//             buffer.0 = Some(image);
//             working.0 = false;
//         }
//         Err(TryRecvError::Empty) => {}
//         Err(e) => Err(e).unwrap(),
//     }
// }

// #[derive(Resource)]
// struct FogOfWarRenderSender(mpmc::Sender<(Vec<(Vec2, f32)>, Option<Terrain>, Image)>);
// #[derive(Resource)]
// struct FogOfWarRenderReceiver(mpmc::Receiver<Image>);
// #[derive(Resource)]
// struct FogOfWarThreadWorking(bool);

// fn work_thread(
//     receiver: mpmc::Receiver<(Vec<(Vec2, f32)>, Option<Terrain>, Image)>,
//     sender: mpmc::Sender<Image>,
// ) {
//     let mut cached_terrain = None;

//     while let Ok((entities, terrain, mut image)) = receiver.recv() {
//         if let Some(terrain) = terrain {
//             cached_terrain = Some(terrain);
//         };

//         let terrain = cached_terrain.as_ref().unwrap();

//         let w = image.width();
//         let h = image.height();

//         if let Some(data) = image.data.as_mut() {
//             data.fill(0xFF);
//         }

//         for (pos, range) in entities {
//             let min_x = (pos.x - range).max(0.0) as u32;
//             let max_x = (pos.x + range).min(w as f32) as u32;
//             let min_y = (pos.y - range).max(0.0) as u32;
//             let max_y = (pos.y + range).min(h as f32) as u32;

//             for tex_x in 0..w {
//                 let x = (tex_x as f32 / w as f32) * 100.0 - 50.0;
//                 for tex_y in 0..h {
//                     let y = (tex_y as f32 / h as f32) * 100.0 - 50.0;

//                     let point = vec2(x, y);

//                     if pos.distance_squared(point) < range * range && can_see(&terrain, pos, point)
//                     {
//                         // Set texture clear
//                         // info!("Setting pixel clear: {}, {}", tex_x, tex_y);
//                         image.pixel_bytes_mut(uvec3(tex_x, tex_y, 0)).unwrap()[0] = 0x00;
//                         // image.set_color_at_3d(tex_x, tex_y, 0, Color).unwrap();
//                     }
//                 }
//             }
//         }

//         sender.send(image).unwrap();
//     }
// }

// #[derive(Resource)]
// pub struct FogOfWarBuffer(Option<Image>);

// #[derive(Resource)]
// pub struct FogOfWarTexture(pub Handle<Image>);

// impl FogOfWarTexture {
//     pub fn new(size: Vec2, asset_server: &AssetServer) -> Self {
//         let size1 = Extent3d {
//             width: 128,
//             height: 128,
//             depth_or_array_layers: 1,
//         };
//         let image = Image::new_fill(
//             size1,
//             TextureDimension::D3,
//             &[0xFF],
//             TextureFormat::R8Unorm,
//             RenderAssetUsages::all(),
//         );
//         let handle = asset_server.add(image);
//         Self(handle)
//     }
// }

fn setup_fow(asset_server: Res<AssetServer>, mut commands: Commands) {
    let mut fow_texture = Image::new_fill(
        Extent3d {
            width: 1024,
            height: 1024,
            depth_or_array_layers: 1,
        },
        TextureDimension::D2,
        &[0xFF; 32],
        TextureFormat::Rgba16Unorm,
        RenderAssetUsages::default(),
    );
    fow_texture.texture_descriptor.usage =
        TextureUsages::TEXTURE_BINDING | TextureUsages::COPY_DST | TextureUsages::RENDER_ATTACHMENT;

    let image = asset_server.add(fow_texture);

    commands.spawn((
        StateScoped(GameState::InGame),
        Camera {
            target: RenderTarget::Image(image.clone().into()),
            order: 1,
            clear_color: ClearColorConfig::Custom(Color::BLACK),
            ..default()
        },
        Camera3d::default(),
        Projection::Orthographic(OrthographicProjection {
            scaling_mode: bevy::render::camera::ScalingMode::Fixed {
                width: 100.0,
                height: 100.0,
            },
            scale: 1.0,
            ..OrthographicProjection::default_3d()
        }),
        Transform::from_xyz(0.0, 1.0, 0.0).looking_to(-Vec3::Y, -Vec3::Z),
        RenderLayers::layer(1),
    ));

    let square = Plane3d::new(Vec3::Y, Vec2::splat(50.0));
    let mat = StandardMaterial {
        base_color: Color::srgb(2.0, 2.0, 2.0).with_alpha(0.5),
        base_color_texture: Some(image),
        unlit: true,
        alpha_mode: AlphaMode::Multiply,
        ..default()
    };
    commands.spawn((
        StateScoped(GameState::InGame),
        Transform::from_xyz(0.0, 0.01, 0.0),
        Mesh3d(asset_server.add(square.into())),
        MeshMaterial3d(asset_server.add(mat)),
        NotShadowCaster,
        NotShadowReceiver,
    ));
}

#[derive(Component)]
#[relationship(relationship_target = FoWMeshes)]
struct FoWMeshOf(Entity);
#[derive(Component)]
#[relationship_target(relationship = FoWMeshOf, linked_spawn)]
struct FoWMeshes(Entity);

fn render_fog_of_war(
    mut buf: Local<Vec<&TerrainObject>>,
    q: Query<(Entity, &Position, &Team, &SightRange, Option<&FoWMeshes>)>,
    mut fow: Query<&mut Mesh3d>,
    terrain: Res<Terrain>,
    my_team: Res<MyTeam>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut gizmos: Gizmos,
    mut commands: Commands,
) {
    let terrain = &*terrain;
    for (e, pos, team, range, fow_meshes) in q {
        if *team != my_team.0 {
            continue;
        }

        let pos = vec2(pos.x, -pos.y);

        // Get all terrain objects in range

        terrain
            .objects
            .iter()
            .filter(|obj| obj.aabb.intersects(&BoundingCircle::new(pos, range.0)))
            .map(|x| {
                // SAFETY: The vector is cleared before the function exits, removing all static references
                unsafe { std::mem::transmute::<&TerrainObject, &TerrainObject>(x) }
            })
            .collect_into(&mut *buf);

        let objs_in_range = &*buf;

        let stop_count = 32;
        let mut stops = (0..stop_count)
            .map(|s| {
                let a = s as f32 / stop_count as f32;
                (C(a * f32::consts::TAU - f32::consts::PI), 1.0, 1.0)
            })
            .collect::<Vec<_>>();

        // gizmos.circle(
        //     Isometry3d::new(
        //         pos.extend(0.14).xzy(),
        //         Quat::from_axis_angle(Vec3::X, f32::consts::PI / 2.0),
        //     ),
        //     range.0,
        //     palettes::tailwind::BLUE_500,
        // );

        for object in objs_in_range {
            if object.aabb.intersects(&BoundingCircle::new(pos, range.0)) {
                // Could intersect
                for [prev, cur, next] in object
                    .vertices
                    .array_windows::<3>()
                    .chain([
                        &[
                            object.vertices[object.vertices.len() - 2],
                            object.vertices[object.vertices.len() - 1],
                            object.vertices[0],
                        ],
                        &[
                            object.vertices[object.vertices.len() - 1],
                            object.vertices[0],
                            object.vertices[1],
                        ],
                    ])
                    .copied()
                {
                    if pos.distance_squared(cur) <= range.0 * range.0 {
                        // Vertex is inside sight range

                        // Is this vertex facing us?
                        let to_prev = prev - pos;
                        let to_cur = cur - pos;
                        let to_next = next - pos;

                        let angle_prev = to_prev.angle_to(to_cur);
                        let angle_next = to_cur.angle_to(to_next);

                        // We check if prev and next are in range
                        // If they aren't, we add stops where they leave range

                        fn get_edge_leave_range(
                            range: &SightRange,
                            pos: Vec2,
                            stops: &mut Vec<(C, f32, f32)>,
                            prev: Vec2,
                            cur: Vec2,
                            gizmos: &mut Gizmos,
                        ) {
                            let a = cur;
                            let b = prev;
                            let c = pos;
                            let ab = b - a;
                            let ac = c - a;
                            let ad = ac.project_onto(ab);
                            let d = ad + a;
                            let l = d.distance(c);
                            let r = range.0;
                            let x = (r.squared() - l.squared()).sqrt() + 0.001;
                            // let t = ad.length() / ab.length();
                            // let t1 = t + x / ab.length();
                            // let t2 = t - x / ab.length();

                            // let mut line = |a: Vec2, b: Vec2, color: Srgba| {
                            //     gizmos.line(a.extend(0.15).xzy(), b.extend(0.15).xzy(), color);
                            // };

                            // let green = palettes::tailwind::GREEN_500;
                            // let red = palettes::tailwind::RED_500;
                            // let yellow = palettes::tailwind::YELLOW_500;
                            // let blue = palettes::tailwind::BLUE_500;

                            // // line(a, b, red);
                            // line(a, c, yellow);
                            // line(c, d, green);

                            // line(d, d + ab.normalize() * x, blue);

                            stops.push((C((d + ab.normalize() * x - pos).to_angle()), 1.0, 1.0));
                        }
                        if to_prev.length() > range.0 {
                            // Where does the edge leave range?
                            get_edge_leave_range(range, pos, &mut stops, prev, cur, &mut gizmos);
                        }
                        if to_next.length() > range.0 {
                            // Where does the edge leave range?
                            get_edge_leave_range(range, pos, &mut stops, next, cur, &mut gizmos);
                        }

                        // if angle_prev >= 0.0 || angle_next >= 0.0 {
                        //     // Facing us!
                        //     gizmos.line(
                        //         pos.extend(0.5).xzy(),
                        //         cur.extend(0.5).xzy(),
                        //         palettes::tailwind::BLUE_500,
                        //     );
                        // } else {

                        //     gizmos.line(
                        //         pos.extend(0.5).xzy(),
                        //         cur.extend(0.5).xzy(),
                        //         palettes::tailwind::RED_500,
                        //     );
                        // }

                        let (max_from, max_to) = if angle_next.signum() != angle_prev.signum() {
                            if angle_prev > 0.0 {
                                // limit max_range_from
                                (pos.distance(cur) / range.0, 1.0)
                            } else {
                                // limit max_range_to
                                (1.0, pos.distance(cur) / range.0)
                            }
                        } else {
                            // limit both
                            (pos.distance(cur) / range.0, pos.distance(cur) / range.0)
                        };

                        stops.push((C(to_cur.to_angle()), max_from, max_to));
                    }
                }
            }
        }

        // We have our lines
        stops.sort_by_key(|x| x.0);

        let mut dists: Vec<(C, f32, bool)> = vec![];

        for (stop, max_from, max_to) in stops {
            let dir = Vec2::from_angle(stop.0);

            let line = Segment2d::new(pos, pos + dir * range.0);

            let mut closest_from = 1.0;
            let mut closest_from_is_wall = false;
            let mut closest_to = 1.0;
            let mut closest_to_is_wall = false;

            for object in objs_in_range {
                if aabb_collide(object.aabb, line) {
                    for (i, [a_vert, b_vert]) in object
                        .vertices
                        .array_windows::<2>()
                        .copied()
                        .chain(std::iter::once([
                            *object.vertices.last().unwrap(),
                            *object.vertices.first().unwrap(),
                        ]))
                        .enumerate()
                    {
                        let segment = Segment2d::new(a_vert, b_vert);

                        let (t, u) = segments_intersection(line, segment);

                        if t >= -0.0001 && t <= 1.0001 && u >= -0.0001 && u <= 1.0001 {
                            // Check if we are at the edge of a segment
                            let x = if u.abs() <= 0.00015 {
                                // We are at a_vert, index i

                                Some((
                                    if i == 0 {
                                        object.vertices.len() - 1
                                    } else {
                                        i - 1
                                    },
                                    i,
                                    (i + 1) % object.vertices.len(),
                                ))
                            } else if (u - 1.0).abs() <= 0.00015 {
                                // We are at b_vert, index i + 1

                                Some((
                                    i,
                                    (i + 1) % object.vertices.len(),
                                    (i + 2) % object.vertices.len(),
                                ))
                            } else {
                                None
                            };

                            if let Some((prev_i, cur_i, next_i)) = x {
                                let prev = object.vertices[prev_i];
                                let cur = object.vertices[cur_i];
                                let next = object.vertices[next_i];

                                let to_prev = prev - pos;
                                let to_cur = cur - pos;
                                let to_next = next - pos;

                                let angle_a = to_prev.angle_to(to_cur);
                                let angle_b = to_cur.angle_to(to_next);

                                if angle_a.signum() != angle_b.signum() {
                                    // This is a corner from our PoV; we should split this ray into two
                                    if angle_a >= 0.0 && closest_from > t {
                                        closest_from = t;
                                        closest_from_is_wall = false;
                                    }
                                    if angle_a <= 0.0 && closest_to > t {
                                        closest_to = t;
                                        closest_to_is_wall = false;
                                    }
                                } else {
                                    // This is not a corner from our PoV
                                    if closest_from > t {
                                        closest_from = t;
                                        closest_from_is_wall = false;
                                    }
                                    if closest_to > t {
                                        closest_to = t;
                                        closest_to_is_wall = false;
                                    }
                                }
                            } else {
                                let should_pretend_not_wall = t >= 0.995;

                                if closest_from > t {
                                    closest_from = t;
                                    closest_from_is_wall = !should_pretend_not_wall;
                                }
                                if closest_to > t {
                                    closest_to = t;
                                    closest_to_is_wall = !should_pretend_not_wall;
                                }
                            }
                        }
                    }
                }
            }
            if !closest_from_is_wall || !closest_to_is_wall {
                dists.push((stop, closest_from, closest_from_is_wall));
                dists.push((stop, closest_to, closest_to_is_wall));
            }
        }

        let mut mesh = Mesh::new(
            bevy::render::mesh::PrimitiveTopology::TriangleList,
            RenderAssetUsages::all(),
        );

        let mut vertices = vec![];

        if !dists.is_empty() {
            let f = dists.remove(0);
            dists.push(f);

            for [(a_stop, a_len, a_is_wall), (b_stop, b_len, b_is_wall)] in dists
                .array_windows()
                .copied()
                .chain([[dists[dists.len() - 1], dists[0]]])
            {
                let a_dir = Vec2::from_angle(a_stop.0);
                let b_dir = Vec2::from_angle(b_stop.0);

                vertices.push(Vec3::ZERO);
                vertices.push((b_dir * b_len * range.0).extend(0.0).xzy());
                vertices.push((a_dir * a_len * range.0).extend(0.0).xzy());

                // gizmos.line(
                //     pos.extend(0.15).xzy(),
                //     (pos + a_dir * a_len * range.0).extend(0.15).xzy(),
                //     if a_is_wall {
                //         palettes::tailwind::YELLOW_500
                //     } else {
                //         palettes::tailwind::BLUE_500
                //     },
                // );

                // gizmos.line(
                //     pos.extend(0.15).xzy(),
                //     (pos + b_dir * b_len * range.0)
                //         .extend(0.15)
                //         .xzy(),
                //     if b_is_wall { palettes::tailwind::YELLOW_500 } else { palettes::tailwind::BLUE_500 },
                // );

                // gizmos.line(
                //     (pos + a_dir * a_len * range.0).extend(0.155).xzy(),
                //     (pos + b_dir * b_len * range.0).extend(0.155).xzy(),
                //     palettes::tailwind::RED_500,
                // );

                // gizmos.linestrip(
                //     [
                //         pos,
                //         pos + a_dir * a_to * range.0,
                //         pos + b_dir * b_from * range.0,
                //         pos,
                //     ]
                //     .into_iter()
                //     .map(|p| p.extend(0.15).xzy()),
                //     palettes::tailwind::BLUE_500,
                // );
            }
        }

        mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, vertices);

        let mesh = meshes.add(mesh);

        if let Some(fows) = fow_meshes {
            assert_eq!(fows.len(), 1);
            let mut mesh3d = fow.get_mut(fows.0).unwrap();
            mesh3d.0 = mesh;
        } else {
            let mat = materials.add(StandardMaterial {
                base_color: Color::srgb(2.0, 2.0, 2.0),
                unlit: true,
                ..default()
            });
            commands.spawn((
                Mesh3d(mesh),
                MeshMaterial3d(mat),
                FoWMeshOf(e),
                Transform::from_translation(vec3(pos.x, 0.0, -pos.y)),
                RenderLayers::layer(1),
            ));
        }

        // IMPORTANT: needed to avoid UB
        buf.clear();
    }
}

fn move_fow_mesh(q: Query<(&mut Transform, &FoWMeshOf)>, t: Query<&Position, Without<FoWMeshOf>>) {
    for (mut trans, fow_mesh_of) in q {
        let par_pos = t.get(fow_mesh_of.0).unwrap();
        trans.translation = par_pos.into();
    }
}
