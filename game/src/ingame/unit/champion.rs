use std::path::PathBuf;

use bevy::{asset::AssetLoader, prelude::*};
use engine_common::{ChampionDef, ChampionId};
use serde::{Deserialize, Serialize};

use crate::ingame::lua::LuaScript;

use super::Unit;

// #[derive(Component, Default, Clone, PartialEq, Serialize, Deserialize)]
// #[require(Unit)]
// pub struct Champion;

pub fn plugin(app: &mut App) {
    app.init_asset::<ChampionDefAsset>()
        .register_asset_loader(ChampionDefLoader);
}

#[derive(Debug, Clone, Reflect, Asset)]
pub struct ChampionDefAsset {
    pub id: ChampionId,
    pub name: String,
    pub icon: PathBuf,
    pub script: Handle<LuaScript>,
}

struct ChampionDefLoader;

impl AssetLoader for ChampionDefLoader {
    type Asset = ChampionDefAsset;

    type Settings = ();

    type Error = anyhow::Error;

    fn load(
        &self,
        reader: &mut dyn bevy::asset::io::Reader,
        _settings: &Self::Settings,
        load_context: &mut bevy::asset::LoadContext,
    ) -> impl bevy::tasks::ConditionalSendFuture<Output = std::result::Result<Self::Asset, Self::Error>>
    {
        async move {
            let mut buf = vec![];
            reader.read_to_end(&mut buf).await?;
            let champ_def: ChampionDef = ron::de::from_bytes(&buf)?;

            let cur_path = load_context.asset_path();

            let path = if champ_def.script.starts_with(".") || champ_def.script.starts_with("..") {
                cur_path.path().parent().unwrap().join(&champ_def.script)
            } else {
                champ_def.script
            };

            Ok(ChampionDefAsset {
                id: champ_def.id,
                name: champ_def.name,
                icon: champ_def.icon,
                script: load_context.load(path),
            })
        }
    }

    fn extensions(&self) -> &[&str] {
        &["ron"]
    }
}
