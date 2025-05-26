use bevy::{
    ecs::{
        relationship::RelatedSpawner,
        spawn::{SpawnIter, SpawnWith},
        system::{IntoObserverSystem, ObserverSystem},
    },
    platform::collections::HashMap,
    prelude::*,
};
use lobby_common::{ClientToLobby, LobbyInfo, LobbySettings, PlayerId, PlayerInfo, Team};
use player_slot::{PlayerMovedFromThisSlot, PlayerMovedToThisSlot, PlayerSlotMap};

use crate::{
    network::LobbySender,
    ui::{GlobalObserver, ObservedBy, button::button2, scrollable, text::text},
};

use super::{
    LobbyAnchor, LobbyMenuState,
    lobby_list::{
        LobbyInfoReceived, MyPlayerId, PlayerChangedPositions, PlayerChangedTeam,
        PlayerInfoReceived, PlayerJoinedLobby, PlayerLeftLobby, WeJoinedLobby, WeLeftLobby,
    },
    send_msg,
};

pub mod player_slot;

pub fn client(app: &mut App) {
    app.add_plugins(player_slot::client)
        .add_observer(on_lobby_info_update)
        .add_observer(setup)
        .add_observer(on_player_info_received)
        .add_observer(update_lobby_on::<PlayerJoinedLobby>)
        .add_observer(update_lobby_on::<PlayerLeftLobby>)
        .add_observer(on_player_swap_team)
        .add_observer(on_player_swap_positions)
        .add_observer(on_we_left_lobby)
        .insert_resource(PlayerInfoCache {
            cache: HashMap::new(),
        });
}

pub fn setup(
    trigger: Trigger<WeJoinedLobby>,
    anchor: Single<Entity, With<LobbyAnchor>>,
    sender: Res<LobbySender>,
    mut commands: Commands,
) {
    commands.set_state(LobbyMenuState::InLobby);
    commands.entity(*anchor).with_child(lobby_ui());
    let _ = sender
        .0
        .send(ClientToLobby::GetLobbyInfo(trigger.event().0));
}

#[derive(Component)]
struct LobbyTitle;

#[derive(Component)]
struct LobbyTeamHorizontalContainer;

#[derive(Component)]
struct LobbyTeamHorizontal(usize);

fn lobby_ui() -> impl Bundle {
    (
        StateScoped(LobbyMenuState::InLobby),
        Node {
            flex_direction: FlexDirection::Column,
            flex_basis: Val::Px(0.0),
            flex_grow: 1.0,
            ..default()
        },
        Name::new("Lobby UI Root"),
        children![
            (
                Node { ..default() },
                Name::new("Lobby Header"),
                children![
                    (text("Lobby").size(24.0), LobbyTitle,),
                    (button2("Exit Lobby", exit_lobby),)
                ]
            ),
            (
                Node::default(),
                LobbySettingsAnchor,
                GlobalObserver::new(llll)
            ),
            (
                Node {
                    flex_direction: FlexDirection::Column,
                    overflow: Overflow::scroll_y(),
                    flex_basis: Val::Px(0.0),
                    flex_grow: 1.0,
                    ..default()
                },
                ScrollPosition::default(),
                ObservedBy::new(scrollable::scroll),
                Name::new("Lobby Horizontal Container"),
                LobbyTeamHorizontalContainer,
            )
        ],
    )
}

fn exit_lobby(sender: Res<LobbySender>) {
    _ = sender.send(ClientToLobby::LeaveCurrentLobby);
}

fn llll(entity: Entity) -> impl ObserverSystem<LobbyInfoReceived, ()> {
    IntoObserverSystem::<_, (), _>::into_system(
        move |trigger: Trigger<LobbyInfoReceived>,
              my_id: Res<MyPlayerId>,
              mut commands: Commands| {
            let leader = trigger.event().0.leader;
            let my_id = my_id.0;
            commands.queue(move |world: &mut World| {
                if let Ok(mut entity) = world.get_entity_mut(entity) {
                    entity.despawn_related::<Children>();
                    if my_id == leader {
                        entity.with_child(lobby_settings());
                    }
                }
            });
        },
    )
}

#[derive(Component)]
struct LobbySettingsAnchor;

fn lobby_settings() -> impl Bundle {
    (
        Node {
            flex_direction: FlexDirection::Row,
            ..default()
        },
        Name::new("Lobby Settings"),
        children![
            button2(
                "Teams - ",
                |current_info: Res<CurrentLobbyInfo>, sender: Res<LobbySender>| {
                    let settings = current_info.0.settings.clone();
                    let _ = sender.0.send(ClientToLobby::SetLobbySettings({
                        LobbySettings {
                            team_count: settings.team_count.saturating_sub(1),
                            ..settings
                        }
                    }));
                }
            ),
            button2(
                "Teams + ",
                |current_info: Res<CurrentLobbyInfo>, sender: Res<LobbySender>| {
                    let settings = current_info.0.settings.clone();
                    let _ = sender.0.send(ClientToLobby::SetLobbySettings({
                        LobbySettings {
                            team_count: settings.team_count + 1,
                            ..settings
                        }
                    }));
                }
            ),
            button2("Start Game", send_msg(ClientToLobby::GoToChampSelect)),
        ],
    )
}

