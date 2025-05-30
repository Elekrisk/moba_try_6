#![feature(never_type)]

use std::{borrow::Borrow, collections::HashMap, path::PathBuf};

use anyhow::{Result, anyhow, bail};
use lobby_common::{
    ClientToLobby, LobbyId, LobbyInfo, LobbyShortInfo, LobbyToClient, PlayerId, PlayerInfo, Team,
};
use serde::{Deserialize, Serialize};
#[cfg(not(target_family = "wasm"))]
use tokio::{
    io::AsyncReadExt as _,
    sync::mpsc::{UnboundedReceiver, UnboundedSender, unbounded_channel},
};
use uuid::Uuid;
#[cfg(not(target_family = "wasm"))]
use wtransport::{
    Connection, Endpoint, Identity, ServerConfig,
    endpoint::{IncomingSession, endpoint_side::Server},
};

#[derive(clap::Parser)]
struct Options {
    #[arg(long)]
    certificate: Option<PathBuf>,
    #[arg(long)]
    privkey: Option<PathBuf>,
}

#[cfg(not(target_family = "wasm"))]
enum InternalMessage {
    NewPlayer(Player),
    PlayerMessage {
        player: PlayerId,
        message: ClientToLobby,
    },
    PlayerDisconnected(PlayerId),
    GameServerClosed(LobbyId),
    GameTokenCreated(PlayerId, Vec<u8>),
}

#[cfg(target_family = "wasm")]
fn main() {
    panic!("Cannot be run on the web");
}

#[cfg(not(target_family = "wasm"))]
#[tokio::main]
async fn main() {
    use clap::Parser;

    let options = Options::parse();

    let identity = match (options.certificate, options.privkey) {
        (Some(cert_pemfile), Some(private_key_pemfile)) => {
            println!("Using cert from disk");
            Identity::load_pemfiles(cert_pemfile, private_key_pemfile)
                .await
                .unwrap()
        }
        (None, None) => {
            println!("Using self-signed cert");
            Identity::self_signed(["localhost", "127.0.0.1", "::1"]).unwrap()
        }
        _ => {
            eprintln!("Specifying certificate or privkey also requires the other");
            return;
        }
    };

    let config = ServerConfig::builder()
        .with_bind_default(54654)
        .with_identity(identity)
        .max_idle_timeout(None)
        .unwrap()
        .build();

    let endpoint = Endpoint::server(config).unwrap();

    let (sender, mut r) = unbounded_channel();

    let s = sender.clone();

    tokio::spawn(async move {
        match server_loop(endpoint, s).await {
            Ok(()) => {}
            Err(err) => eprintln!("Server error: {err:?}"),
        }
    });

    let mut state = State::new(sender);

    loop {
        match state.handle(&mut r).await {
            Ok(()) => {}
            Err(err) => eprintln!("Error: {err:?}"),
        }
    }
}

#[cfg(not(target_family = "wasm"))]
use wee::*;

#[cfg(not(target_family = "wasm"))]
mod wee {
    use std::{collections::HashSet, process::Command};

    use engine_common::ChampionId;
    use lobby_common::{
        ChampionSelection, LobbySettings, LobbyState, LobbyToServer, PlayerGameInfo, ServerToLobby,
    };
    use wtransport::ClientConfig;

    use super::*;

    pub async fn server_loop(
        endpoint: Endpoint<Server>,
        s: UnboundedSender<InternalMessage>,
    ) -> Result<()> {
        loop {
            let incoming = endpoint.accept().await;
            tokio::spawn(handle_connection(incoming, s.clone()));
        }
    }

