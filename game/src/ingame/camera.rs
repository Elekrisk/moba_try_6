use bevy::{prelude::*, window::PrimaryWindow};
use bevy_enhanced_input::prelude::{ActionState, *};

use crate::{client_setup, main_ui::lobby_list::MyPlayerId, GameState};

#[derive(Component)]
pub struct PrimaryCamera;

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
                .chain().run_if(in_state(GameState::InGame)),
        );
}

fn setup_camera(camera: Single<Entity, With<PrimaryCamera>>, mut commands: Commands) {
    commands
        .spawn((CameraFocus, Transform::from_xyz(0.0, 0.0, 0.0)))
        .add_child(*camera);
    commands.run_system_cached(lock_mouse);

    let mut actions = Actions::<CameraInput>::default();
    actions.bind::<CameraDrag>().to(MouseButton::Middle);
    actions.bind::<CameraZoom>().to(Input::mouse_wheel());
    commands.spawn(actions).observe(zoom_camera);
}

fn lock_mouse(mut window: Single<&mut Window, With<PrimaryWindow>>) {
    // info!("Locking cursor");
    // window.cursor_options.grab_mode = bevy::window::CursorGrabMode::Confined;
}

fn move_camera_on_mouse_edges(
    window: Single<&Window, With<PrimaryWindow>>,
    mut camera_focus: Single<&mut Transform, With<CameraFocus>>,
    time: Res<Time>,
) {
    let Some(mouse_pos) = window.cursor_position() else {
        return;
    };

    let edge_margin = 10.0;

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
    q: Query<(&Transform, &ControlledByClient), Without<CameraFocus>>,
    players: Res<Players>,
    my_id: Res<MyPlayerId>,
) {
    if input.pressed(KeyCode::Space) {
        // focus.translation = Vec3::ZERO;

        let my_client_id =  players.players.get(&my_id.0).unwrap().client_id;
        for (trans, control) in q {
            if control.0 == my_client_id {
                focus.translation = trans.translation;
            }
        }
    }
}

#[derive(Component)]
pub struct CameraFocus;

#[derive(Default, Resource)]
pub struct MousePos {
    pub plane_pos: Position,
    pub world_pos: Vec3,
}

fn update_mouse_pos(
    window: Single<&Window, With<PrimaryWindow>>,
    camera: Single<(&Camera, &GlobalTransform), With<PrimaryCamera>>,
    mut mouse_pos: ResMut<MousePos>,
) {
    let Some(pos) = window.cursor_position() else {
        return;
    };

    let (camera, camera_pos) = *camera;

    let ray = camera.viewport_to_world(camera_pos, pos).unwrap();
    if let Some(pos) = ray.intersect_plane(Vec3::ZERO, InfinitePlane3d::new(Vec3::Y)) {
        let pos = ray.get_point(pos);
        mouse_pos.plane_pos = pos.into();
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
    mut start_drag_pos: Local<Option<Position>>,
    mouse_pos: Res<MousePos>,
    mut focus: Single<&mut Transform, With<CameraFocus>>,
) {
    match input.state::<CameraDrag>().unwrap() {
        ActionState::None => *start_drag_pos = None,
        ActionState::Fired => {
            if start_drag_pos.is_none() {
                *start_drag_pos = Some(mouse_pos.plane_pos);
            }
            let start = start_drag_pos.unwrap();

            let diff = *mouse_pos.plane_pos - *start;

            focus.translation.x -= diff.x;
            focus.translation.z += diff.y;
        }
        _ => {}
    }
}

fn zoom_camera(
    trigger: Trigger<Fired<CameraZoom>>,
    mut camera: Single<&mut Transform, With<PrimaryCamera>>,
) {
    let x = trigger.event().value;
    let change = 1.0 - x.y * 0.2;
    camera.translation *= change;
}

use std::marker::PhantomData;

use bevy::{ecs::query::QuerySingleError, prelude::*, ui::UiSystem};

use super::{targetable::Position, unit::ControlledByClient, Players};

/// Defines where the point that is anchored is located on the height of UI node that is anchored
#[derive(Default, Reflect, Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum VerticalAnchor {
    Top,
    #[default]
    Mid,
    Bottom,
}
/// Defines where the point that is anchored is located on the width of UI node that is anchored
#[derive(Default, Reflect, Debug, Clone, Copy, PartialEq, Eq)]
pub enum HorizontalAnchor {
    Left,
    #[default]
    Mid,
    Right,
}

