use std::sync::mpmc::Receiver;

use bevy::{
    ecs::{
        bundle::NoBundleEffect,
        spawn::SpawnIter,
        system::{IntoObserverSystem, ObserverSystem},
    },
    prelude::*,
};
use lobby_common::{
    ClientToLobby, LobbyId, LobbyInfo, LobbyShortInfo, LobbyToClient, PlayerId, PlayerInfo, Team,
};
use tokio::sync::mpsc::error::TryRecvError;

use crate::{
    main_ui::ConnectionState, network::{LobbyConnectionFailed, LobbyMessage, LobbyReceiver, LobbySender}, new_ui::{
        button::ButtonView, list::ListView, subtree::SubtreeView, text::TextView, tree::IfRunner, ErasedView, View, ViewExt
    }, ui::{button::button2, scrollable, text::text, ObservedBy}, LobbyMode, Options
};

use super::{in_champ_select::champ_select2, in_lobby::lobby_ui2, send_msg, LobbyAnchor, LobbyMenuState};

pub fn client(app: &mut App) {
    app.add_systems(Update, listen_to_lobby_server)
        .add_systems(OnEnter(LobbyMenuState::LobbyList), on_state_lobby_list)
        .add_observer(on_lobby_disconnect);
}

macro events($($name:ident $(($($tt:tt)*))?;)*) {
    $(
        #[derive(Event)]
        pub struct $name $(($($tt)*))?;
    )*
}

events! {
    LobbyConnected;
    GoToChampSelect;
    ReturnFromChampSelect;
    PlayerSelectedChamp(pub PlayerId, pub String);
}

// #[derive(Event)]
// pub struct LobbyConnected;
#[derive(Event)]
pub struct LobbyConnectionLost;
#[derive(Event)]
pub struct LobbyListReceived(pub Vec<LobbyShortInfo>);
#[derive(Event)]
pub struct LobbyInfoReceived(pub LobbyInfo);
#[derive(Event)]
pub struct WeJoinedLobby(pub LobbyId);
#[derive(Event)]
pub struct WeLeftLobby;
#[derive(Event)]
pub struct PlayerJoinedLobby(pub PlayerId);
#[derive(Event)]
pub struct PlayerLeftLobby(pub PlayerId);
#[derive(Event)]
pub struct PlayerInfoReceived(pub PlayerInfo);
#[derive(Debug, Event)]
pub struct PlayerChangedTeam(pub PlayerId, pub Team);
#[derive(Event)]
pub struct PlayerChangedPositions(pub PlayerId, pub PlayerId);

#[derive(Resource)]
pub struct MyPlayerId(pub PlayerId);

fn listen_to_lobby_server(
    options: Res<Options>,
    receiver: Option<ResMut<LobbyReceiver>>,
    sender: Option<Res<LobbySender>>,
    state: Option<Res<State<LobbyMenuState>>>,
    mut commands: Commands,
) {
    let (Some(mut receiver), Some(sender)) = (receiver, sender) else {
        return;
    };

    loop {
        let event = match receiver.0.try_recv() {
            Ok(event) => event,
            Err(TryRecvError::Empty) => break,
            Err(TryRecvError::Disconnected) => {
                commands.remove_resource::<LobbyReceiver>();
                break;
            }
        };

        info!("event: {event:?}");
        match event {
            LobbyMessage::LobbyConnected(id) => {
                commands.set_state(ConnectionState::Connected);
                commands.insert_resource(MyPlayerId(id));
            }
            LobbyMessage::LobbyConnectionFailed(err) => {
                info!("Triggering lobbyconnectedfailed");
                commands.trigger(LobbyConnectionFailed(err));
            }
            LobbyMessage::ConnectionLost => {
                commands.trigger(LobbyConnectionLost);
            }
            LobbyMessage::Message(lobby_to_client) => match lobby_to_client {
                LobbyToClient::Handshake { .. } => unreachable!(),
                LobbyToClient::LobbyList(lobby_short_infos) => {
                    if matches!(options.lobby_mode, LobbyMode::AutoJoinFirst)
                        && lobby_short_infos.len() > 0
                    {
                        _ = sender
                            .0
                            .send(ClientToLobby::JoinLobby(lobby_short_infos[0].id));
                    } else if let Some(ref state) = state
                        && *state.get() == LobbyMenuState::LobbyList
                    {
                        commands.run_system_cached_with(populate_lobby_list, lobby_short_infos);
                    }
                }
                LobbyToClient::LobbyInfo(lobby_info) => {
                    commands.trigger(LobbyInfoReceived(lobby_info));
                }
                LobbyToClient::YouJoinedLobby(lobby_id) => {
                    commands.trigger(WeJoinedLobby(lobby_id));
                }
                LobbyToClient::YouLeftLobby => {
                    commands.trigger(WeLeftLobby);
                }
                LobbyToClient::PlayerJoinedLobby(player_id) => {
                    commands.trigger(PlayerLeftLobby(player_id));
                }
                LobbyToClient::PlayerLeftLobby(player_id) => {
                    commands.trigger(PlayerLeftLobby(player_id));
                }
                LobbyToClient::PlayerInfo(player_info) => {
                    commands.trigger(PlayerInfoReceived(player_info));
                }
                LobbyToClient::PlayerChangedTeam(player_id, team) => {
                    commands.trigger(PlayerChangedTeam(player_id, team))
                }
                LobbyToClient::PlayerChangedPositions(player_id, player_id1) => {
                    commands.trigger(PlayerChangedPositions(player_id, player_id1));
                }
                LobbyToClient::GoToChampSelect => {
                    commands.trigger(GoToChampSelect);
                }
                LobbyToClient::ReturnFromChampSelect => {
                    commands.trigger(ReturnFromChampSelect);
                }
                LobbyToClient::PlayerSelectedChamp(player_id, champ) => {
                    commands.trigger(PlayerSelectedChamp(player_id, champ));
                }
            },
        }
    }
}

