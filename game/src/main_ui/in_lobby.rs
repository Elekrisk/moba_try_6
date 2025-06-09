use bevy::{ecs::system::ObserverSystem, platform::collections::HashMap, prelude::*};
use lobby_common::{ClientToLobby, LobbyInfo, LobbySettings, PlayerId, PlayerInfo, Team};

use crate::{
    Options,
    network::LobbySender,
    new_ui::{
        View, ViewExt,
        button::{ButtonCallback, ButtonView},
        container::ContainerView,
        list::ListView,
        subtree::SubtreeView,
        text::TextView,
    },
};

use super::{
    LobbyMenuState,
    lobby_list::{
        LobbyInfoReceived, MyPlayerId, PlayerChangedPositions, PlayerChangedTeam,
        PlayerInfoReceived, PlayerJoinedLobby, PlayerLeftLobby, PlayerLockedSelection,
        PlayerSelectedChamp, WeJoinedLobby, WeLeftLobby,
    },
    send_msg,
};

pub fn client(app: &mut App) {
    app.add_observer(on_lobby_info_update)
        .add_observer(setup)
        .add_observer(on_player_info_received)
        .add_observer(update_lobby_on::<PlayerJoinedLobby>)
        .add_observer(update_lobby_on::<PlayerLeftLobby>)
        .add_observer(update_lobby_on::<PlayerSelectedChamp>)
        .add_observer(update_lobby_on::<PlayerLockedSelection>)
        .add_observer(on_player_swap_team)
        .add_observer(on_player_swap_positions)
        .add_observer(on_we_left_lobby)
        .init_resource::<PlayerInfoCache>();
    if app.world().resource::<Options>().auto_start.is_some() {
        app.add_systems(
            Update,
            auto_start
                .run_if(in_state(LobbyMenuState::InLobby).and(resource_exists::<CurrentLobbyInfo>)),
        );
    }
}

pub fn setup(trigger: Trigger<WeJoinedLobby>, sender: Res<LobbySender>, mut commands: Commands) {
    commands.set_state(LobbyMenuState::InLobby);
    let _ = sender
        .0
        .send(ClientToLobby::GetLobbyInfo(trigger.event().0));
}

pub fn auto_start(
    mut options: ResMut<Options>,
    mut timer: Local<f32>,
    time: Res<Time>,
    info: Res<CurrentLobbyInfo>,
    sender: Res<LobbySender>,
) {
    if let Some(count) = options.auto_start
        && info.0.teams.iter().map(Vec::len).sum::<usize>() >= count
    {
        *timer += time.elapsed_secs();
        if count <= 1 || *timer > 1.0 {
            _ = sender.send(ClientToLobby::GoToChampSelect);
            options.auto_start = None;
        }
    }
}

#[derive(Resource)]
pub struct CurrentLobbyInfo(pub LobbyInfo);

fn on_lobby_info_update(trigger: Trigger<LobbyInfoReceived>, mut commands: Commands) {
    let info = &trigger.event().0;
    commands.insert_resource(CurrentLobbyInfo(info.clone()));
}

#[derive(Resource, Default)]
pub struct PlayerInfoCache {
    cache: HashMap<PlayerId, PlayerInfo>,
    last_fetch_times: HashMap<PlayerId, f32>,
}

impl PlayerInfoCache {
    // pub fn get(&self, id: PlayerId) -> Option<&PlayerInfo> {
    //     self.cache.get(&id)
    // }

    pub fn fetch(
        &mut self,
        id: PlayerId,
        sender: &LobbySender,
        time: &Time,
    ) -> Option<&PlayerInfo> {
        let now = time.delta_secs();
        if let Some(info) = self.cache.get(&id) {
            return Some(info);
        }
        if self.last_fetch_times.get(&id).is_none_or(|x| now - x > 5.0) {
            _ = sender.send(ClientToLobby::GetPlayerInfo(id));
            self.last_fetch_times.insert(id, now);
        }
        None
    }
}

fn on_player_info_received(
    trigger: Trigger<PlayerInfoReceived>,
    mut cache: ResMut<PlayerInfoCache>,
) {
    let event = &trigger.event().0;
    cache.cache.insert(event.id, event.clone());
}