#[derive(Default, Reflect, Debug, Clone, Copy, PartialEq, Eq)]
/// defines where the UIs anchorpoint should be,
/// this is the point on the UI that will match the in-world location of the entity
pub struct AnchorPoint {
    /// Defines where the horizontal part of the UI tries to synchronize towards the chosen target
    horizontal: HorizontalAnchor,
    /// Defines where the vertical part of the UI tries to synchronize towards the chosen target
    vertical: VerticalAnchor,
}

impl AnchorPoint {
    pub fn topleft() -> Self {
        Self {
            horizontal: HorizontalAnchor::Left,
            vertical: VerticalAnchor::Top,
        }
    }
    pub fn topright() -> Self {
        Self {
            horizontal: HorizontalAnchor::Right,
            vertical: VerticalAnchor::Top,
        }
    }
    pub fn bottomleft() -> Self {
        Self {
            horizontal: HorizontalAnchor::Left,
            vertical: VerticalAnchor::Bottom,
        }
    }
    pub fn bottomright() -> Self {
        Self {
            horizontal: HorizontalAnchor::Right,
            vertical: VerticalAnchor::Bottom,
        }
    }
    pub fn middle() -> Self {
        Self {
            horizontal: HorizontalAnchor::Mid,
            vertical: VerticalAnchor::Mid,
        }
    }
}

/// relationship that defines which uinodes are anchored to this entity
#[derive(Component, Reflect, Clone, Debug, PartialEq)]
#[relationship_target(relationship = AnchorUiNode, linked_spawn)]
pub struct AnchoredUiNodes(Vec<Entity>);

/// Component that will continuosly update the UI location on screen, to match an in world location either chosen as a fixed
/// position, or chosen as another entities ['GlobalTransformation']
#[derive(Component, Reflect, Clone, Debug, PartialEq)]
#[relationship(relationship_target = AnchoredUiNodes)]
#[require(AnchorUiConfig, Node)]
pub struct AnchorUiNode {
    /// The Ui will be placed onto the screen, matching where this entity is located in the world
    #[relationship]
    pub target: Entity,
}

#[derive(Component, Reflect, Clone, Debug, PartialEq, Default)]
/// Configures how the UI Is anchored to the entity
pub struct AnchorUiConfig {
    /// Defines where on the UI node the anchorpoint is located
    pub anchorpoint: AnchorPoint,
    /// Offset will be calculated for the 'AnchorTarget'
    /// and the chosen anchoring of the UI element, and can be used to put UI elements away from what they are targeted to
    pub offset: Option<Vec3>,
}

impl AnchorUiConfig {
    pub fn with_offset(mut self, offset: Vec3) -> Self {
        self.offset = Some(offset);
        self
    }
    pub fn with_horizontal_anchoring(mut self, horizontal: HorizontalAnchor) -> Self {
        self.anchorpoint.horizontal = horizontal;
        self
    }
    pub fn with_vertical_anchoring(mut self, vertical: VerticalAnchor) -> Self {
        self.anchorpoint.vertical = vertical;
        self
    }
}

impl AnchorUiNode {
    /// Will anchor the midpoint of this UI element towards the chosen entity
    pub fn to_entity(entity: Entity) -> Self {
        Self { target: entity }
    }
}

pub struct AnchorUiPlugin<SingleCameraMarker: Component> {
    _component: PhantomData<SingleCameraMarker>,
}

impl<SingleCameraMarker: Component> AnchorUiPlugin<SingleCameraMarker> {
    pub fn new() -> Self {
        Self {
            _component: PhantomData::default(),
        }
    }
}