fn dual_team(
    my_id: PlayerId,
    left: (Team, Vec<PlayerId>),
    right: Option<(Team, Vec<PlayerId>)>,
    max_players: usize,
) -> impl Bundle {
    (
        Node {
            flex_direction: FlexDirection::Row,
            justify_content: JustifyContent::SpaceBetween,
            ..default()
        },
        Name::new(format!(
            "Dual Team {}-{}",
            left.0.0,
            right.as_ref().map(|x| x.0.0).unwrap_or_default()
        )),
        Children::spawn(SpawnIter(
            std::iter::once(left)
                .chain(right)
                .map(move |(t, p)| team(my_id, t, p, max_players)),
        )),
    )
}

#[derive(Component)]
struct TeamMovementButton(Team);

#[derive(Component)]
struct TeamList(Team);

fn team(my_id: PlayerId, team: Team, players: Vec<PlayerId>, max_players: usize) -> impl Bundle {
    let n = max_players - players.len();
    (
        Node {
            flex_direction: FlexDirection::Column,
            width: Val::Percent(40.0),
            ..default()
        },
        (Name::new(format!("Team {}", team.0))),
        children![
            (
                Node::default(),
                children![
                    text(format!("Team {}", team.0)),
                    (
                        Node {
                            display: if players.contains(&my_id) {
                                Display::None
                            } else {
                                Display::Flex
                            },
                            ..default()
                        },
                        button2("Move to this team", move_to_team(team)),
                        GlobalObserver::new(move |entity| {
                            move |trigger: Trigger<PlayerChangedTeam>,
                                  mut node: Query<&mut Node>,
                                  my_id: Res<MyPlayerId>,
                                  mut commands: Commands| {
                                if trigger.event().0 != my_id.0 {
                                    return;
                                }
                                let Ok(mut node) = node.get_mut(entity) else {
                                    commands.entity(trigger.observer()).despawn();
                                    return;
                                };
                                node.display = if trigger.event().1 == team {
                                    Display::None
                                } else {
                                    Display::Flex
                                };
                            }
                        })
                    )
                ]
            ),
            (
                Node {
                    flex_direction: FlexDirection::Column,
                    ..default()
                },
                Children::spawn(SpawnIter(
                    players
                        .into_iter()
                        .map(Some)
                        .chain(std::iter::repeat_n(None, n))
                        .enumerate()
                        .map(move |(i, p)| player_slot::player_slot(p, team, i))
                ))
            ),
        ],
    )
}

fn move_to_team(team: Team) -> impl System<In = (), Out = ()> {
    IntoSystem::<_, (), _>::into_system(move |my_id: Res<MyPlayerId>, sender: Res<LobbySender>| {
        _ = sender.send(ClientToLobby::ChangePlayerTeam(my_id.0, team));
    })
}

#[derive(Resource)]
pub struct CurrentLobbyInfo(pub LobbyInfo);

fn on_lobby_info_update(
    trigger: Trigger<LobbyInfoReceived>,
    current: Option<Res<CurrentLobbyInfo>>,
    player_cache: Res<PlayerInfoCache>,
    anchor: Single<Entity, With<LobbyTeamHorizontalContainer>>,
    sender: Res<LobbySender>,
    my_id: Res<MyPlayerId>,
    mut commands: Commands,
) {
    let info = &trigger.event().0;
    let max_players = info.settings.max_players_per_team;
    commands.insert_resource(CurrentLobbyInfo(info.clone()));

    // for player in info.teams.iter().flatten() {
    //     if !player_cache.cache.contains_key(player) {
    //         info!("Updating player cache for {player:?}");
    //         let _ = sender.0.send(ClientToLobby::GetPlayerInfo(*player));
    //     }
    // }

    // if current.is_none() {
    // Update from scratch
    // Kill children of anchor
    let mut iter = info
        .teams
        .iter()
        .enumerate()
        .map(|(a, b)| (Team(a), b.clone()))
        .array_chunks::<2>();
    let duals = iter.by_ref().collect::<Vec<_>>();
    let rest = iter.into_remainder().into_iter().flatten().next();

    let mut entity = commands.entity(*anchor);
    let my_id = my_id.0;
    entity
        .despawn_related::<Children>()
        .insert(Children::spawn(SpawnIter(
            duals
                .into_iter()
                .map(move |[a, b]| dual_team(my_id, a, Some(b), max_players)),
        )));
    if let Some(rest) = rest {
        entity.with_related::<ChildOf>(dual_team(my_id, rest, None, max_players));
    }

    // let mut settings = commands.entity(*settings_anchor);
    // settings.despawn_related::<Children>();
    // if my_id == info.leader {
    //     settings.insert(children![lobby_settings()]);
    // }
    // }
    // commands.run_system_cached(on_leader_changed_team);
}

