use bevy::prelude::*;
use lobby_common::{ClientToLobby, PlayerId, Team};

use crate::{
    ChampDefs, LobbySender,
    new_ui::{
        View, ViewExt, button::ButtonView, image::ImageView, list::ListView, subtree::SubtreeView,
        tree::UiFunc,
    },
};

use super::{
    LobbyMenuState,
    in_lobby::{CurrentLobbyInfo, PlayerInfoCache},
    lobby_list::{GoToChampSelect, ReturnFromChampSelect},
    send_msg,
};

pub fn client(app: &mut App) {
    app.add_systems(OnEnter(LobbyMenuState::InChampSelect), setup_ui)
        .add_observer(on_goto_champ_select)
        .add_observer(on_return_from_champ_select);
}

fn setup_ui(info: Res<CurrentLobbyInfo>, sender: Res<LobbySender>) {
    _ = sender.send(ClientToLobby::GetLobbyInfo(info.0.short.id));
}

fn on_goto_champ_select(_trigger: Trigger<GoToChampSelect>, mut commands: Commands) {
    commands.set_state(LobbyMenuState::InChampSelect);
}

fn on_return_from_champ_select(_trigger: Trigger<ReturnFromChampSelect>, mut commands: Commands) {
    commands.set_state(LobbyMenuState::InLobby);
}

pub fn champ_select2() -> impl View {
    SubtreeView::new("champ select", champ_select3)
        .styled()
        .width(Val::Percent(100.0))
}

fn champ_select3(lobby: Res<CurrentLobbyInfo>) -> Option<impl View + use<>> {
    if !lobby.is_changed() {
        return None;
    }

    let lobby = &lobby.0;

    let team_pairs = lobby.teams.chunks(2).enumerate().map(|(i, slice)| {
        slice
            .iter()
            .enumerate()
            .map(move |(j, p)| (Team(i * 2 + j), p))
    });

    let mut team_list = ListView::new();

    for pair in team_pairs {
        team_list.add(team_pair(pair));
    }

    let middle = ListView::new()
        .with(
            SubtreeView::new("champ_select_buttons", champ_select_buttons)
                .styled()
                .width(Val::Percent(100.0))
                .scrollable(),
        )
        .with(ButtonView::new(
            "Lock",
            "lock_selection",
            send_msg(ClientToLobby::LockSelection),
        ))
        .styled()
        .width(Val::Percent(34.0))
        .position_type(PositionType::Absolute)
        .flex_direction(FlexDirection::Column)
        .align_items(AlignItems::Center)
        .height(Val::Percent(100.0));

    let container = ListView::new()
        .with(
            team_list
                .styled()
                .flex_direction(FlexDirection::Column)
                .width(Val::Percent(100.0))
                .flex_grow(1.0)
                .flex_basis(Val::Px(0.0))
                .scrollable(),
        )
        .with(middle);

    Some(
        container
            .styled()
            .width(Val::Percent(100.0))
            .flex_direction(FlexDirection::Column)
            .align_items(AlignItems::Center),
    )
}

fn team_pair(mut pair: impl Iterator<Item = (Team, &Vec<PlayerId>)>) -> impl View {
    let (left_team, left_players) = pair.next().unwrap();

    let mut list = ListView::new().with(team(left_team, left_players));
    if let Some((right_team, right_players)) = pair.next() {
        list.add(team(right_team, right_players));
    }

    list.styled()
        .flex_direction(FlexDirection::Row)
        .justify_content(JustifyContent::SpaceBetween)
}

fn team(team: Team, players: &Vec<PlayerId>) -> impl View {
    ListView::from_iter(
        players
            .iter()
            .copied()
            .chain(std::iter::repeat(PlayerId::new()))
            .take(5)
            .map(|p| player_slot(team, p)),
    )
    .styled()
    .flex_direction(FlexDirection::Column)
    .width(Val::Percent(33.0))
}

fn player_slot(team: Team, player: PlayerId) -> impl View {
    player_slot_2(team, player)
}

fn player_slot_2(team: Team, player: PlayerId) -> SubtreeView<impl UiFunc> {
    let switch = team.0 % 2 == 1;
    SubtreeView::new(
        format!("player_slot_{}", player.0),
        move |lobby: Res<CurrentLobbyInfo>,
              mut cache: ResMut<PlayerInfoCache>,
              champs: Res<ChampDefs>,
              sender: Res<LobbySender>,
              time: Res<Time>| {
            if !lobby.is_changed() && !cache.is_changed() && !champs.is_changed() {
                return None;
            }

            let Some(player_info) = cache.fetch(player, &sender, &time) else {
                return Some("what".boxed());
            };

            let mut list = ListView::new()
                .with(player_info.name.clone())
                .with(
                    lobby
                        .0
                        .selected_champs
                        .get(&player)
                        .map(|c| champs.map.get(&c.id).map(|n| (n, c.locked)))
                        .flatten()
                        .map(|(def, locked)| {
                            if locked {
                                format!("[{}]", def.name)
                            } else {
                                def.name.clone()
                            }
                        }),
                )
                .styled()
                .justify_content(JustifyContent::SpaceBetween)
                .flex_grow(1.0);
            if switch {
                list = list.flex_direction(FlexDirection::RowReverse);
            }
            Some(list.boxed())
        },
    )
}

fn champ_select_buttons(res: Res<ChampDefs>) -> Option<impl View + use<>> {
    if !res.is_changed() {
        return None;
    }

    let mut list = ListView::new();

    let mut champs = res
        .map
        .values()
        .collect::<Vec<_>>();
    champs.sort_by_key(|c| &c.name);

    for champ in champs {
        let label = format!("champ_select_{}", champ.id.0);

        let button = ButtonView::new(
            ListView::new()
                .with(
                    ImageView::new(&champ.icon)
                        .styled()
                        .width(Val::Px(100.0))
                        .height(Val::Px(100.0)),
                )
                .with(champ.name.clone().styled().max_width(Val::Px(100.0)))
                .styled()
                .flex_direction(FlexDirection::Column),
            label,
            send_msg(ClientToLobby::SelectChamp(champ.id)),
        );

        list.add(button);
    }

    Some(
        list.styled()
            .flex_wrap(FlexWrap::Wrap)
            .justify_content(JustifyContent::Center)
            .row_gap(Val::Px(10.0))
            .column_gap(Val::Px(10.0))
            .width(Val::Percent(100.0)),
    )
}