fn on_player_swap_team(trigger: Trigger<PlayerChangedTeam>, mut cur: ResMut<CurrentLobbyInfo>) {
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
    cur.0.teams[from_team.0].remove(index);
    cur.0.teams[to_team.0].push(moving_player);
}

fn on_player_swap_positions(
    trigger: Trigger<PlayerChangedPositions>,
    mut cur: ResMut<CurrentLobbyInfo>,
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

    if team_a == team_b {
        cur.0.teams[team_a.0].swap(index_a, index_b);
    } else {
        let [a, b] = cur.0.teams.get_disjoint_mut([team_a.0, team_b.0]).unwrap();
        std::mem::swap(&mut a[index_a], &mut b[index_b]);
    }
}

fn on_we_left_lobby(_trigger: Trigger<WeLeftLobby>, mut commands: Commands) {
    commands.set_state(LobbyMenuState::LobbyList);
}

fn update_lobby_on<E: Event>(_trigger: Trigger<E>, mut commands: Commands) {
    commands.run_system_cached(update_lobby_info);
}

fn update_lobby_info(lobby: Res<CurrentLobbyInfo>, sender: Res<LobbySender>) {
    _ = sender.0.send(ClientToLobby::GetLobbyInfo(lobby.0.short.id));
}

pub fn lobby_ui2() -> impl View {
    SubtreeView::run_if("in_lobby_ui", lobby_ui3, |world| {
        world.contains_resource::<CurrentLobbyInfo>()
    })
    .styled()
    .width(Val::Percent(100.0))
}

pub fn lobby_ui3(info: Res<CurrentLobbyInfo>, my_id: Res<MyPlayerId>) -> Option<impl View + use<>> {
    if !info.is_changed() {
        return None;
    }

    let info = &info.0;
    let lobby_title = TextView::new(&info.short.name);
    let leave_button = ButtonView::new(
        TextView::new("Leave lobby"),
        "leave_lobby",
        send_msg(ClientToLobby::LeaveCurrentLobby),
    );

    let top_bar = ListView::new()
        .with(lobby_title)
        .with(leave_button)
        .styled()
        .column_gap(Val::Px(10.0))
        .align_items(AlignItems::Baseline)
        .justify_content(JustifyContent::SpaceBetween);

    let teams = team_list(&info.teams, info, my_id.0);

    let i_am_leader = info.leader == my_id.0;

    let root = ListView::new()
        .with(top_bar)
        .with(i_am_leader.then(|| lobby_settings2(&info.settings)))
        .with(teams)
        .styled()
        .flex_direction(FlexDirection::Column)
        .width(Val::Percent(100.0));

    Some(root)
}

fn lobby_settings2(settings: &LobbySettings) -> impl View {
    ListView::new()
        .with(ButtonView::new(
            "Teams -",
            "teams -",
            edit_settings(|s| s.team_count -= 1),
        ))
        .with(settings.team_count.to_string())
        .with(ButtonView::new(
            "Teams +",
            "teams +",
            edit_settings(|s| s.team_count += 1),
        ))
        .with(ButtonView::new(
            "Players -",
            "players -",
            edit_settings(|s| s.max_players_per_team -= 1),
        ))
        .with(settings.max_players_per_team.to_string())
        .with(ButtonView::new(
            "Players +",
            "players +",
            edit_settings(|s| s.max_players_per_team += 1),
        ))
        .with(ButtonView::new(
            "Start Game",
            "start game",
            send_msg(ClientToLobby::GoToChampSelect),
        ))
        .styled()
        .align_items(AlignItems::Baseline)
        .column_gap(Val::Px(20.0))
}

fn edit_settings(
    callback: impl Fn(&mut LobbySettings) + Send + Sync + 'static,
) -> impl ObserverSystem<Pointer<Click>, ()> {
    ButtonCallback::to_observer_system(
        move |lobby: Res<CurrentLobbyInfo>, sender: Res<LobbySender>| {
            let mut settings = lobby.0.settings.clone();
            callback(&mut settings);
            _ = sender.send(ClientToLobby::SetLobbySettings(settings));
        },
    )
}

