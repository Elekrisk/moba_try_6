use std::collections::HashMap;

use engine_common::ChampionId;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum LobbyToClient {
    Handshake { id: PlayerId },
    LobbyList(Vec<LobbyShortInfo>),
    LobbyInfo(LobbyInfo),
    YouJoinedLobby(LobbyId),
    YouLeftLobby,
    PlayerJoinedLobby(PlayerId),
    PlayerLeftLobby(PlayerId),
    PlayerInfo(PlayerInfo),
    PlayerChangedTeam(PlayerId, Team),
    PlayerChangedPositions(PlayerId, PlayerId),
    GoToChampSelect,
    ReturnFromChampSelect,
    PlayerSelectedChamp(PlayerId, ChampionId),
    PlayerLockedSelection(PlayerId),
    GameStarted(Vec<u8>),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ClientToLobby {
    Handshake { name: String },
    FetchLobbyList,
    CreateAndJoinLobby,
    JoinLobby(LobbyId),
    LeaveCurrentLobby,
    GetLobbyInfo(LobbyId),
    GetPlayerInfo(PlayerId),
    SetLobbySettings(LobbySettings),
    ChangePlayerTeam(PlayerId, Team),
    SwitchPlayerPositions(PlayerId, PlayerId),
    KickPlayer(PlayerId),
    GoToChampSelect,
    SelectChamp(ChampionId),
    LockSelection,
    Disconnect,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LobbyShortInfo {
    pub id: LobbyId,
    pub name: String,
    pub player_count: usize,
    pub max_player_count: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LobbyInfo {
    pub short: LobbyShortInfo,
    pub settings: LobbySettings,
    pub teams: Vec<Vec<PlayerId>>,
    pub leader: PlayerId,
    pub lobby_state: LobbyState,
    pub selected_champs: HashMap<PlayerId, ChampionSelection>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum LobbyState {
    InLobby,
    InChampSelect,
    InGame,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChampionSelection {
    pub id: ChampionId,
    pub locked: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LobbySettings {
    pub name: String,
    pub locked: bool,
    pub team_count: usize,
    pub max_players_per_team: usize,
}

impl LobbySettings {
    pub fn max_players(&self) -> usize {
        self.team_count * self.max_players_per_team
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlayerInfo {
    pub id: PlayerId,
    pub name: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct Team(pub usize);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct LobbyId(pub Uuid);

impl LobbyId {
    pub fn new() -> Self {
        Self(Uuid::new_v4())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct PlayerId(pub Uuid);

impl PlayerId {
    pub fn new() -> Self {
        Self(Uuid::new_v4())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum LobbyToServer {
    Handshake {
        settings: LobbySettings,
        players: Vec<PlayerGameInfo>,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ServerToLobby {
    PlayerTokens { tokens: HashMap<PlayerId, Vec<u8>> },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlayerGameInfo {
    pub id: PlayerId,
    pub team: Team,
    pub champ: ChampionId,
}
