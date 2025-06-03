use std::path::PathBuf;

use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[cfg_attr(feature = "bevy", derive(bevy_reflect::prelude::Reflect))]
pub struct ChampionId(pub String);

#[derive(Clone, Debug, Serialize, Deserialize)]
#[cfg_attr(feature = "bevy", derive(bevy_reflect::prelude::Reflect, bevy_asset::prelude::Asset))]
pub struct ChampionDef {
    pub id: ChampionId,
    pub name: String,
    pub icon: PathBuf,
}

impl ChampionDef {
    pub fn example() -> Self {
        Self {
            id: ChampionId("example_champion".into()),
            name: "Example Champion".into(),
            icon: "champs/unknown.png".into(),
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[cfg_attr(feature = "bevy", derive(bevy_reflect::prelude::Reflect, bevy_asset::prelude::Asset))]
pub struct ChampList(pub Vec<String>);
impl ChampList {
    pub fn example() -> Self {
        Self(vec!["champ1".into()])
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[cfg_attr(feature = "bevy", derive(bevy_reflect::prelude::Reflect))]
pub struct MapId(pub String);

#[derive(Clone, Debug, Serialize, Deserialize)]
#[cfg_attr(feature = "bevy", derive(bevy_reflect::prelude::Reflect, bevy_asset::prelude::Asset))]
pub struct MapDef {
    pub id: MapId,
    pub name: String,
    pub script: PathBuf,
}