    pub async fn handle_connection(
        incoming: IncomingSession,
        s: UnboundedSender<InternalMessage>,
    ) -> Result<()> {
        println!("Incoming connection");
        let connection = incoming.await?;
        println!("Accepting connection...");
        let connection = connection.accept().await?;
        println!("Connection accepted");
        println!("Waiting for application  handshake...");
        match connection.recv::<ClientToLobby>().await {
            Ok(ClientToLobby::Handshake { name }) => {
                println!("Application handshake received");
                let player_id = PlayerId(Uuid::new_v4());
                println!("Sending handshake response...");
                connection
                    .send(LobbyToClient::Handshake { id: player_id })
                    .await?;
                println!("Handshake response sent");
                let player = Player {
                    id: player_id,
                    name,
                    current_lobby: None,
                    connection: connection.clone(),
                };
                s.send(InternalMessage::NewPlayer(player))?;
                tokio::spawn(listen_to_connection(player_id, connection, s.clone()));
            }
            Ok(other) => {
                eprintln!("Error receiving handshake: unexpected message {other:?}");
            }
            Err(err) => {
                eprintln!("Error receiving handshake: {err}");
            }
        }

        Ok(())
    }

    pub async fn listen_to_connection(
        player: PlayerId,
        connection: Connection,
        s: UnboundedSender<InternalMessage>,
    ) -> Result<()> {
        loop {
            match connection.recv::<ClientToLobby>().await {
                Ok(ClientToLobby::Handshake { .. }) => {
                    // Invalid message, ignore
                }
                Ok(message) => s.send(InternalMessage::PlayerMessage { player, message })?,
                Err(e) => {
                    eprintln!("Read failed: {e}");
                    s.send(InternalMessage::PlayerDisconnected(player))?;
                    break;
                }
            }
        }

        Ok(())
    }

    pub struct Player {
        pub id: PlayerId,
        pub name: String,
        pub current_lobby: Option<LobbyId>,
        pub connection: Connection,
    }

    impl Player {
        fn get_info(&self) -> PlayerInfo {
            PlayerInfo {
                id: self.id,
                name: self.name.clone(),
            }
        }
    }

    pub struct Lobby {
        pub id: LobbyId,
        pub settings: LobbySettings,
        pub teams: Vec<Vec<PlayerId>>,
        pub leader: PlayerId,
        pub lobby_state: LobbyState,
        pub selected_champs: HashMap<PlayerId, ChampionSelection>,
    }

    impl Lobby {
        fn player_count(&self) -> usize {
            self.teams.iter().map(Vec::len).sum()
        }

        fn is_empty(&self) -> bool {
            self.teams.iter().all(Vec::is_empty)
        }

        fn get_short_info(&self) -> LobbyShortInfo {
            LobbyShortInfo {
                id: self.id,
                name: self.settings.name.clone(),
                player_count: self.player_count(),
                max_player_count: self.settings.max_players_per_team * self.teams.len(),
            }
        }

        fn get_info(&self) -> LobbyInfo {
            LobbyInfo {
                short: self.get_short_info(),
                settings: self.settings.clone(),
                teams: self.teams.clone(),
                leader: self.leader,
                lobby_state: self.lobby_state,
                selected_champs: self.selected_champs.clone(),
            }
        }

        fn add_player(&mut self, player: PlayerId) {
            // Find team with lowest amount of players
            let lowest_team = self.teams.iter_mut().min_by_key(|x| x.len()).unwrap();
            lowest_team.push(player);
        }

        /// Readjusts players so that no team has more players than they are allowed to,
        /// if possible.
        fn readjust_players(&mut self) {
            let teams = (0..self.settings.team_count).map(Team).collect::<Vec<_>>();

            if self.teams.len() > self.settings.team_count {
                for from_team in (self.settings.team_count..self.teams.len()).rev().map(Team) {
                    let players = &self.teams[from_team.0];
                    // Players need to be moved from this team
                    'outer: for _ in 0..players.len() {
                        for &to_team in &teams {
                            if self.teams[to_team.0].len() < self.settings.max_players_per_team {
                                // We can move them here
                                let player = self.teams[from_team.0].pop().unwrap();
                                self.teams[to_team.0].push(player);
                                continue 'outer;
                            }
                        }
                        // We could not find a team with space, just find the one with
                        // the least amount of players
                        if let Some(team_to_move_to) =
                            teams.iter().min_by_key(|t| self.teams[t.0].len())
                        {
                            let player = self.teams[from_team.0].pop().unwrap();
                            self.teams[team_to_move_to.0].push(player);
                        } else {
                            // There are no teams :(
                            panic!("Lobby without teams is invalid");
                        }
                    }
                }
                self.teams.pop();
            }
            if self.teams.len() < self.settings.team_count {
                for team in (self.teams.len()..self.settings.team_count).map(Team) {
                    self.teams.push(vec![]);
                }
            }
            let teams = (0..self.settings.team_count).map(Team).collect::<Vec<_>>();

