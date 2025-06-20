use crate::ingame::{
    camera::{
        AnchorPoint, AnchorUiConfig, AnchorUiPlugin, AnchoredUiNodes, HorizontalAnchor,
        VerticalAnchor,
    },
    unit::attack::AutoAttackTimer,
};
use bevy::{color::palettes, prelude::*};
use lightyear::prelude::*;
use lobby_common::Team;

use crate::{AppExt, GameState, UiCameraMarker, ingame::unit::MyTeam};

use super::unit::stats::StatBlock;

pub mod healthbar;

#[derive(Debug, Clone, Copy, PartialEq, Component, Serialize, Deserialize)]
pub struct Health(pub f32);

pub fn common(app: &mut App) {
    app.add_plugins(healthbar::plugin);

    app.register_component::<Health>(ChannelDirection::ServerToClient);
    app.register_component::<Position>(ChannelDirection::ServerToClient);
    app.register_component::<Facing>(ChannelDirection::ServerToClient);

    if app.is_client() {
        app.add_plugins(AnchorUiPlugin::<UiCameraMarker>::new());
        app.add_systems(
            Update,
            (
                (update_previous_position, interpolate_transform).chain(),
                update_facing,
            ),
        );
        app.add_observer(on_insert_position);
        // app.add_observer(spawn_floating_health_bars);
        // app.add_systems(
        //     Update,
        //     update_floating_health_bar.run_if(in_state(GameState::InGame)),
        // );
    } else {
        app.add_systems(FixedUpdate, kill_if_low_health);
    }
}

fn kill_if_low_health(
    query: Query<(Entity, &Health /*Option<&LuaObject>*/)>,
    mut commands: Commands,
) {
    for (e, hp /*obj*/) in query {
        if hp.0 <= 0.0 {
            commands.entity(e).try_despawn();
        }
    }
}

#[derive(
    Default, Debug, Component, Clone, Copy, PartialEq, Serialize, Deserialize, Deref, DerefMut,
)]
#[require(CachedPosition, PreviousPosition, PostitionLastChanged, Transform)]
#[repr(transparent)]
pub struct Position(pub Vec2);

// impl Position {
//     pub fn to_translation(self) -> Vec3 {
//         vec3(self.x, 0.0, -self.y)
//     }
// }

impl From<Vec3> for Position {
    fn from(value: Vec3) -> Self {
        Self(vec2(value.x, -value.z))
    }
}

impl From<Position> for Vec3 {
    fn from(value: Position) -> Self {
        vec3(value.x, 0.0, -value.y)
    }
}

impl From<&Position> for Vec3 {
    fn from(value: &Position) -> Self {
        vec3(value.x, 0.0, -value.y)
    }
}

#[derive(
    Default, Debug, Component, Clone, Copy, PartialEq, Serialize, Deserialize, Deref, DerefMut,
)]
pub struct CachedPosition(pub Vec2);

#[derive(
    Default, Debug, Component, Clone, Copy, PartialEq, Serialize, Deserialize, Deref, DerefMut,
)]
pub struct PreviousPosition(pub Vec2);

#[derive(Default, Debug, Component, Clone, Copy)]
pub struct PostitionLastChanged(f32);

fn interpolate_transform(
    q: Query<(
        Ref<Position>,
        &PreviousPosition,
        &mut PostitionLastChanged,
        &mut Transform,
    )>,
    time: Res<Time>,
) {
    for (pos, prev, mut last_changed, mut trans) in q {
        if pos.is_changed() {
            last_changed.0 = time.elapsed_secs();
        }

        let t = ((time.elapsed_secs() - last_changed.0) * 20.0).clamp(0.0, 1.0);

        trans.translation = Vec3::from(Position(prev.lerp(**pos, t))).with_y(trans.translation.y);
    }
}

fn update_previous_position(
    q: Query<(&Position, &mut PreviousPosition, &mut CachedPosition), Changed<Position>>,
) {
    for (pos, mut prev, mut cache) in q {
        prev.0 = cache.0;
        cache.0 = pos.0;
    }
}

fn on_insert_position(
    trigger: Trigger<OnAdd, Position>,
    mut q: Query<(&Position, &mut PreviousPosition, &mut CachedPosition, &mut Transform)>,
) {
    let (pos, mut prev, mut cache, mut trans) = q.get_mut(trigger.target()).unwrap();
    prev.0 = pos.0;
    cache.0 = pos.0;
    trans.translation = pos.into();
}

#[derive(
    Default, Debug, Component, Clone, Copy, PartialEq, Serialize, Deserialize, Deref, DerefMut,
)]
#[require(CachedPosition, PreviousPosition, PostitionLastChanged, Transform)]
#[repr(transparent)]
pub struct Facing(pub f32);

impl From<Vec2> for Facing {
    fn from(value: Vec2) -> Self {
        Self(value.to_angle())
    }
}

impl From<Facing> for Vec2 {
    fn from(val: Facing) -> Self {
        Vec2::from_angle(val.0)
    }
}

impl From<Vec3> for Facing {
    fn from(value: Vec3) -> Self {
        Self(Position::from(value).to_angle())
    }
}

impl From<Facing> for Vec3 {
    fn from(val: Facing) -> Self {
        Position(Vec2::from_angle(val.0)).into()
    }
}

fn update_facing(q: Query<(&Facing, &mut Transform), Changed<Facing>>) {
    for (facing, mut trans) in q {
        trans.look_to(Vec3::from(*facing), Vec3::Y);
    }
}
