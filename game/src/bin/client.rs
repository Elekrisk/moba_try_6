use std::{any::TypeId, fs::File, path::PathBuf, sync::OnceLock};

use bevy::{
    asset::{ReflectAsset, UntypedAssetId}, dev_tools::fps_overlay::FpsOverlayPlugin, ecs::system::SystemIdMarker, log::{tracing_subscriber::Layer, BoxedLayer, LogPlugin}, prelude::*, reflect::TypeRegistry, render::camera::Viewport, window::PrimaryWindow
};
use bevy_enhanced_input::{
    EnhancedInputPlugin,
    events::Fired,
    prelude::{Actions, InputAction, InputContext, InputContextAppExt, JustPress},
};
use bevy_inspector_egui::{
    DefaultInspectorConfigPlugin,
    bevy_egui::{EguiContext, EguiContextPass, EguiContextSettings, EguiPlugin},
    bevy_inspector::{
        self, EntityFilter,
        hierarchy::{Hierarchy, SelectedEntities},
        ui_for_entities_shared_components, ui_for_entity_with_children,
    },
};
use clap::Parser;
use egui_dock::{DockArea, DockState, NodeIndex, Style, egui};
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

fn main() -> AppExit {
    let options = Options::parse();

    let mut app = App::new();
    if let Some(log_file) = &options.log_file {
        LOG_FILE.set(log_file.clone()).unwrap()
    }
    app.insert_resource(options)
        .add_plugins((
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
                    // filter: "lightyear=debug,lightyear::client::sync=info".into(),
                    // level: bevy::log::Level::DEBUG,
                    ..default()
                }),
            FpsOverlayPlugin::default(),
            EguiPlugin {
                enable_multipass_for_primary_context: true,
            },
            DefaultInspectorConfigPlugin,
            inspector,
            EnhancedInputPlugin,
            game::client,
        ))
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

fn inspector(app: &mut App) {
    app.add_systems(EguiContextPass, show_ui_system)
        .add_systems(PostUpdate, set_camera_viewport.after(show_ui_system))
        .insert_resource(UiState::new());
}

fn show_ui_system(world: &mut World) {
    let Ok(egui_context) = world
        .query_filtered::<&mut EguiContext, With<PrimaryWindow>>()
        .single(world)
    else {
        return;
    };

    let mut egui_context = egui_context.clone();

    world.resource_scope::<UiState, _>(|world, mut ui_state| {
        ui_state.ui(world, egui_context.get_mut())
    });
}

#[derive(Resource)]
struct UiState {
    state: DockState<EguiWindow>,
    viewport_rect: egui::Rect,
    selected_entities: SelectedEntities,
    selection: InspectorSelection,
}

#[derive(Eq, PartialEq)]
enum InspectorSelection {
    Entities,
    Resource(TypeId, String),
    Asset(TypeId, String, UntypedAssetId),
}

impl UiState {
    pub fn new() -> Self {
        let mut state = DockState::new(vec![EguiWindow::GameView]);
        let tree = state.main_surface_mut();
        let [game, _inspector] =
            tree.split_right(NodeIndex::root(), 0.75, vec![EguiWindow::Inspector]);
        let [game, _hierarchy] = tree.split_left(game, 0.2, vec![EguiWindow::Hierarchy]);
        let [_game, _bottom] =
            tree.split_below(game, 0.8, vec![EguiWindow::Resources, EguiWindow::Assets]);

        Self {
            state,
            viewport_rect: egui::Rect::NOTHING,
            selected_entities: SelectedEntities::default(),
            selection: InspectorSelection::Entities,
        }
    }

    fn ui(&mut self, world: &mut World, ctx: &mut egui::Context) {
        if !world.resource::<RunInspector>().0 {
            return;
        }
        let mut tab_viewer = TabViewer {
            world,
            viewport_rect: &mut self.viewport_rect,
            selected_entities: &mut self.selected_entities,
            selection: &mut self.selection,
        };
        DockArea::new(&mut self.state)
            .style(Style::from_egui(ctx.style().as_ref()))
            .show(ctx, &mut tab_viewer);
    }
}

#[derive(Debug)]
enum EguiWindow {
    GameView,
    Hierarchy,
    Resources,
    Assets,
    Inspector,
}

struct TabViewer<'a> {
    world: &'a mut World,
    viewport_rect: &'a mut egui::Rect,
    selected_entities: &'a mut SelectedEntities,
    selection: &'a mut InspectorSelection,
}

impl egui_dock::TabViewer for TabViewer<'_> {
    type Tab = EguiWindow;

    fn title(&mut self, tab: &mut Self::Tab) -> egui::WidgetText {
        format!("{tab:?}").into()
    }

    fn clear_background(&self, tab: &Self::Tab) -> bool {
        !matches!(tab, EguiWindow::GameView)
    }

    fn ui(&mut self, ui: &mut egui::Ui, tab: &mut Self::Tab) {
        let type_registry = self.world.resource::<AppTypeRegistry>().0.clone();
        let type_registry = type_registry.read();

        match tab {
            EguiWindow::GameView => {
                *self.viewport_rect = ui.clip_rect();
            }
            EguiWindow::Hierarchy => {
                // let selected = hierarchy_ui(self.world, ui, self.selected_entities);
                let type_registry = self.world.resource::<AppTypeRegistry>().clone();
                let type_registry = type_registry.read();

                let filter = ComponentSearch::from_ui(ui, egui::Id::new("component_search_filter"));
                let selected = Hierarchy {
                    world: self.world,
                    type_registry: &type_registry,
                    selected: self.selected_entities,
                    context_menu: None,
                    shortcircuit_entity: None,
                    extra_state: &mut (),
                }
                .show_with_filter::<(Without<SystemIdMarker>, Without<Observer>), _>(ui, filter);

                if selected {
                    *self.selection = InspectorSelection::Entities;
                }
            }
            EguiWindow::Resources => select_resource(ui, &type_registry, self.selection),
            EguiWindow::Assets => select_asset(ui, &type_registry, self.world, self.selection),
            EguiWindow::Inspector => match *self.selection {
                InspectorSelection::Entities => match self.selected_entities.as_slice() {
                    &[entity] => ui_for_entity_with_children(self.world, entity, ui),
                    entities => ui_for_entities_shared_components(self.world, entities, ui),
                },
                InspectorSelection::Resource(type_id, ref name) => {
                    ui.label(name);
                    bevy_inspector::by_type_id::ui_for_resource(
                        self.world,
                        type_id,
                        ui,
                        name,
                        &type_registry,
                    )
                }
                InspectorSelection::Asset(type_id, ref name, handle) => {
                    ui.label(name);
                    bevy_inspector::by_type_id::ui_for_asset(
                        self.world,
                        type_id,
                        handle,
                        ui,
                        &type_registry,
                    );
                }
            },
        }
    }
}