            for &from_team in &teams {
                let players = &self.teams[from_team.0];
                if players.len() > self.settings.max_players_per_team {
                    // Players need to be moved from this team
                    let amount_to_move = players.len() - self.settings.max_players_per_team;
                    for _ in 0..amount_to_move {
                        for &to_team in &teams {
                            if self.teams[to_team.0].len() < self.settings.max_players_per_team {
                                // We can move them here
                                let player = self.teams[from_team.0].pop().unwrap();
                                self.teams[to_team.0].push(player);
                                break;
                            }
                        }
                    }
                }
            }
        }

        fn needs_readjustment(&self) -> bool {
            self.teams.len() != self.settings.team_count
                || self
                    .teams
                    .iter()
                    .any(|t| t.len() > self.settings.max_players_per_team)
                    && !self
                        .teams
                        .iter()
                        .all(|t| t.len() > self.settings.max_players_per_team)
        }

        fn readjust_if_needed(&mut self) {
            if self.needs_readjustment() {
                self.readjust_players();
            }
        }

        fn remove_player(&mut self, player_id: PlayerId, only_temporarily: bool) {
            for team in self.teams.iter_mut() {
                if let Some(pos) = team.iter().position(|p| *p == player_id) {
                    team.remove(pos);
                    break;
                }
            }
            if !only_temporarily {
                if self.leader == player_id
                    && let Some(player) = self.teams.iter().flatten().next()
                {
                    self.leader = *player;
                }
                self.readjust_if_needed();
            }
        }
    }

    pub struct State {
        players: HashMap<PlayerId, Player>,
        used_player_names: HashSet<String>,
        lobbies: HashMap<LobbyId, Lobby>,
        sender: UnboundedSender<InternalMessage>,
    }

    impl State {
        pub fn new(sender: UnboundedSender<InternalMessage>) -> Self {
            Self {
                players: HashMap::new(),
                used_player_names: HashSet::new(),
                lobbies: HashMap::new(),
                sender,
            }
        }

        pub async fn handle(&mut self, r: &mut UnboundedReceiver<InternalMessage>) -> Result<()> {
            match r.recv().await.ok_or(anyhow!("Reading failed"))? {
                InternalMessage::NewPlayer(mut player) => {
                    println!("Player connected: {:?}", player.id.0);
                    // Find unused username
                    let mut i = 1;
                    let mut name = player.name.clone();
                    while self.used_player_names.contains(&name) {
                        i += 1;
                        name = format!("{} {i}", player.name);
                        println!("Incrementing name to {name}");
                    }
                    player.name = name.clone();
                    self.players.insert(player.id, player);
                    self.used_player_names.insert(name);
                }
                InternalMessage::PlayerDisconnected(player_id)
                | InternalMessage::PlayerMessage {
                    player: player_id,
                    message: ClientToLobby::Disconnect,
                } => {
                    println!("Player disconnected: {player_id:?}");
                    let _ = self.handle_player_left(player_id);
                    if let Some(player) = self.players.remove(&player_id) {
                        self.used_player_names.remove(&player.name);
                    }
                }
                InternalMessage::PlayerMessage { player, message } => {
                    self.handle_player_message(player, message).await?;
                }
                InternalMessage::GameServerClosed(lobby_id) => {
                    if let Some(lobby) = self.lobbies.get_mut(&lobby_id) {
                        lobby.lobby_state = LobbyState::InLobby;
                        lobby.selected_champs.clear();
                        _ = self.broadcast_message(
                            lobby_id,
                            None,
                            LobbyToClient::ReturnFromChampSelect,
                        );
                    }
                }
                InternalMessage::GameTokenCreated(player_id, token) => {
                    _ = self.send_message(player_id, LobbyToClient::GameStarted(token));
                }
            }

            Ok(())
        }

        async fn handle_player_message(
            &mut self,
            player_id: PlayerId,
            message: ClientToLobby,
        ) -> Result<()> {
            match message {
                ClientToLobby::Handshake { .. } => unreachable!(),
                ClientToLobby::FetchLobbyList => {
                    let _ = self.send_message(
                        player_id,
                        LobbyToClient::LobbyList(
                            self.lobbies.values().map(Lobby::get_short_info).collect(),
                        ),
                    );
                }
                ClientToLobby::CreateAndJoinLobby => {
                    let player = self
                        .players
                        .get_mut(&player_id)
                        .ok_or(anyhow!("Invalid player"))?;

                    if player.current_lobby.is_some() {
                        bail!("Player is already in lobby");
                    }

                    let lobby_id = LobbyId(Uuid::new_v4());
                    let lobby = Lobby {
                        id: lobby_id,
                        settings: LobbySettings {
                            name: format!("{}'s lobby", player.name),
                            locked: false,
                            team_count: 2,
                            max_players_per_team: 5,
                        },
                        teams: [vec![player_id], vec![]].into_iter().collect(),
                        leader: player_id,
                        lobby_state: LobbyState::InLobby,
                        selected_champs: HashMap::new(),
                    };

                    self.lobbies.insert(lobby_id, lobby);

                    player.current_lobby = Some(lobby_id);

                    let _ = self.send_message(player_id, LobbyToClient::YouJoinedLobby(lobby_id));
                }
                ClientToLobby::JoinLobby(lobby_id) => {
                    let player = self
                        .players
                        .get_mut(&player_id)
                        .ok_or(anyhow!("Invalid player"))?;

                    if player.current_lobby.is_some() {
                        bail!("Player is already in lobby");
                    }

                    let lobby = self
                        .lobbies
                        .get_mut(&lobby_id)
                        .ok_or(anyhow!("Lobby doesn't exist"))?;

                    if lobby.settings.locked {
                        bail!("Lobby is locked");
                    }

                    if lobby.player_count() >= lobby.settings.max_players() {
                        bail!("Lobby is full");
                    }

                    if lobby.lobby_state != LobbyState::InLobby {
                        bail!("Lobby is in champ select or in game");
                    }

                    // Add player to lobby
                    lobby.add_player(player_id);
                    player.current_lobby = Some(lobby_id);

                    let _ = self.send_message(player_id, LobbyToClient::YouJoinedLobby(lobby_id));
                    let _ = self.broadcast_message(
                        lobby_id,
                        player_id,
                        LobbyToClient::PlayerJoinedLobby(player_id),
                    );
                }
                ClientToLobby::LeaveCurrentLobby => {
                    self.handle_player_left(player_id)?;
                }
                ClientToLobby::GetLobbyInfo(lobby_id) => {
                    let Some(lobby) = self.lobbies.get(&lobby_id) else {
                        bail!("Lobby doesn't exist")
                    };

                    let _ =
                        self.send_message(player_id, LobbyToClient::LobbyInfo(lobby.get_info()));
                }
                ClientToLobby::GetPlayerInfo(req_player_id) => {
                    let Some(player) = self.players.get(&req_player_id) else {
                        bail!("Player doesn't exist")
                    };

                    let _ =
                        self.send_message(player_id, LobbyToClient::PlayerInfo(player.get_info()));
                }
                ClientToLobby::SetLobbySettings(mut lobby_settings) => {
                    let Some(player) = self.players.get(&player_id) else {
                        bail!("Player doesn't exist");
                    };
                    let Some(lobby_id) = player.current_lobby else {
                        bail!("Player is not in a lobby");
                    };
                    let Some(lobby) = self.lobbies.get_mut(&lobby_id) else {
                        bail!("Lobby doesn't exist");
                    };
                    if lobby.leader != player_id {
                        bail!("Player is not lobby leader");
                    }
                    lobby_settings.team_count = lobby_settings.team_count.max(1);
                    lobby_settings.max_players_per_team =
                        lobby_settings.max_players_per_team.max(1);
                    lobby.settings = lobby_settings;
                    lobby.readjust_if_needed();
                    let info = lobby.get_info();
                    let _ = self.broadcast_message(lobby_id, None, LobbyToClient::LobbyInfo(info));
                }
                ClientToLobby::Disconnect => unreachable!(),
                ClientToLobby::ChangePlayerTeam(player_to_move, team) => {
                    let Some(player) = self.players.get(&player_id) else {
                        bail!("Player doesn't exist");
                    };
                    let Some(lobby_id) = player.current_lobby else {
                        bail!("Player is not in a lobby");
                    };
                    let Some(lobby) = self.lobbies.get_mut(&lobby_id) else {
                        bail!("Lobby doesn't exist");
                    };
                    if player_to_move != player_id && lobby.leader != player_id {
                        bail!("Player is not lobby leader");
                    }
                    if lobby.teams.len() <= team.0 {
                        bail!("Invalid team");
                    }
                    if lobby.teams[team.0].len() >= lobby.settings.max_players_per_team {
                        bail!("Team is full");
                    }

                    lobby.remove_player(player_to_move, true);
                    lobby.teams[team.0].push(player_to_move);

                    _ = self.broadcast_message(
                        lobby_id,
                        None,
                        LobbyToClient::PlayerChangedTeam(player_to_move, team),
                    );
                }
                ClientToLobby::SwitchPlayerPositions(player_a_id, player_b_id) => {
                    let Some(player) = self.players.get(&player_id) else {
                        bail!("Player doesn't exist");
                    };
                    let Some(lobby_id) = player.current_lobby else {
                        bail!("Player is not in a lobby");
                    };
                    let Some(lobby) = self.lobbies.get_mut(&lobby_id) else {
                        bail!("Lobby doesn't exist");
                    };
                    if lobby.leader != player_id {
                        bail!("Player is not lobby leader");
                    }
                    if player_a_id == player_b_id {
                        bail!("Cannot switch player to themselves");
                    }
                    let a = lobby
                        .teams
                        .iter()
                        .enumerate()
                        .find_map(|(t, p)| {
                            p.iter()
                                .position(|p| *p == player_a_id)
                                .map(|i| (Team(t), i))
                        })
                        .ok_or(anyhow!("No player a in lobby"))?;
                    let b = lobby
                        .teams
                        .iter()
                        .enumerate()
                        .find_map(|(t, p)| {
                            p.iter()
                                .position(|p| *p == player_b_id)
                                .map(|i| (Team(t), i))
                        })
                        .ok_or(anyhow!("No player b in lobby"))?;

                    if a.0 == b.0 {
                        lobby.teams[a.0.0].swap(a.1, b.1);
                    } else {
                        let [at, bt] = lobby.teams.get_disjoint_mut([a.0.0, b.0.0]).unwrap();
                        std::mem::swap(&mut at[a.1], &mut bt[b.1]);
                    }

                    _ = self.broadcast_message(
                        lobby_id,
                        None,
                        LobbyToClient::PlayerChangedPositions(player_a_id, player_b_id),
                    );
                }
                ClientToLobby::KickPlayer(player_to_kick) => {
                    let Some(player) = self.players.get(&player_id) else {
                        bail!("Player doesn't exist");
                    };
                    let Some(lobby_id) = player.current_lobby else {
                        bail!("Player is not in a lobby");
                    };
                    let Some(lobby) = self.lobbies.get_mut(&lobby_id) else {
                        bail!("Lobby doesn't exist");
                    };
                    if lobby.leader != player_id {
                        bail!("Player is not lobby leader");
                    }
                    if lobby.teams.iter().all(|t| !t.contains(&player_to_kick)) {
                        bail!("Player to kick not in this lobby");
                    }

                    self.handle_player_left(player_to_kick)?;
                }
                ClientToLobby::GoToChampSelect => {
                    let Some(player) = self.players.get(&player_id) else {
                        bail!("Player doesn't exist");
                    };
                    let Some(lobby_id) = player.current_lobby else {
                        bail!("Player is not in a lobby");
                    };
                    let Some(lobby) = self.lobbies.get_mut(&lobby_id) else {
                        bail!("Lobby doesn't exist");
                    };
                    if lobby.leader != player_id {
                        bail!("Player is not lobby leader");
                    }

                    lobby.lobby_state = LobbyState::InChampSelect;

                    _ = self.broadcast_message(lobby_id, None, LobbyToClient::GoToChampSelect);
                }
                ClientToLobby::SelectChamp(champ) => {
                    let Some(player) = self.players.get(&player_id) else {
                        bail!("Player doesn't exist");
                    };
                    let Some(lobby_id) = player.current_lobby else {
                        bail!("Player is not in a lobby");
                    };
                    let Some(lobby) = self.lobbies.get_mut(&lobby_id) else {
                        bail!("Lobby doesn't exist");
                    };
                    let entry =
                        lobby
                            .selected_champs
                            .entry(player_id)
                            .or_insert(ChampionSelection {
                                id: ChampionId(Uuid::nil()),
                                locked: false,
                            });
                    if entry.locked {
                        bail!("Cannot change locked selection");
                    }

                    entry.id = champ;

                    _ = self.broadcast_message(
                        lobby_id,
                        None,
                        LobbyToClient::PlayerSelectedChamp(player_id, champ),
                    );
                }
                ClientToLobby::LockSelection => {
                    let Some(player) = self.players.get(&player_id) else {
                        bail!("Player doesn't exist");
                    };
                    let Some(lobby_id) = player.current_lobby else {
                        bail!("Player is not in a lobby");
                    };
                    let Some(lobby) = self.lobbies.get_mut(&lobby_id) else {
                        bail!("Lobby doesn't exist");
                    };
                    let Some(selection) = lobby.selected_champs.get_mut(&player_id) else {
                        bail!("Cannot lock non-existant selection");
                    };
                    if selection.locked {
                        bail!("Cannot lock locked selection");
                    }

                    selection.locked = true;

                    if lobby.selected_champs.len() == lobby.player_count()
                        && lobby.selected_champs.values().all(|s| s.locked)
                    {
                        self.start_game(lobby_id)?;
                    }
                    _ = self.broadcast_message(
                        lobby_id,
                        None,
                        LobbyToClient::PlayerLockedSelection(player_id),
                    );
                }
            }

            Ok(())
        }

        fn handle_player_left(&mut self, player_id: PlayerId) -> Result<()> {
            let player = self
                .players
                .get_mut(&player_id)
                .ok_or(anyhow!("Player doesn't exist"))?;
            let Some(lobby_id) = player.current_lobby else {
                return Ok(());
            };
            let lobby = self
                .lobbies
                .get_mut(&lobby_id)
                .ok_or(anyhow!("Lobby doesn't exist"))?;
            lobby.remove_player(player_id, false);
            player.current_lobby = None;

            let in_champ_select = lobby.lobby_state == LobbyState::InChampSelect;
            if in_champ_select {
                lobby.lobby_state = LobbyState::InLobby;
                lobby.selected_champs.clear();
            }

            if lobby.is_empty() {
                self.lobbies.remove(&lobby_id);
            } else {
                if in_champ_select {
                    _ = self.broadcast_message(
                        lobby_id,
                        None,
                        LobbyToClient::ReturnFromChampSelect,
                    );
                }
                let _ = self.broadcast_message(
                    lobby_id,
                    None,
                    LobbyToClient::PlayerLeftLobby(player_id),
                );
            }
            let _ = self.send_message(player_id, LobbyToClient::YouLeftLobby);

            Ok(())
        }

        fn send_message(&self, player: PlayerId, message: LobbyToClient) -> Result<()> {
            let connection = self
                .players
                .get(&player)
                .ok_or(anyhow!("No such player exists"))?
                .connection
                .clone();
            println!("Sending message to {player:?}: {message:?}");
            tokio::spawn(async move {
                if connection.send(message).await.is_err() {
                    println!("Error: Failed sending message");
                }
            });
            Ok(())
        }

        fn broadcast_message(
            &self,
            lobby: LobbyId,
            exclude_player: impl Into<Option<PlayerId>>,
            message: LobbyToClient,
        ) -> Result<()> {
            let exclude_player = exclude_player.into();
            for player in self
                .lobbies
                .get(&lobby)
                .ok_or(anyhow!("No such lobby exists"))?
                .teams
                .iter()
                .flatten()
                .copied()
            {
                if exclude_player == Some(player) {
                    continue;
                }

                let _ = self.send_message(player, message.clone());
            }

            Ok(())
        }

        fn start_game(&mut self, lobby_id: LobbyId) -> Result<()> {
            let lobby = self
                .lobbies
                .get_mut(&lobby_id)
                .ok_or(anyhow!("No such lobby"))?;
            if lobby.selected_champs.len() < lobby.player_count() {
                bail!("Not all players have selected a champion");
            }

            if lobby.selected_champs.values().any(|s| !s.locked) {
                bail!("Not all players have locked their selection");
            }

            println!("Starting game server...");
            // Start game in some way
            let mut child = Command::new("cargo")
                .args(["run", "--bin=server", "--", "--address", "127.0.0.1", "54655"])
                .spawn()?;

            lobby.lobby_state = LobbyState::InGame;

            let sender = self.sender.clone();

            tokio::spawn(async move {
                _ = child.wait();

                println!("Child just exited!");

                _ = sender.send(InternalMessage::GameServerClosed(lobby_id));
            });

            let settings = lobby.settings.clone();
            let lobby = &*lobby;
            let players = lobby
                .teams
                .iter()
                .enumerate()
                .flat_map(|(i, p)| {
                    p.iter().map(move |p| PlayerGameInfo {
                        id: *p,
                        team: Team(i),
                        champ: lobby.selected_champs.get(p).unwrap().id,
                    })
                })
                .collect();

            let sender = self.sender.clone();

            tokio::spawn(async move {
                // Connect to server
                let conn = Endpoint::client(
                    ClientConfig::builder()
                        .with_bind_default()
                        .with_no_cert_validation()
                        .build(),
                )
                .unwrap()
                .connect("https://localhost:54653")
                .await
                .unwrap();
                conn.send(LobbyToServer::Handshake { settings, players })
                    .await
                    .unwrap();
                let ServerToLobby::PlayerTokens { tokens } = conn.recv().await.unwrap();

                for (player, token) in tokens {
                    sender
                        .send(InternalMessage::GameTokenCreated(player, token))
                        .unwrap();
                }
            });

            Ok(())
        }
    }

    pub trait SendMessage {
        async fn send<T: Serialize>(&self, msg: T) -> anyhow::Result<()>;
    }

    impl SendMessage for Connection {
        async fn send<T: Serialize>(&self, msg: T) -> anyhow::Result<()> {
            let msg = serde_json::to_vec_pretty(&msg)?;
            self.open_uni().await?.await?.write_all(&msg).await?;
            Ok(())
        }
    }

    pub trait RecvMessage {
        async fn recv<T: for<'de> Deserialize<'de>>(&self) -> anyhow::Result<T>;
    }

    impl RecvMessage for Connection {
        async fn recv<T: for<'de> Deserialize<'de>>(&self) -> anyhow::Result<T> {
            let mut buf = vec![];
            self.accept_uni().await?.read_to_end(&mut buf).await?;
            Ok(serde_json::from_slice(&buf)?)
        }
    }
}
