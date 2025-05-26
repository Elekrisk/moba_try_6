use bevy::{input::mouse::MouseScrollUnit, prelude::*};

pub fn client(app: &mut App) {}

pub fn scroll(trigger: Trigger<Pointer<Scroll>>, mut q: Query<&mut ScrollPosition>) {
    let Ok(mut pos) = q.get_mut(trigger.target()) else {
        return;
    };
    let y = match trigger.event().unit {
        MouseScrollUnit::Line => trigger.event().y * 24.0,
        MouseScrollUnit::Pixel => trigger.event().y,
    };

    pos.offset_y -= y;

    info!("Offset: {}", pos.offset_y);
}
