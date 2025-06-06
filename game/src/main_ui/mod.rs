use crate::{
    network::{ConnectToLobbyCommand, LobbyConnectionFailed}, new_ui::{
        button::ButtonView, container::ContainerView, custom::{CustomView, CustomWidget}, list::ListView, subtree::SubtreeView, tabbed::TabbedView, text::TextView, text_edit::{TextEdit, TextEditView}, tree::{OnceRunner, UiTree}, View, ViewExt
    }, GameState, LobbySender, Options
};
use bevy::{input_focus::InputFocus, prelude::*, state::state::FreelyMutableState};
use lobby_common::ClientToLobby;
use lobby_list::connected_to_lobby_server;
use crate::new_ui::Widget;

pub mod in_champ_select;
pub mod in_lobby;
pub mod lobby_list;

pub fn client(app: &mut App) {
    app.insert_state(ConnectionState::NotConnected)
        .add_sub_state::<LobbyMenuState>()
        .add_plugins((
            lobby_list::client,
            in_lobby::client,
            in_champ_select::client,
        ))
        .add_systems(OnEnter(GameState::NotInGame), create_ui)
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

pub fn create_ui(mut options: ResMut<Options>, mut commands: Commands) {
    commands.spawn((
        StateScoped(GameState::NotInGame),
        Node {
            width: Val::Percent(100.0),
            height: Val::Percent(100.0),
            ..default()
        },
        UiTree::once(ui_root2),
    ));
    if options.connect {
        options.connect = false;
        commands.insert_resource(LobbyUrl("localhost".into()));
        commands.run_system_cached(connect);
    }
}

fn ui_root2() -> Option<impl View> {

    let tabbed = TabbedView::new()
            .with(
                "Lobbies",
                ContainerView::new(
                    SubtreeView::new("lobby_anchor", lobby_anchor2)
                        .styled()
                        .width(Val::Percent(100.0)),
                )
                .styled()
                .width(Val::Percent(100.0))
                .flex_grow(1.0),
            )
            .with("Reference", TextView::new("Waow, reference :D"))
            .with("Settings", TextView::new("Waow, settings :D"))
            .styled()
            .width(Val::Percent(100.0))
            .height(Val::Percent(100.0));

    let root = CustomView {
        data: tabbed,
        build: Box::new(|tabbed, parent| {
            let inner = tabbed.build(parent);
            parent.commands().entity(inner.entity()).observe(|mut trigger: Trigger<Pointer<Click>>, mut input_focus: ResMut<InputFocus>| {
                trigger.propagate(false);
                input_focus.0 = None;
            });
            CustomWidget {
                entity: inner.entity(),
                parent: parent.target_entity(),
                data: inner,
            }
        }),
        rebuild: Box::new(|tabbed, prev, widget, commands| {
            tabbed.rebuild(&prev.data, &mut widget.data, commands);
        }),
    };

    Some(
        root,
    )
}

fn lobby_anchor2(state: Res<State<ConnectionState>>) -> Option<impl View + use<>> {
    if !state.is_changed() {
        return None;
    }

    // Some(TextEditView::new("abc", ""))

    Some(match state.get() {
        ConnectionState::NotConnected => lobby_connection_screen2().boxed(),
        ConnectionState::Connecting => connecting_screen().boxed(),
        ConnectionState::ConnectionFailed => {
            SubtreeView::new("error_screen", OnceRunner::new(connection_error_screen))
                .styled()
                .width(Val::Percent(100.0))
                .boxed()
        }
        ConnectionState::Connected => {
            SubtreeView::new("lobby_connected_view", connected_to_lobby_server)
                .styled()
                .width(Val::Percent(100.0))
                .boxed()
        }
    })
}

#[derive(Component, Debug, Default)]
struct ConnectionAddr;

#[derive(Resource)]
struct LobbyUrl(String);

fn lobby_connection_screen2() -> impl View {
    // ListView::new()
    //     .with(TextView::new("Connect to localhost?"))
    //     .with(ButtonView::new(
    //         TextView::new("Connect"),
    //         "connect",
    //         set_state(ConnectionState::Connecting),
    //     ))
    //     .styled()
    //     .flex_direction(FlexDirection::Column)

    ListView::new()
        .with(ListView::new().with("Address:").with(TextEditView::<ConnectionAddr>::new("localhost", "")).styled().column_gap(Val::Px(10.0)))
        .with(ButtonView::new(
            TextView::new("Connect"),
            "connect",
            |text: Single<&TextEdit, With<ConnectionAddr>>, mut commands: Commands| {
                let text = &text.text;
                commands.insert_resource(LobbyUrl(text.clone()));
                commands.set_state(ConnectionState::Connecting);
            },
        ))
        .styled()
        .flex_direction(FlexDirection::Column)
}

fn connect(mut commands: Commands) {
    commands.set_state(ConnectionState::Connecting);
}

fn on_connect_start(lobby_url: Res<LobbyUrl>, mut commands: Commands) {
    commands.queue(ConnectToLobbyCommand(format!("{}:54654", lobby_url.0)));
    commands.add_observer(on_lobby_connection_error);
}

fn connecting_screen() -> impl View {
    TextView::new("Connecting...")
}

fn on_lobby_connection_error(trigger: Trigger<LobbyConnectionFailed>, mut commands: Commands) {
    commands.insert_resource(LobbyConnectionFailureReason(trigger.event().0.to_string()));
    commands.set_state(ConnectionState::ConnectionFailed);

    // Despawn observer
    commands.entity(trigger.observer()).despawn();
}

#[derive(Resource)]
struct LobbyConnectionFailureReason(String);

fn connection_error_screen(reason: Res<LobbyConnectionFailureReason>) -> Option<impl View + use<>> {
    Some(
        ListView::new()
            .with(TextView::new("Could not connect to lobby server:"))
            .with(TextView::new(&reason.0))
            .with(ButtonView::new(
                TextView::new("Back"),
                "back",
                set_state(ConnectionState::NotConnected),
            ))
            .styled()
            .flex_direction(FlexDirection::Column),
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
