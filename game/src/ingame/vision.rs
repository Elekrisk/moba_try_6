use std::sync::{mpmc, mpsc::TryRecvError};

use bevy::{
    asset::RenderAssetUsages,
    ecs::{component::Tick, system::SystemChangeTick},
    image::ImageSampler,
    math::bounding::{Aabb2d, Bounded2d, IntersectsVolume},
    platform::collections::HashSet,
    prelude::*,
    render::render_resource::{Extent3d, TextureDimension, TextureFormat},
};
use lightyear::prelude::*;
use lobby_common::Team;
use serde::{Deserialize, Serialize};

use crate::{
    AppExt, GameState,
    ingame::{terrain::Terrain, unit::MyTeam},
};

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
    } else {
        app.add_systems(
            FixedUpdate,
            change_visibility.run_if(in_state(GameState::InGame)),
        );
        app.add_systems(
            Update,
            (render_fog_of_war, listen_for_fog_of_war_image)
                .chain()
                .run_if(in_state(GameState::InGame).and(resource_exists::<FogOfWarTexture>)),
        );
        app.insert_resource(FogOfWarBuffer(None));

        app.add_systems(OnEnter(GameState::InGame), |mut commands: Commands| {
            let (sender1, receiver1) = mpmc::channel();
            let (sender2, receiver2) = mpmc::channel();

            // TODO: stop thread and remove resources on exit game state
            std::thread::spawn(|| work_thread(receiver1, sender2));

            commands.insert_resource(FogOfWarRenderSender(sender1));
            commands.insert_resource(FogOfWarRenderReceiver(receiver2));
            commands.insert_resource(FogOfWarThreadWorking(false));
        });
    }
}

#[derive(Debug, Component, Clone, PartialEq, Serialize, Deserialize)]
pub struct SightRange(pub f32);

#[derive(Default, Debug, Component, Clone, PartialEq, Serialize, Deserialize)]
pub struct VisibleByEntities(pub HashSet<Entity>);

#[derive(Default, Debug, Component, Clone, PartialEq, Serialize, Deserialize)]
#[require(VisibleByEntities)]
pub struct VisibleBy(pub HashSet<Team>);

