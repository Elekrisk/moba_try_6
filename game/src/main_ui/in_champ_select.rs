use bevy::{ecs::spawn::SpawnIter, prelude::*};
use lobby_common::{ClientToLobby, PlayerId, Team};

use crate::{new_ui::View, ui::text::text, LobbySender};

use super::{
    LobbyAnchor, LobbyMenuState,
    in_lobby::CurrentLobbyInfo,
    lobby_list::{GoToChampSelect, ReturnFromChampSelect},
};

pub fn client(app: &mut App) {
    app.add_systems(OnEnter(LobbyMenuState::InChampSelect), setup_ui)
        .add_observer(on_goto_champ_select)
        .add_observer(on_return_from_champ_select);
}

fn setup_ui(
    anchor: Single<Entity, With<LobbyAnchor>>,
    info: Res<CurrentLobbyInfo>,
    sender: Res<LobbySender>,
    mut commands: Commands,
) {
    commands.entity(*anchor).with_child(champ_select(&info));
    // _ = sender.send(ClientToLobby::GetLobbyInfo(info.0.short.id));
}

fn champ_select(info: &CurrentLobbyInfo) -> impl Bundle {
    let even_teams = info
        .0
        .teams
        .chunks(2)
        .enumerate()
        .map(|(i, c)| (i * 2, c[0].clone()))
        .collect::<Vec<_>>();
    let odd_teams = info
        .0
        .teams
        .chunks(2)
        .enumerate()
        .flat_map(|(i, c)| c.get(1).map(|p| (i * 2 + 1, p.clone())))
        .collect::<Vec<_>>();

    (
        Node { ..default() },
        children![
            (
                Node {
                    flex_direction: FlexDirection::Column,
                    flex_grow: 1.0,
                    ..default()
                },
                Children::spawn(SpawnIter(
                    even_teams.into_iter().map(|(t, p)| team_list(Team(t), p))
                ))
            ),
            (
                Node {
                    flex_direction: FlexDirection::Column,
                    flex_grow: 1.0,
                    ..default()
                },
                Children::spawn(SpawnIter(
                    odd_teams.into_iter().map(|(t, p)| team_list(Team(t), p))
                ))
            ),
        ],
    )
}

fn team_list(team: Team, players: Vec<PlayerId>) -> impl Bundle {
    (
        Node {
            flex_direction: FlexDirection::Column,
            ..default()
        },
        children![text(format!("Team {}", team.0)), player_list(players),],
    )
}

fn player_list(players: Vec<PlayerId>) -> impl Bundle {
    (
        Node {
            flex_direction: FlexDirection::Column,
            ..default()
        },
        Children::spawn(SpawnIter(players.into_iter().map(player_entry))),
    )
}

fn player_entry(player: PlayerId) -> impl Bundle {
    (
        Node {
            flex_direction: FlexDirection::Row,
            ..default()
        },
        children![text("Unknown")],
    )
}

fn on_goto_champ_select(trigger: Trigger<GoToChampSelect>, mut commands: Commands) {
    commands.set_state(LobbyMenuState::InChampSelect);
}

fn on_return_from_champ_select(trigger: Trigger<ReturnFromChampSelect>, mut commands: Commands) {
    commands.set_state(LobbyMenuState::InLobby);
}

pub fn champ_select2() -> impl View {
    
}