fn on_state_lobby_list(
    mut options: ResMut<Options>,
    sender: Res<LobbySender>,
    mut commands: Commands,
) {
    // commands.spawn(lobby_list(&[]));
    match options.lobby_mode {
        LobbyMode::AutoCreate => {
            options.lobby_mode = LobbyMode::None;
            _ = sender.0.send(ClientToLobby::CreateAndJoinLobby)
        }
        _ => _ = sender.0.send(ClientToLobby::FetchLobbyList),
    }
}

fn lobby_list(lobbies: &[LobbyShortInfo]) -> impl Bundle {
    let bundles = lobbies.iter().map(lobby_list_entry).collect::<Vec<_>>();
    (
        StateScoped(LobbyMenuState::LobbyList),
        Node {
            flex_direction: FlexDirection::Column,
            flex_grow: 1.0,
            ..default()
        },
        children![
            button_bar(),
            (
                Node {
                    flex_direction: FlexDirection::Column,
                    overflow: Overflow::scroll_y(),
                    flex_basis: Val::Px(0.0),
                    flex_grow: 1.0,
                    ..default()
                },
                ScrollPosition::default(),
                Children::spawn(SpawnIter(bundles.into_iter())),
                ObservedBy::new(scrollable::scroll)
            )
        ],
    )
}

fn button_bar() -> impl Bundle {
    (
        Node {
            flex_direction: FlexDirection::Row,
            justify_content: JustifyContent::SpaceBetween,
            ..default()
        },
        children![
            button2("Create Lobby", create_lobby),
            button2("Refresh", refresh_lobby_list),
        ],
    )
}

fn create_lobby(sender: Res<LobbySender>) {
    let _ = sender.0.send(ClientToLobby::CreateAndJoinLobby);
}

fn lobby_list_entry(lobby: &LobbyShortInfo) -> impl Bundle {
    (
        Node {
            flex_direction: FlexDirection::Row,
            column_gap: Val::Px(10.0),
            ..default()
        },
        children![
            text(lobby.name.clone()),
            text(format!("{}/{}", lobby.player_count, lobby.max_player_count)),
            button2("Join", send_msg(ClientToLobby::JoinLobby(lobby.id))),
        ],
    )
}

fn refresh_lobby_list(sender: Res<LobbySender>) {
    let _ = sender.0.send(ClientToLobby::FetchLobbyList);
}

fn populate_lobby_list(
    lobbies: In<Vec<LobbyShortInfo>>,
    // anchor: Single<Entity, With<LobbyAnchor>>,
    mut commands: Commands,
) {
    // commands
    //     .entity(*anchor)
    //     .despawn_related::<Children>()
    //     .with_child(lobby_list(&lobbies));
    // commands.insert_resource(LobbyList(lobbies.0));
    commands.insert_resource(LobbyList(
       lobbies.0
    ));
}

fn on_lobby_disconnect(trigger: Trigger<LobbyConnectionLost>, mut commands: Commands) {
    commands.set_state(ConnectionState::NotConnected);
}

#[derive(Resource)]
struct LobbyList(Vec<LobbyShortInfo>);

pub fn connected_to_lobby_server(lobby_state: Res<State<LobbyMenuState>>) -> Option<impl View + use<>> {
    if !lobby_state.is_changed() {
        return None;
    }

    Some(match lobby_state.get() {
        LobbyMenuState::LobbyList => lobby_list2().boxed(),
        LobbyMenuState::InLobby => lobby_ui2().boxed(),
        LobbyMenuState::InChampSelect => champ_select2().boxed(),
    })
}

pub fn lobby_list2() -> impl View {
    ListView::new()
    .with(ListView::new()
        .with(ButtonView::new(TextView::new("Create Lobby"), "create lobby", send_msg(ClientToLobby::CreateAndJoinLobby)))
        .with(ButtonView::new(TextView::new("Refresh"), "refresh", send_msg(ClientToLobby::FetchLobbyList)))
    ).with(
        SubtreeView::new(
            "lobby_list",
            IfRunner::new(lobby_list3, |world| world.contains_resource::<LobbyList>()),
        ).styled().height(Val::Px(0.0)).flex_grow(1.0).flex_basis(Val::Px(0.0))
    )
    .styled()
    .width(Val::Percent(100.0))
    .height(Val::Percent(100.0))
    .flex_direction(FlexDirection::Column)
    .flex_grow(1.0)
}

fn lobby_list3(list: Res<LobbyList>) -> Option<impl View + use<>> {
    if !list.is_changed() {
        return None;
    }

    let mut list_view = ListView::new();
    for lobby in &list.0 {
        list_view.add(lobby_list_entry2(lobby));
    }

    Some(
        list_view
            .styled()
            .flex_direction(FlexDirection::Column)
            .flex_grow(1.0)
            .scrollable(),
    )
}

fn lobby_list_entry2(info: &LobbyShortInfo) -> impl View + use<> {
    ListView::new()
        .with(TextView::new(&info.name).styled().flex_grow(1.0))
        .with(TextView::new(format!(
            "{}/{}",
            info.player_count, info.max_player_count
        )))
        .with(ButtonView::new(
            TextView::new("Join"),
            format!("join_btn_{:?}", info.id),
            send_msg(ClientToLobby::JoinLobby(info.id)),
        ))
        .styled()
        .column_gap(Val::Px(10.0))
        .padding(UiRect::all(Val::Px(5.0)))
}
