use bevy::{color::palettes, prelude::*};
use lobby_common::Team;

use crate::{
    AppExt, GameState, Unit,
    ingame::{
        camera::{AnchorUiConfig, AnchoredUiNodes, HorizontalAnchor, VerticalAnchor},
        structure::Structure,
        unit::{MyTeam, UnitType, stats::StatBlock},
        vision,
    },
};

use super::Health;

pub fn plugin(app: &mut App) {
    if app.is_client() {
        app.add_observer(on_health_added);
        app.add_systems(
            Update,
            update_healthbar
                .run_if(in_state(GameState::InGame))
                .after(vision::change_visibility),
        );
    }
}

#[derive(Component)]
#[relationship_target(relationship = HealthBarOf)]
struct HealthBarRef(Entity);

#[derive(Component)]
#[relationship(relationship_target = HealthBarRef)]
struct HealthBarOf(Entity);

#[derive(Component)]
#[relationship_target(relationship = HealthBarValueOf)]
struct HealthBarValueRef(Entity);

#[derive(Component)]
#[relationship(relationship_target = HealthBarValueRef)]
struct HealthBarValueOf(Entity);

pub fn on_health_added(
    trigger: Trigger<OnInsert, Health>,
    q: Query<&UnitType>,
    mut commands: Commands,
) {
    let Ok(unit_type) = q.get(trigger.target()) else {
        info!("No unit type on");
        return;
    };

    match unit_type {
        UnitType::Normal => {
            // Small healthbar
            commands
                .entity(trigger.target())
                .insert(AnchoredUiNodes::spawn_one((
                    AnchorUiConfig::default()
                        .with_horizontal_anchoring(HorizontalAnchor::Mid)
                        .with_vertical_anchoring(VerticalAnchor::Bottom)
                        .with_offset(vec3(0.0, 1.2, -0.2)),
                    GlobalZIndex(0),
                    Node::default(),
                    Children::spawn_one((
                        Node {
                            width: Val::Px(80.0),
                            height: Val::Px(5.0),
                            border: UiRect::all(Val::Px(1.0)),
                            ..default()
                        },
                        BorderColor(Color::Srgba(palettes::tailwind::AMBER_500)),
                        BackgroundColor(Color::Srgba(palettes::tailwind::GRAY_500)),
                        HealthBarOf(trigger.target()),
                        Children::spawn_one((
                            Node {
                                width: Val::Percent(100.0),
                                height: Val::Percent(100.0),
                                ..default()
                            },
                            BackgroundColor(Color::Srgba(palettes::tailwind::GREEN_500)),
                            HealthBarValueOf(trigger.target()),
                        )),
                    )),
                )));
        }
        UnitType::Champion => {
            // Big healthbar
            commands
                .entity(trigger.target())
                .insert(AnchoredUiNodes::spawn_one((
                    AnchorUiConfig::default()
                        .with_horizontal_anchoring(HorizontalAnchor::Mid)
                        .with_vertical_anchoring(VerticalAnchor::Bottom)
                        .with_offset(vec3(0.0, 2.2, -0.2)),
                    GlobalZIndex(1),
                    Node::default(),
                    Children::spawn_one((
                        Node {
                            width: Val::Px(160.0),
                            height: Val::Px(20.0),
                            border: UiRect::all(Val::Px(2.0)),
                            ..default()
                        },
                        BorderColor(Color::Srgba(palettes::tailwind::AMBER_500)),
                        BackgroundColor(Color::Srgba(palettes::tailwind::GRAY_500)),
                        HealthBarOf(trigger.target()),
                        Children::spawn_one((
                            Node {
                                width: Val::Percent(100.0),
                                height: Val::Percent(100.0),
                                ..default()
                            },
                            BackgroundColor(Color::Srgba(palettes::tailwind::GREEN_500)),
                            HealthBarValueOf(trigger.target()),
                        )),
                    )),
                )));
        }
    }
}

fn update_healthbar(
    q: Query<
        (&Health, &StatBlock, &HealthBarRef, &HealthBarValueRef, &Visibility, &Team),
        Or<(Changed<Health>, Changed<Visibility>)>,
    >,
    mut q2: Query<(&mut Node, &mut BackgroundColor, &mut Visibility), Without<Health>>,
    my_team: Res<MyTeam>,
) {
    for (hp, stats, healthbar, healthbar_value, vis, team) in q {
        let (_, _, mut bar_vis) = q2.get_mut(healthbar.0).unwrap();
        bar_vis.set_if_neq(*vis);

        let (mut node, mut bg, _) = q2.get_mut(healthbar_value.0).unwrap();
        node.width = Val::Percent(hp.0 / stats.max_health.base * 100.0);
        bg.0 = if *team == my_team.0 {
            palettes::tailwind::GREEN_500.into()
        } else {
            palettes::tailwind::RED_500.into()
        };
    }
}
