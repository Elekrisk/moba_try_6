use bevy::prelude::*;

use crate::{ingame::terrain::Terrain, AppExt, Unit};

pub fn plugin(app: &mut App) {
    if app.is_server() {
        app.add_systems(FixedUpdate, unit_collision);
    } else {
        app.add_systems(Update, unit_collision);
    }
}

const COL_DIST: f32 = 0.5;

fn unit_collision(mut q: Query<&mut Transform, With<Unit>>, terrain: Res<Terrain>) {
    let mut pairs = q.iter_combinations_mut();
    while let Some([mut a_trans, mut b_trans]) = pairs.fetch_next() {
        let a_pos = a_trans.translation.xz();
        let b_pos = b_trans.translation.xz();

        if a_pos.distance_squared(b_pos) < COL_DIST * COL_DIST {
            // Move them
            let dist = a_pos.distance(b_pos);
            let shift = (COL_DIST - dist) / 2.0;
            let new_a_pos = a_pos.move_towards(b_pos, -shift);
            let new_b_pos = b_pos.move_towards(a_pos, -shift);
            a_trans.translation = new_a_pos.extend(a_trans.translation.y).xzy();
            b_trans.translation = new_b_pos.extend(b_trans.translation.y).xzy();
        }
    }

    for trans in q {
        for obj in &terrain.objects {
            
        }
    }
}