#[derive(Resource)]
struct PlayerInfoCache {
    cache: HashMap<PlayerId, PlayerInfo>,
}

fn on_player_info_received(
    trigger: Trigger<PlayerInfoReceived>,
    mut cache: ResMut<PlayerInfoCache>,
    mut commands: Commands,
) {
    let event = trigger.event();
    info!("Player info received: {:?}", event.0);
    cache
        .cache
        .insert(trigger.event().0.id, trigger.event().0.clone());
    commands.run_system_cached_with(
        player_slot::update_lobby_player_names,
        trigger.event().0.clone(),
    );
}

fn on_player_swap_team(
    trigger: Trigger<PlayerChangedTeam>,
    mut cur: ResMut<CurrentLobbyInfo>,
    slot_map: Res<PlayerSlotMap>,
    mut commands: Commands,
) {
    let event = trigger.event();
    let PlayerChangedTeam(moving_player, to_team) = *event;
    let Some((from_team, index)) = cur.0.teams.iter().enumerate().find_map(|(team, players)| {
        players
            .iter()
            .position(|p| *p == moving_player)
            .map(|i| (Team(team), i))
    }) else {
        return;
    };

    let players = &cur.0.teams[from_team.0];
    for i in index..players.len() - 1 {
        let event = PlayerMovedToThisSlot(players[i + 1]);
        commands.trigger_targets(event, slot_map.get(from_team, i).unwrap());
    }

    let event = PlayerMovedFromThisSlot;
    commands.trigger_targets(event, slot_map.get(from_team, players.len() - 1).unwrap());

    if let Some(x) = slot_map.get(to_team, cur.0.teams[to_team.0].len()) {
        commands.trigger_targets(PlayerMovedToThisSlot(moving_player), x);
    } else {
        commands.run_system_cached_with(
            spawn_player_slot,
            (to_team, cur.0.teams[to_team.0].len(), moving_player),
        );
    }

    cur.0.teams[from_team.0].remove(index);
    cur.0.teams[to_team.0].push(moving_player);
}

fn spawn_player_slot(
    inp: In<(Team, usize, PlayerId)>,
    q: Query<(Entity, &TeamList)>,
    mut commands: Commands,
) {
    let (team, index, player) = *inp;

    for (e, list) in &q {
        if list.0 != inp.0.0 {
            continue;
        }

        commands
            .entity(e)
            .with_child(player_slot::player_slot(Some(player), team, index));
    }
}

fn on_player_swap_positions(
    trigger: Trigger<PlayerChangedPositions>,
    mut cur: ResMut<CurrentLobbyInfo>,
    slot_map: Res<PlayerSlotMap>,
    mut commands: Commands,
) {
    let event = trigger.event();
    let PlayerChangedPositions(player_a, player_b) = *event;
    let Some((team_a, index_a)) = cur.0.teams.iter().enumerate().find_map(|(team, players)| {
        players
            .iter()
            .position(|p| *p == player_a)
            .map(|i| (Team(team), i))
    }) else {
        return;
    };
    let Some((team_b, index_b)) = cur.0.teams.iter().enumerate().find_map(|(team, players)| {
        players
            .iter()
            .position(|p| *p == player_b)
            .map(|i| (Team(team), i))
    }) else {
        return;
    };

    commands.trigger_targets(
        PlayerMovedToThisSlot(player_b),
        slot_map.get(team_a, index_a).unwrap(),
    );
    commands.trigger_targets(
        PlayerMovedToThisSlot(player_a),
        slot_map.get(team_b, index_b).unwrap(),
    );

    if team_a == team_b {
        cur.0.teams[team_a.0].swap(index_a, index_b);
    } else {
        let [a, b] = cur.0.teams.get_disjoint_mut([team_a.0, team_b.0]).unwrap();
        std::mem::swap(&mut a[index_a], &mut b[index_b]);
    }
}

fn on_we_left_lobby(trigger: Trigger<WeLeftLobby>, mut commands: Commands) {
    commands.set_state(LobbyMenuState::LobbyList);
}

fn update_lobby_on<E: Event>(trigger: Trigger<E>, mut commands: Commands) {
    commands.run_system_cached(update_lobby_info);
}

fn update_lobby_info(lobby: Res<CurrentLobbyInfo>, sender: Res<LobbySender>) {
    _ = sender.0.send(ClientToLobby::GetLobbyInfo(lobby.0.short.id));
}
