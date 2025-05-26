#![feature(decl_macro)]
#![feature(mpmc_channel)]
#![feature(iter_array_chunks)]
#![feature(type_alias_impl_trait)]
#![feature(anonymous_lifetime_in_impl_trait)]
#![feature(iter_intersperse)]
#![feature(impl_trait_in_assoc_type)]

use std::{fmt::Display, path::PathBuf};

use bevy::{
    ecs::bundle::{BundleEffect, DynamicBundle},
    prelude::*,
};

mod r#async;
mod main_ui;
mod network;
pub mod ui;
pub mod new_ui;

pub use network::LobbySender;
use ui::custom_effect;

pub fn client(app: &mut App) {
    common(app);

    app.add_plugins((
        ui::client,
        new_ui::client,
        main_ui::client,
        network::client,
        r#async::client,
    ));
    app.add_systems(Startup, client_setup);

    app.insert_state(ClientState::NotInGame);
}

pub fn server(app: &mut App) {
    common(app);
}

fn common(app: &mut App) {}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash, States)]
pub enum ClientState {
    #[default]
    NotInGame,
    InGame,
}

fn client_setup(mut commands: Commands) {
    commands.spawn((
        Camera3d::default(),
        Projection::Perspective(PerspectiveProjection::default()),
    ));
}

#[derive(Resource, clap::Parser)]
pub struct Options {
    #[arg(long)]
    connect: bool,
    #[arg(long, default_value_t = LobbyMode::None)]
    lobby_mode: LobbyMode,
    #[arg(long)]
    pub log_file: Option<PathBuf>,
}

#[derive(Clone, Default, clap::ValueEnum)]
pub enum LobbyMode {
    #[default]
    None,
    AutoCreate,
    AutoJoinFirst,
}

impl Display for LobbyMode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{}",
            match self {
                Self::None => "none",
                Self::AutoCreate => "auto-create",
                Self::AutoJoinFirst => "auto-join-first",
            }
        )
    }
}

struct InsertIf<F, B>(F, B);

custom_effect!(InsertIf<F, B> where F: FnOnce(&mut EntityWorldMut) -> bool + Send + Sync + 'static, B: Bundle);

impl<F: FnOnce(&mut EntityWorldMut) -> bool + Send + Sync + 'static, B: Bundle> BundleEffect
    for InsertIf<F, B>
{
    fn apply(self, entity: &mut EntityWorldMut) {
        if self.0(entity) {
            entity.insert(self.1);
        }
    }
}
