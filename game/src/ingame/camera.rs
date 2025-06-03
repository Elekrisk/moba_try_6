use bevy::{
    prelude::*,
    window::PrimaryWindow,
};
use bevy_enhanced_input::prelude::{ActionState, *};

use crate::client_setup;

pub fn client(app: &mut App) {
    app.init_resource::<MousePos>()
        .add_input_context::<CameraInput>()
        .add_systems(Startup, setup_camera.after(client_setup))
        .add_systems(
            Update,
            (
                update_mouse_pos,
                move_camera_on_mouse_edges,
                drag_camera,
                reset_camera,
            )
                .chain(),
        );
}

fn setup_camera(camera: Single<Entity, With<Camera>>, mut commands: Commands) {
    commands
        .spawn((CameraFocus, Transform::from_xyz(0.0, 0.0, 0.0)))
        .add_child(*camera);
    commands.run_system_cached(lock_mouse);

    let mut actions = Actions::<CameraInput>::default();
    actions.bind::<CameraDrag>().to(MouseButton::Middle);
    actions.bind::<CameraZoom>().to(Input::mouse_wheel());
    commands.spawn(actions).observe(zoom_camera);
}

fn lock_mouse(_window: Single<&mut Window, With<PrimaryWindow>>) {
//     info!("Locking cursor");
//     window.cursor_options.grab_mode = CursorGrabMode::Confined;
}

fn move_camera_on_mouse_edges(
    window: Single<&Window, With<PrimaryWindow>>,
    mut camera_focus: Single<&mut Transform, With<CameraFocus>>,
    time: Res<Time>,
) {
    let Some(mouse_pos) = window.cursor_position() else {
        return;
    };

    let edge_margin = 1.0;

    let size = window.size();

    let left = mouse_pos.x <= edge_margin;
    let right = mouse_pos.x >= size.x - edge_margin;
    let top = mouse_pos.y <= edge_margin;
    let bottom = mouse_pos.y >= size.y - edge_margin;

    let horizontal = right as isize - left as isize;
    let vertical = bottom as isize - top as isize;

    let shift = Vec2::new(horizontal as f32, vertical as f32);

    if shift != Vec2::ZERO {
        camera_focus.translation.x += shift.x * time.delta_secs() * 10.0;
        camera_focus.translation.z += shift.y * time.delta_secs() * 10.0;
    }
}

fn reset_camera(
    input: Res<ButtonInput<KeyCode>>,
    mut focus: Single<&mut Transform, With<CameraFocus>>,
) {
    if input.pressed(KeyCode::Space) {
        focus.translation = Vec3::ZERO;
    }
}

#[derive(Component)]
pub struct CameraFocus;

#[derive(Default, Resource)]
pub struct MousePos {
    pub plane_pos: Vec2,
    pub world_pos: Vec3,
}

fn update_mouse_pos(
    window: Single<&Window, With<PrimaryWindow>>,
    camera: Single<(&Camera, &GlobalTransform)>,
    mut mouse_pos: ResMut<MousePos>,
) {
    let Some(pos) = window.cursor_position() else {
        return;
    };

    let (camera, camera_pos) = *camera;

    let ray = camera.viewport_to_world(camera_pos, pos).unwrap();
    if let Some(pos) = ray.intersect_plane(Vec3::ZERO, InfinitePlane3d::new(Vec3::Y)) {
        let pos = ray.get_point(pos);
        mouse_pos.plane_pos = pos.xz();
        mouse_pos.world_pos = pos;
    }
}

#[derive(Debug, InputContext)]
struct CameraInput;

#[derive(Debug, InputAction)]
#[input_action(output = bool)]
struct CameraDrag;

#[derive(Debug, InputAction)]
#[input_action(output = Vec2)]
struct CameraZoom;

fn drag_camera(
    input: Single<&Actions<CameraInput>>,
    mut start_drag_pos: Local<Option<Vec2>>,
    mouse_pos: Res<MousePos>,
    mut focus: Single<&mut Transform, With<CameraFocus>>,
) {
    match input.action::<CameraDrag>().state() {
        ActionState::None => *start_drag_pos = None,
        ActionState::Fired => {
            if start_drag_pos.is_none() {
                *start_drag_pos = Some(mouse_pos.plane_pos);
            }
            let start = start_drag_pos.unwrap();

            let diff = mouse_pos.plane_pos - start;

            focus.translation.x -= diff.x;
            focus.translation.z -= diff.y;
        }
        _ => {}
    }
}

fn zoom_camera(
    trigger: Trigger<Fired<CameraZoom>>,
    mut camera: Single<&mut Transform, With<Camera>>,
) {
    let x = trigger.event().value;
    let change = 1.0 - x.y * 0.2;
    camera.translation *= change;
}
