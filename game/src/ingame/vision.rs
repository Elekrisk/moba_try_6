use bevy::{platform::collections::HashSet, prelude::*};
use lightyear::prelude::*;
use lobby_common::Team;
use serde::{Deserialize, Serialize};

use crate::{
    AppExt, GameState,
    ingame::{terrain::Terrain, unit::MyTeam},
};

pub fn plugin(app: &mut App) {
    app.register_component::<VisibleBy>(ChannelDirection::ServerToClient);

    if app.is_server() {
        app.add_systems(
            FixedUpdate,
            update_visibility.run_if(in_state(GameState::InGame)),
        );
    } else {
        app.add_systems(
            FixedUpdate,
            change_visibility.run_if(in_state(GameState::InGame)),
        );
    }
}

#[derive(Default, Debug, Component, Clone, PartialEq, Serialize, Deserialize)]
pub struct VisibleBy(pub HashSet<Team>);

pub fn update_visibility(mut q: Query<(&Transform, &Team, &mut VisibleBy)>, terrain: Res<Terrain>) {
    for (_, _, mut v) in q.iter_mut() {
        v.0.clear();
    }
    let mut iter = q.iter_combinations_mut();
    'outer: while let Some([mut a, mut b]) = iter.fetch_next() {
        let line = Segment2d::new(a.0.translation.xz(), b.0.translation.xz());

        for object in &terrain.objects {
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
                    continue 'outer;
                } else {
                }
            }
        }
        a.2.0.insert(*b.1);
        b.2.0.insert(*a.1);
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

pub fn change_visibility(q: Query<(&Team, &VisibleBy, &mut Visibility)>, me: Res<MyTeam>) {
    for (team, visible, mut visibility) in q {
        if *team == me.0 || visible.0.contains(&me.0) {
            *visibility = Visibility::Inherited;
        } else {
            *visibility = Visibility::Hidden;
        }
    }
}