impl<SingleCameraMarker: Component> Plugin for AnchorUiPlugin<SingleCameraMarker> {
    fn build(&self, app: &mut App) {
        app.add_systems(
            PostUpdate,
            system_move_ui_nodes::<SingleCameraMarker>
                .before(TransformSystem::TransformPropagate)
                .before(UiSystem::Layout),
        );

        app.register_type::<AnchorUiNode>();
    }
}

fn system_move_ui_nodes<C: Component>(
    cameras: Query<(Entity, &Camera), With<C>>,
    window: Query<&Window, With<PrimaryWindow>>,
    mut uinodes: Query<(
        Entity,
        &mut Node,
        &mut Visibility,
        &ComputedNode,
        &AnchorUiNode,
        &AnchorUiConfig,
    )>,
    transformhelper: TransformHelper,
) {
    let window = match window.single() {
        Ok(window) => window,
        Err(QuerySingleError::NoEntities(_)) => return,
        Err(err @ QuerySingleError::MultipleEntities(_)) => {
            bevy::log::error!("more than one primary window: {err}");
            return;
        }
    };
    let (camera_entity, main_camera) = match cameras.single() {
        Ok(camera) => camera,
        Err(QuerySingleError::NoEntities(_)) => return,
        Err(err @ QuerySingleError::MultipleEntities(_)) => {
            bevy::log::error!("more than one camera with the specified marker component: {err}");
            return;
        }
    };
    let Ok(main_camera_transform) = transformhelper.compute_global_transform(camera_entity) else {
        warn!("Failed computing global transform for Camera Entity");
        return;
    };

    for (uientity, mut node, mut vis, computed_node, uinode, uianchorconf) in uinodes.iter_mut() {
        if node.display == Display::None {
            // The node is not displayed, skip it
            continue;
        }

        // what location should we sync to
        let world_location = if let Ok(gt) = transformhelper.compute_global_transform(uinode.target)
        {
            gt.translation()
        } else {
            warn!("AnchorTarget({}) failed to compute global transform, uinode: {uientity} will not be updated", uinode.target);
            continue;
        };

        let world_location = if let Some(offset) = uianchorconf.offset {
            world_location + offset
        } else {
            world_location
        };

        let Ok(position) =
            main_camera.world_to_viewport_with_depth(&main_camera_transform, world_location)
        else {
            // Object is offscreen and should not be drawn
            bevy::log::debug!("world location is offscreen, and thus we dont change the position");
            // We make it invisible
            *vis = Visibility::Hidden;
            continue;
        };
        *vis = Visibility::Inherited;

        if node.as_ref().position_type != PositionType::Absolute {
            node.position_type = PositionType::Absolute;
        }

        let nodewidth = if let Val::Px(width) = node.width {
            width
        } else {
            computed_node.size().x * computed_node.inverse_scale_factor()
        };
        let leftpos = match uianchorconf.anchorpoint.horizontal {
            HorizontalAnchor::Left => Val::Px(position.x),
            HorizontalAnchor::Mid => Val::Px(position.x - nodewidth / 2.0),
            HorizontalAnchor::Right => Val::Px(position.x - nodewidth),
        };

        // if check_if_not_close(node.as_ref().left, leftpos) {
        node.left = leftpos;
        // }

        let window_height = window.height();

        let nodeheight = if let Val::Px(height) = node.height {
            height
        } else {
            computed_node.size().y * computed_node.inverse_scale_factor()
        };

        let newheight = match uianchorconf.anchorpoint.vertical {
            VerticalAnchor::Top => Val::Px(window_height - position.y - nodeheight),
            VerticalAnchor::Mid => Val::Px(window_height - position.y - nodeheight / 2.0),
            VerticalAnchor::Bottom => Val::Px(window_height - position.y),
        };

        // if check_if_not_close(node.as_ref().bottom, newheight) {
        node.bottom = newheight;
        // }
    }
}

// // only move if the change position is more than one pixel from each other, stops vibrations
// fn check_if_not_close(a: Val, b: Val) -> bool {
//     if a == b {
//         return false;
//     }

//     match (a, b) {
//         (Val::Px(a), Val::Px(b)) => (a - b).abs() > 1.0, // If they are more than a pixel from eachother
//         _ => true,
//     }
// }