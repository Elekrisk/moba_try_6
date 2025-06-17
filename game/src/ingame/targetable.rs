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

#[derive(Debug, Clone, Copy, PartialEq, Component, Serialize, Deserialize)]
pub struct Health(pub f32);

pub fn common(app: &mut App) {
    app.register_component::<Health>(ChannelDirection::ServerToClient);

    if app.is_client() {
        app.add_plugins(AnchorUiPlugin::<UiCameraMarker>::new());
        app.add_observer(spawn_floating_health_bars);
        app.add_systems(
            Update,
            update_floating_health_bar.run_if(in_state(GameState::InGame)),
        );
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

fn spawn_floating_health_bars(trigger: Trigger<OnInsert, Health>, mut commands: Commands) {
    commands
        .entity(trigger.target())
        .insert(AnchoredUiNodes::spawn_one((
            AnchorUiConfig::default()
                .with_horizontal_anchoring(HorizontalAnchor::Mid)
                .with_vertical_anchoring(VerticalAnchor::Bottom)
                .with_offset(vec3(0.0, 2.2, -0.2)),
            StateScoped(GameState::InGame),
            Node { ..default() },
            children![
                (
                    Node {
                        width: Val::Px(20.0),
                        height: Val::Px(20.0),
                        ..default()
                    },
                    BackgroundColor(Color::Srgba(palettes::tailwind::BLUE_500)),
                ),
                (
                    Node {
                        width: Val::Px(160.0),
                        height: Val::Px(20.0),
                        border: UiRect::all(Val::Px(2.0)),
                        ..default()
                    },
                    BorderColor(Color::Srgba(palettes::tailwind::AMBER_500)),
                    BackgroundColor(Color::Srgba(palettes::tailwind::GRAY_500)),
                    children![(
                        Node {
                            width: Val::Percent(100.0),
                            height: Val::Percent(100.0),
                            ..default()
                        },
                        BackgroundColor(Color::Srgba(palettes::tailwind::GREEN_500)),
                    )],
                )
            ],
        )));
}

fn update_floating_health_bar(
    q: Query<
        (
            &Health,
            &AnchoredUiNodes,
            &Team,
            &Visibility,
            Option<&AutoAttackTimer>,
        ),
        Or<(Changed<Health>, Changed<Visibility>)>,
    >,
    q2: Query<&Children>,
    mut q3: Query<(&mut Node, &mut BackgroundColor)>,
    my_team: Res<MyTeam>,
) {
    for (hp, ui_nodes, team, vis, aa) in q {
        assert_eq!(ui_nodes.iter().len(), 1, "multiple anchored ui nodes");

        let base = ui_nodes.iter().next().unwrap();
        q3.get_mut(base).unwrap().0.display = match vis {
            Visibility::Hidden => Display::None,
            _ => Display::Flex,
        };

        let e = q2.get(q2.get(base).unwrap()[1]).unwrap()[0];
        let (mut node, mut bg) = q3.get_mut(e).unwrap();
        node.width = Val::Percent(hp.0);
        bg.0 = if *team == my_team.0 {
            palettes::tailwind::GREEN_500.into()
        } else {
            palettes::tailwind::RED_500.into()
        };

        if let Some(aa) = aa {
            let e = q2.get(base).unwrap()[0];

            let percent = aa.timer / aa.total_time;
            let (mut node, mut bg) = q3.get_mut(e).unwrap();
            node.height = Val::Px((1.0 - percent) * 20.0);
            bg.0 = if aa.timer == 0.0 || !aa.has_occurred {
                palettes::tailwind::AMBER_500.into()
            } else {
                palettes::tailwind::BLUE_500.into()
            };
        }
    }
}