fn team_list(teams: &Vec<Vec<PlayerId>>, lobby: &LobbyInfo, my_id: PlayerId) -> impl View {
    let team_pairs = teams.chunks(2).enumerate().map(|(i, slice)| {
        slice
            .iter()
            .enumerate()
            .map(move |(j, p)| (Team(i * 2 + j), p))
    });

    let mut list = ListView::new();

    for pair in team_pairs {
        list.add(team_pair(pair, lobby, my_id));
    }

    list.styled()
        .flex_direction(FlexDirection::Column)
        .flex_grow(1.0)
        .flex_basis(Val::Px(0.0))
        .scrollable()
}

fn team_pair<'a>(
    mut pair: impl Iterator<Item = (Team, &'a Vec<PlayerId>)>,
    lobby: &LobbyInfo,
    my_id: PlayerId,
) -> impl View {
    let mut list = ListView::new();
    let (left_team, left_players) = pair.next().unwrap();
    let right = pair.next();
    debug_assert!(pair.next().is_none());
    list.add(team2(left_team, left_players, lobby, my_id, false));
    if let Some((right_team, right_players)) = right {
        list.add(team2(right_team, right_players, lobby, my_id, true));
    }

    list.styled().justify_content(JustifyContent::SpaceBetween)
}

fn team2(
    team: Team,
    players: &[PlayerId],
    lobby: &LobbyInfo,
    my_id: PlayerId,
    right: bool,
) -> impl View {
    let mut top_bar = ListView::new().with(TextView::new(format!("Team {}", team.0)));
    if !players.contains(&my_id) {
        top_bar.add(ButtonView::new(
            TextView::new(format!("Move here")),
            format!("move_team_{}", team.0),
            send_msg(ClientToLobby::ChangePlayerTeam(my_id, team)),
        ));
    }

    let mut top_bar = top_bar
        .styled()
        .justify_content(JustifyContent::SpaceBetween)
        .align_items(AlignItems::Center)
        .height(Val::Px(45.0));
    if right {
        top_bar = top_bar.flex_direction(FlexDirection::RowReverse);
    }

    let mut player_slot_list = ListView::new();

    for player in players.iter().copied().map(Some).chain(std::iter::repeat_n(
        None,
        lobby
            .settings
            .max_players_per_team
            .saturating_sub(players.len()),
    )) {
        player_slot_list.add(player_slot(player))
    }

    ListView::new()
        .with(top_bar)
        .with(
            player_slot_list
                .styled()
                .flex_direction(FlexDirection::Column),
        )
        .styled()
        .flex_direction(FlexDirection::Column)
        .min_width(Val::Percent(40.0))
}

fn player_slot(player: Option<PlayerId>) -> impl View {
    let container = ContainerView::new(player.map(player_slot_content));
    container
        .styled()
        .border_color(Color::WHITE)
        .border(UiRect::all(Val::Px(1.0)))
        .height(Val::Px(45.0))
        .align_items(AlignItems::Center)
}

fn player_slot_content(player: PlayerId) -> impl View {
    SubtreeView::new(
        format!("player_slot_{}", player.0),
        move |mut cache: ResMut<PlayerInfoCache>,
              lobby: Res<CurrentLobbyInfo>,
              sender: Res<LobbySender>,
              time: Res<Time>,
              my_id: Res<MyPlayerId>| {
            if !cache.is_changed() && !lobby.is_changed() {
                return None;
            }

            let this_player = cache.fetch(player, &sender, &time)?;

            let i_am_leader = lobby.0.leader == my_id.0;
            let is_leader = lobby.0.leader == player;
            let this_is_me = my_id.0 == player;

            let can_kick = i_am_leader && !this_is_me;

            let view = ListView::new()
                .with(is_leader.then(|| TextView::new("[L]")))
                .with(TextView::new(&this_player.name).styled().flex_grow(1.0))
                .with(can_kick.then(|| {
                    ButtonView::new(
                        TextView::new("Kick"),
                        format!("kick_{}", player.0),
                        send_msg(ClientToLobby::KickPlayer(player)),
                    )
                }))
                .styled()
                .column_gap(Val::Px(10.0))
                .flex_grow(1.0)
                .align_items(AlignItems::Baseline)
                .padding(UiRect::all(Val::Px(2.0)));
            Some(view)
        },
    )
    .styled()
    .width(Val::Percent(100.0))
    // .styled()
    // .flex_grow(1.0)
}
