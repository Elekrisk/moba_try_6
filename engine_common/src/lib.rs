use std::path::PathBuf;

use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[cfg_attr(feature = "bevy", derive(bevy::prelude::Reflect))]
pub struct ChampionId(pub Uuid);

#[derive(Clone, Debug, Serialize, Deserialize)]
#[cfg_attr(feature = "bevy", derive(bevy::prelude::Reflect, bevy::prelude::Asset))]
pub struct ChampionDef {
    pub id: ChampionId,
    pub name: String,
    pub icon: PathBuf,
}

impl ChampionDef {
    pub fn example() -> Self {
        Self {
            id: ChampionId(Uuid::nil()),
            name: "Example Champion".into(),
            icon: "champs/unknown.png".into(),
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[cfg_attr(feature = "bevy", derive(bevy::prelude::Reflect, bevy::prelude::Asset))]
pub struct ChampList(pub Vec<String>);
impl ChampList {
    pub fn example() -> Self {
        Self(vec!["champ1".into()])
    }
}
