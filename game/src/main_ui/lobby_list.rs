use bevy::prelude::*;
use engine_common::ChampionId;
use lightyear::netcode::ConnectToken;
use lobby_common::{
    ClientToLobby, LobbyId, LobbyInfo, LobbyShortInfo, LobbyToClient, PlayerId, PlayerInfo, Team,
};
use tokio::sync::mpsc::error::TryRecvError;

use crate::{
    LobbyMode, Options,
    ingame::ConnectToGameServer,
    main_ui::ConnectionState,
    network::{LobbyConnectionFailed, LobbyMessage, LobbyReceiver, LobbySender},
    new_ui::{
        View, ViewExt, button::ButtonView, list::ListView, subtree::SubtreeView, text::TextView,
        tree::IfRunner,
    },
};

use super::{LobbyMenuState, in_champ_select::champ_select2, in_lobby::lobby_ui2, send_msg};

pub fn client(app: &mut App) {
    app.add_systems(Update, listen_to_lobby_server)
        .add_systems(OnEnter(LobbyMenuState::LobbyList), on_state_lobby_list)
        .add_observer(on_lobby_disconnect);
}

macro events($($name:ident $(($($tt:tt)*))?;)*) {
    $(

        #[allow(dead_code)]
        #[derive(Event)]
        pub struct $name $(($($tt)*))?;
    )*
}

events! {
    LobbyConnected;
    GoToChampSelect;
    ReturnFromChampSelect;
    PlayerSelectedChamp(pub PlayerId, pub ChampionId);
    PlayerLockedSelection(pub PlayerId);
}

// #[derive(Event)]
// pub struct LobbyConnected;
#[derive(Event)]
pub struct LobbyConnectionLost;
#[derive(Event)]
#[allow(dead_code)]
pub struct LobbyListReceived(pub Vec<LobbyShortInfo>);
#[derive(Event)]
pub struct LobbyInfoReceived(pub LobbyInfo);
#[derive(Event)]
pub struct WeJoinedLobby(pub LobbyId);
#[derive(Event)]
pub struct WeLeftLobby;
#[derive(Event)]
#[allow(dead_code)]
pub struct PlayerJoinedLobby(pub PlayerId);
#[derive(Event)]
#[allow(dead_code)]
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

        // info!("event: {event:?}");
        match event {
            LobbyMessage::LobbyConnected(id) => {
                commands.set_state(ConnectionState::Connected);
                commands.insert_resource(MyPlayerId(id));
            }
            LobbyMessage::LobbyConnectionFailed(err) => {
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
                LobbyToClient::PlayerLockedSelection(player_id) => {
                    commands.trigger(PlayerLockedSelection(player_id));
                }
                LobbyToClient::GameStarted(items) => {
                    let token = ConnectToken::try_from_bytes(&items).unwrap();
                    commands.queue(ConnectToGameServer(token));
                }
            },
        }
    }
}

fn on_state_lobby_list(mut options: ResMut<Options>, sender: Res<LobbySender>) {
    match options.lobby_mode {
        LobbyMode::AutoCreate => {
            options.lobby_mode = LobbyMode::None;
            _ = sender.0.send(ClientToLobby::CreateAndJoinLobby)
        }
        _ => _ = sender.0.send(ClientToLobby::FetchLobbyList),
    }
}

fn populate_lobby_list(lobbies: In<Vec<LobbyShortInfo>>, mut commands: Commands) {
    commands.insert_resource(LobbyList(lobbies.0));
}

fn on_lobby_disconnect(_trigger: Trigger<LobbyConnectionLost>, mut commands: Commands) {
    commands.set_state(ConnectionState::NotConnected);
}

#[derive(Resource)]
struct LobbyList(Vec<LobbyShortInfo>);

pub fn connected_to_lobby_server(
    lobby_state: Res<State<LobbyMenuState>>,
) -> Option<impl View + use<>> {
    if !lobby_state.is_changed() {
        return None;
    }

    Some(match lobby_state.get() {
        LobbyMenuState::LobbyList => lobby_list().boxed(),
        LobbyMenuState::InLobby => lobby_ui2().boxed(),
        LobbyMenuState::InChampSelect => champ_select2().boxed(),
    })
}

pub fn lobby_list() -> impl View {
    ListView::new()
        .with(
            ListView::new()
                .with(ButtonView::new(
                    TextView::new("Create Lobby"),
                    "create lobby",
                    send_msg(ClientToLobby::CreateAndJoinLobby),
                ))
                .with(ButtonView::new(
                    TextView::new("Refresh"),
                    "refresh",
                    send_msg(ClientToLobby::FetchLobbyList),
                )),
        )
        .with(
            SubtreeView::new(
                "lobby_list",
                IfRunner::new(lobby_list_subtree, |world| {
                    world.contains_resource::<LobbyList>()
                }),
            )
            .styled()
            .width(Val::Percent(100.0)),
        )
        .styled()
        .width(Val::Percent(100.0))
        .height(Val::Percent(100.0))
        .flex_direction(FlexDirection::Column)
        .flex_grow(1.0)
}

fn lobby_list_subtree(list: Res<LobbyList>) -> Option<impl View + use<>> {
    if !list.is_changed() {
        return None;
    }

    let mut list_view = ListView::new();
    for lobby in &list.0 {
        list_view.add(lobby_list_entry(lobby));
    }

    Some(
        list_view
            .styled()
            .flex_direction(FlexDirection::Column)
            .flex_grow(1.0)
            .scrollable(),
    )
}

fn lobby_list_entry(info: &LobbyShortInfo) -> impl View + use<> {
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
