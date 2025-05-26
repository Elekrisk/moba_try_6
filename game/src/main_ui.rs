use bevy::{
    ecs::{entity_disabling::Disabled, spawn::SpawnIter},
    prelude::*,
    state::state::FreelyMutableState,
};
use lobby_common::ClientToLobby;

use crate::{
    ClientState, LobbySender, Options,
    network::{ConnectToLobbyCommand, LobbyConnectionFailed},
    ui::{ObservedBy, button::button2, scrollable, tab_bar::tab_bar, text::text},
};

mod in_lobby;
mod lobby_list;
mod in_champ_select;

pub fn client(app: &mut App) {
    app.insert_state(ConnectionState::NotConnected)
        .add_sub_state::<LobbyMenuState>()
        .add_plugins((lobby_list::client, in_lobby::client, in_champ_select::client))
        .add_systems(OnEnter(ClientState::NotInGame), create_ui)
        .add_systems(
            OnEnter(ConnectionState::NotConnected),
            create_lobby_connection_screen,
        )
        .add_systems(OnEnter(ConnectionState::Connecting), on_connect_start);
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, States)]
#[states(scoped_entities)]
pub enum ConnectionState {
    NotConnected,
    Connecting,
    ConnectionFailed,
    Connected,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, SubStates, Default)]
#[source(ConnectionState = ConnectionState::Connected)]
#[states(scoped_entities)]
pub enum LobbyMenuState {
    #[default]
    LobbyList,
    InLobby,
    InChampSelect,
}

pub fn create_ui(options: Res<Options>, mut commands: Commands) {
    commands.spawn((StateScoped(ClientState::NotInGame), ui_root()));
    if options.connect {
        commands.run_system_cached(connect);
    }
}

fn ui_root() -> impl Bundle {
    (
        Node {
            width: Val::Percent(100.0),
            height: Val::Percent(100.0),
            flex_direction: FlexDirection::Column,
            ..default()
        },
        children![
            tab_bar()
                .tab("Lobbies", lobby_anchor())
                .tab("Reference", Text::new("Reference placeholder"))
                .tab("Settings", Text::new("Yeehaw")),
        ],
    )
}

#[derive(Component)]
pub struct LobbyAnchor;

fn lobby_anchor() -> impl Bundle {
    (
        Node {
            width: Val::Percent(100.0),
            height: Val::Percent(100.0),
            flex_direction: FlexDirection::Column,
            ..default()
        },
        LobbyAnchor,
        children![lobby_connection_screen()],
    )
}

fn create_lobby_connection_screen(
    anchor: Single<Entity, With<LobbyAnchor>>,
    mut commands: Commands,
) {
    commands
        .entity(*anchor)
        .with_child(lobby_connection_screen());
}

fn lobby_connection_screen() -> impl Bundle {
    (
        StateScoped(ConnectionState::NotConnected),
        Node {
            // width: Val::Percent(100.0),
            align_items: AlignItems::Center,
            justify_content: JustifyContent::Center,
            flex_direction: FlexDirection::Column,
            flex_grow: 1.0,
            ..default()
        },
        children![
            text("Connect to localhost?"),
            button2("Connect", set_state(ConnectionState::Connecting)),
        ],
    )
}

fn connect(mut commands: Commands) {
    commands.set_state(ConnectionState::Connecting);
}

fn on_connect_start(anchor: Single<Entity, With<LobbyAnchor>>, mut commands: Commands) {
    commands.entity(*anchor).with_child((
        StateScoped(ConnectionState::Connecting),
        connecting_screen(),
    ));
    commands.queue(ConnectToLobbyCommand("localhost:54654".into()));
    commands.add_observer(on_lobby_connection_error);
}

fn connecting_screen() -> impl Bundle {
    Text::new("Connecting...")
}

fn on_lobby_connection_error(
    trigger: Trigger<LobbyConnectionFailed>,
    anchor: Single<Entity, With<LobbyAnchor>>,
    mut commands: Commands,
) {
    commands.entity(*anchor).with_child((
        StateScoped(ConnectionState::ConnectionFailed),
        connection_error_screen(trigger.event()),
    ));

    commands.set_state(ConnectionState::ConnectionFailed);

    // Despawn observer
    commands.entity(trigger.observer()).despawn();
}

fn connection_error_screen(msg: &LobbyConnectionFailed) -> impl Bundle {
    (
        Node {
            flex_direction: FlexDirection::Column,
            ..default()
        },
        children![
            Text::new("Could not connect to lobby server:"),
            Text::new(msg.0.to_string()),
            button2("Back", set_state(ConnectionState::NotConnected))
        ],
    )
}

fn send_msg(msg: ClientToLobby) -> impl System<In = (), Out = ()> {
    IntoSystem::into_system(move |sender: Res<LobbySender>| {
        _ = sender.send(msg.clone());
    })
}

fn set_state<T: FreelyMutableState>(state: T) -> impl System<In = (), Out = ()> {
    IntoSystem::into_system(move |mut commands: Commands| {
        commands.set_state(state.clone());
    })
}