pub fn update_visibility(
    mut q: Query<(
        Entity,
        Ref<Transform>,
        &Team,
        &SightRange,
        &mut VisibleByEntities,
    )>,
    terrain: Res<Terrain>,
) {
    let mut iter = q.iter_combinations_mut();
    while let Some(
        [
            (a, a_trans, a_team, a_range, mut a_vis_es),
            (b, b_trans, b_team, b_range, mut b_vis_es),
        ],
    ) = iter.fetch_next()
    {
        if (!a_trans.is_changed() && !b_trans.is_changed() && !terrain.is_changed())
            || (a_team == b_team)
        {
            continue;
        }

        let a_pos = a_trans.translation.xz();
        let b_pos = b_trans.translation.xz();

        // Whether the units are in sight range
        let a_reaches = a_pos.distance_squared(b_pos) <= a_range.0 * a_range.0;
        let b_reaches = b_pos.distance_squared(a_pos) <= b_range.0 * b_range.0;

        // If there is a free sightline between them
        // If neither part is in range of the other, don't actually do the check,
        // as they can't see each other either way
        let can_see = (a_reaches || b_reaches)
            && can_see(&terrain, a_trans.translation.xz(), b_trans.translation.xz());

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
    q: Query<(&VisibleByEntities, &mut VisibleBy), Changed<VisibleByEntities>>,
    q2: Query<&Team>,
) {
    for (vis_by_e, mut vis_by) in q {
        vis_by.0.clear();
        for e in &vis_by_e.0 {
            vis_by.0.insert(*q2.get(*e).unwrap());
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

    0.0 <= t && t <= 1.0 && 0.0 <= u && u <= 1.0
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

fn render_fog_of_war(
    q: Query<(&Transform, &Team, &SightRange)>,
    fow: Res<FogOfWarTexture>,
    assets: Res<Assets<Image>>,
    mut buffer: ResMut<FogOfWarBuffer>,
    terrain: Res<Terrain>,
    my_team: Res<MyTeam>,
    sender: Res<FogOfWarRenderSender>,
    mut fow_working: ResMut<FogOfWarThreadWorking>,
    x: SystemChangeTick,
    mut last_started: Local<Tick>,
) {
    // How should we render fog of war? Actual fog? :)

    if !assets.contains(&fow.0) {
        return;
    }

    // Check if the working thread is currently working
    if fow_working.0 {
        return;
    }

    fow_working.0 = true;

    // Check if the terrain has started since last we sent a terrain
    let terrain = if terrain
        .last_changed()
        .is_newer_than(*last_started, x.this_run())
    {
        *last_started = x.this_run();
        Some((*terrain).clone())
    } else {
        None
    };

    // We will write to the image buffer on another thread
    let image = buffer
        .0
        .take()
        .unwrap_or_else(|| assets.get(&fow.0).unwrap().clone());

    // We collect the entities we use for fog of war rendering
    let entities = q
        .into_iter()
        .filter_map(|(trans, team, range)| {
            (*team == my_team.0).then_some((trans.translation.xz(), range.0))
        })
        .collect();

    sender.0.send((entities, terrain, image)).unwrap();
}

fn listen_for_fog_of_war_image(
    res: Res<FogOfWarRenderReceiver>,
    fow: Res<FogOfWarTexture>,
    mut assets: ResMut<Assets<Image>>,
    mut buffer: ResMut<FogOfWarBuffer>,
    mut working: ResMut<FogOfWarThreadWorking>,
) {
    match res.0.try_recv() {
        Ok(mut image) => {
            std::mem::swap(assets.get_mut(&fow.0).unwrap(), &mut image);
            buffer.0 = Some(image);
            working.0 = false;
        }
        Err(TryRecvError::Empty) => {}
        Err(e) => Err(e).unwrap(),
    }
}

#[derive(Resource)]
struct FogOfWarRenderSender(mpmc::Sender<(Vec<(Vec2, f32)>, Option<Terrain>, Image)>);
#[derive(Resource)]
struct FogOfWarRenderReceiver(mpmc::Receiver<Image>);
#[derive(Resource)]
struct FogOfWarThreadWorking(bool);

fn work_thread(
    receiver: mpmc::Receiver<(Vec<(Vec2, f32)>, Option<Terrain>, Image)>,
    sender: mpmc::Sender<Image>,
) {
    let mut cached_terrain = None;

    while let Ok((entities, terrain, mut image)) = receiver.recv() {
        if let Some(terrain) = terrain {
            cached_terrain = Some(terrain);
        };

        let terrain = cached_terrain.as_ref().unwrap();

        let w = image.width();
        let h = image.height();

        if let Some(data) = image.data.as_mut() {
            data.fill(0xFF);
        }

        for (pos, range) in entities {
            let min_x = (pos.x - range).max(0.0) as u32;
            let max_x = (pos.x + range).min(w as f32) as u32;
            let min_y = (pos.y - range).max(0.0) as u32;
            let max_y = (pos.y + range).min(h as f32) as u32;

            for tex_x in 0..w {
                let x = (tex_x as f32 / w as f32) * 100.0 - 50.0;
                for tex_y in 0..h {
                    let y = (tex_y as f32 / h as f32) * 100.0 - 50.0;

                    let point = vec2(x, y);

                    if pos.distance_squared(point) < range * range && can_see(&terrain, pos, point)
                    {
                        // Set texture clear
                        // info!("Setting pixel clear: {}, {}", tex_x, tex_y);
                        image.pixel_bytes_mut(uvec3(tex_x, tex_y, 0)).unwrap()[0] = 0x00;
                        // image.set_color_at_3d(tex_x, tex_y, 0, Color).unwrap();
                    }
                }
            }
        }

        sender.send(image).unwrap();
    }
}

#[derive(Resource)]
pub struct FogOfWarBuffer(Option<Image>);

#[derive(Resource)]
pub struct FogOfWarTexture(pub Handle<Image>);

impl FogOfWarTexture {
    pub fn new(size: Vec2, asset_server: &AssetServer) -> Self {
        let size1 = Extent3d {
            width: 128,
            height: 128,
            depth_or_array_layers: 1,
        };
        let image = Image::new_fill(
            size1,
            TextureDimension::D3,
            &[0xFF],
            TextureFormat::R8Unorm,
            RenderAssetUsages::all(),
        );
        let handle = asset_server.add(image);
        Self(handle)
    }
}