struct ComponentSearch {
    component_name: String,
}

impl ComponentSearch {
    fn from_ui(ui: &mut egui::Ui, id: egui::Id) -> Self {
        let word = {
            let id = id.with("word");
            let mut filter_string = ui.memory_mut(|mem| {
                let filter: &mut String = mem.data.get_persisted_mut_or_default(id);
                filter.clone()
            });
            ui.text_edit_singleline(&mut filter_string);
            ui.memory_mut(|mem| {
                *mem.data.get_persisted_mut_or_default(id) = filter_string.clone();
            });

            // improves overall matching
            filter_string.to_lowercase()
        };

        Self {
            component_name: word,
        }
    }
}

impl EntityFilter for ComponentSearch {
    type StaticFilter = (Without<SystemIdMarker>, Without<Observer>);

    fn filter_entity(&self, world: &mut World, entity: Entity) -> bool {
        for comp in world.inspect_entity(entity).unwrap() {
            if comp
                .name()
                .to_lowercase()
                .contains(&self.component_name)
            {
                return true;
            }
        }

        false
    }
}

fn select_resource(
    ui: &mut egui::Ui,
    type_registry: &TypeRegistry,
    selection: &mut InspectorSelection,
) {
    let mut resources: Vec<_> = type_registry
        .iter()
        .filter(|registration| registration.data::<ReflectResource>().is_some())
        .map(|registration| {
            (
                registration.type_info().type_path_table().short_path(),
                registration.type_id(),
            )
        })
        .collect();
    resources.sort_by(|(name_a, _), (name_b, _)| name_a.cmp(name_b));

    for (resource_name, type_id) in resources {
        let selected = match *selection {
            InspectorSelection::Resource(selected, _) => selected == type_id,
            _ => false,
        };

        if ui.selectable_label(selected, resource_name).clicked() {
            *selection = InspectorSelection::Resource(type_id, resource_name.to_string());
        }
    }
}

fn select_asset(
    ui: &mut egui::Ui,
    type_registry: &TypeRegistry,
    world: &World,
    selection: &mut InspectorSelection,
) {
    let mut assets: Vec<_> = type_registry
        .iter()
        .filter_map(|registration| {
            let reflect_asset = registration.data::<ReflectAsset>()?;
            Some((
                registration.type_info().type_path_table().short_path(),
                registration.type_id(),
                reflect_asset,
            ))
        })
        .collect();
    assets.sort_by(|(name_a, ..), (name_b, ..)| name_a.cmp(name_b));

    for (asset_name, asset_type_id, reflect_asset) in assets {
        let handles: Vec<_> = reflect_asset.ids(world).collect();

        ui.collapsing(format!("{asset_name} ({})", handles.len()), |ui| {
            for handle in handles {
                let selected = match *selection {
                    InspectorSelection::Asset(_, _, selected_id) => selected_id == handle,
                    _ => false,
                };

                if ui
                    .selectable_label(selected, format!("{handle:?}"))
                    .clicked()
                {
                    *selection =
                        InspectorSelection::Asset(asset_type_id, asset_name.to_string(), handle);
                }
            }
        });
    }
}

// make camera only render to view not obstructed by UI
fn set_camera_viewport(
    ui_state: Res<UiState>,
    primary_window: Query<&mut Window, With<PrimaryWindow>>,
    egui_settings: Single<&EguiContextSettings>,
    run_inspector: Res<RunInspector>,
    mut cam: Single<&mut Camera>,
) {
    let Ok(window) = primary_window.single() else {
        return;
    };

    if !run_inspector.0 {
        cam.viewport = None;
        return;
    }

    let scale_factor = window.scale_factor() * egui_settings.scale_factor;

    let viewport_pos = ui_state.viewport_rect.left_top().to_vec2() * scale_factor;
    let viewport_size = ui_state.viewport_rect.size() * scale_factor;

    let physical_position = UVec2::new(viewport_pos.x as u32, viewport_pos.y as u32);
    let physical_size = UVec2::new(viewport_size.x as u32, viewport_size.y as u32);

    // The desired viewport rectangle at its offset in "physical pixel space"
    let rect = physical_position + physical_size;

    let window_size = window.physical_size();
    // wgpu will panic if trying to set a viewport rect which has coordinates extending
    // past the size of the render target, i.e. the physical window in our case.
    // Typically this shouldn't happen- but during init and resizing etc. edge cases might occur.
    // Simply do nothing in those cases.
    if rect.x <= window_size.x && rect.y <= window_size.y {
        cam.viewport = Some(Viewport {
            physical_position,
            physical_size,
            depth: 0.0..1.0,
        });
    }
}
