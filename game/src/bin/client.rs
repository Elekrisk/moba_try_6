use std::{fs::File, path::PathBuf, sync::OnceLock};

use bevy::{
    log::{BoxedLayer, LogPlugin, tracing_subscriber::Layer},
    prelude::*,
};
use bevy_enhanced_input::{
    EnhancedInputPlugin,
    events::Fired,
    prelude::{Actions, InputAction, InputContext, InputContextAppExt, JustPress},
};
use bevy_inspector_egui::{bevy_egui::EguiPlugin, quick::WorldInspectorPlugin};
use clap::Parser;
use game::{LobbySender, Options};
use lobby_common::ClientToLobby;

use tracing_appender::non_blocking::WorkerGuard;

static LOG_GUARD: OnceLock<WorkerGuard> = OnceLock::new();

static LOG_FILE: OnceLock<PathBuf> = OnceLock::new();

fn custom_layer(_app: &mut App) -> Option<BoxedLayer> {
    let file = File::create(LOG_FILE.get()?).unwrap();
    let (non_blocking, guard) = tracing_appender::non_blocking(file);
    let _ = LOG_GUARD.set(guard);
    Some(
        bevy::log::tracing_subscriber::fmt::layer()
            .with_writer(non_blocking)
            .with_file(true)
            .with_line_number(true)
            .with_ansi(false)
            .with_file(false)
            .boxed(),
    )
}

#[derive(Resource)]
struct LogFile(PathBuf);

fn main() -> AppExit {
    let options = Options::parse();

    let mut app = App::new();
    if let Some(log_file) = &options.log_file {
        LOG_FILE.set(log_file.clone()).unwrap()
    }
    app.add_plugins((
        DefaultPlugins
            .set(WindowPlugin {
                primary_window: Some(Window {
                    fit_canvas_to_parent: true,
                    ..default()
                }),
                ..default()
            })
            .set(LogPlugin {
                custom_layer,
                ..default()
            }),
        game::client,
        EguiPlugin {
            enable_multipass_for_primary_context: true,
        },
        WorldInspectorPlugin::new().run_if(resource_equals(RunInspector(true))),
        EnhancedInputPlugin,
    ))
    .insert_resource(options)
    .insert_resource(RunInspector(false))
    .add_input_context::<GlobalInput>()
    .add_systems(Startup, |mut commands: Commands| {
        let mut actions = Actions::<GlobalInput>::default();
        actions
            .bind::<ToggleInspector>()
            .to(KeyCode::F3)
            .with_conditions(JustPress::default());
        commands.spawn(actions).observe(
            |_: Trigger<Fired<ToggleInspector>>, mut r: ResMut<RunInspector>| {
                info!("Toggle inspector fired!");
                r.0 = !r.0;
            },
        );
    })
    .add_systems(
        PostUpdate,
        (|mut events: EventReader<AppExit>, sender: Option<Res<LobbySender>>| {
            for _ in events.read() {
                if let Some(ref sender) = sender {
                    info!("Sending DISCONNECT");
                    _ = sender.0.send(ClientToLobby::Disconnect);
                }
            }
        })
        .after(bevy::window::exit_on_all_closed),
    )
    .run()
}

#[derive(Resource, PartialEq, Eq)]
struct RunInspector(bool);

#[derive(Debug, InputAction)]
#[input_action(output = bool)]
struct ToggleInspector;

#[derive(InputContext)]
struct GlobalInput;
