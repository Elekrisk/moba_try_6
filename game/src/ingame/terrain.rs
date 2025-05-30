use core::f32;
use std::path::PathBuf;

use bevy::{
    asset::uuid::Uuid,
    math::bounding::{Bounded2d, BoundingVolume},
    prelude::*,
};
use bevy_enhanced_input::prelude::*;
use bevy_inspector_egui::bevy_egui::{EguiContextPass, EguiContexts, input::egui_wants_any_input};
use egui_dock::egui;
use mlua::prelude::*;
use ron::ser::PrettyConfig;
use serde::{Deserialize, Serialize};

use super::{
    camera::MousePos,
    lua::{AppLuaExt, LuaExt},
};

#[derive(Debug, Default, PartialEq, Eq, Clone, Copy, Hash, States)]
enum IsTerrainEditingState {
    #[default]
    No,
    Yes,
}

pub fn client(app: &mut App) {
    app.init_state::<EditingState>()
        .init_state::<IsTerrainEditingState>()
        .init_resource::<SelectedTerrainObject>()
        .add_input_context::<TerrainEditing>()
        .add_systems(Startup, |mut commands: Commands| {
            commands.spawn(Actions::<TerrainEditing>::default());
        })
        .add_observer(bind_terrain_edit_actions)
        .add_observer(drag_closest_vertex)
        .add_observer(select_object)
        .add_observer(draw_object)
        .add_observer(
            |_: Trigger<Fired<ToggleEditing>>,
             state: Res<State<IsTerrainEditingState>>,
             mut commands: Commands| {
                commands.set_state(match state.get() {
                    IsTerrainEditingState::No => IsTerrainEditingState::Yes,
                    IsTerrainEditingState::Yes => IsTerrainEditingState::No,
                });
            },
        )
        .add_systems(EguiContextPass, terrain_editing_ui)
        .add_systems(
            OnEnter(EditingState::ObjectCreating),
            |mut sel: ResMut<SelectedTerrainObject>| {
                sel.0 = None;
            },
        )
        .add_systems(Update, draw_terrain.run_if(resource_exists::<Terrain>))
        .add_systems(
            Update,
            (
                enable_input.run_if(not(
                    egui_wants_any_input.and(in_state(IsTerrainEditingState::Yes))
                )),
                disable_input
                    .run_if(egui_wants_any_input.and(in_state(IsTerrainEditingState::Yes))),
            ),
        );
}

pub fn common(app: &mut App) {
    app.insert_resource(Terrain { objects: vec![] })
        .setup_lua(setup_lua);
}

fn setup_lua(lua: &Lua) -> LuaResult<()> {
    let table = lua.table("game")?;
    table.set(
        "load_terrain",
        lua.create_function(|lua, args: LoadTerrainArgs| {
            info!("Loading terrain from {}", args.file.display());
            let path = lua.path(&args.file);
            info!("Converted path is {}", path.display());
            let terrain =
                std::fs::read(format!("assets/{}", path.display())).map_err(LuaError::external)?;
            let mut terrain: Terrain = ron::de::from_bytes(&terrain).map_err(LuaError::external)?;

            if args.new_uuids.unwrap_or_default() {
                for object in &mut terrain.objects {
                    object.uuid = Uuid::new_v4();
                }
            }

            if args.mirror.unwrap_or_default() {
                for object in &mut terrain.objects {
                    for vertex in &mut object.vertices {
                        let old = *vertex;
                        vertex.x = old.y;
                        vertex.y = old.x;
                    }
                }
            }

            lua.world()
                .get_resource_or_init::<Terrain>()
                .objects
                .append(&mut terrain.objects);
            Ok(())
        })?,
    )?;
    Ok(())
}

struct LoadTerrainArgs {
    file: PathBuf,
    new_uuids: Option<bool>,
    mirror: Option<bool>,
}

from_into_lua_table!(
    struct LoadTerrainArgs {
        file: PathBuf,
        new_uuids: Option<bool>,
        mirror: Option<bool>,
    }
);

fn enable_input(mut action_sources: ResMut<ActionSources>) {
    action_sources.keyboard = true;
    action_sources.mouse_buttons = true;
    action_sources.mouse_motion = true;
    action_sources.mouse_wheel = true;
}

fn disable_input(mut action_sources: ResMut<ActionSources>) {
    action_sources.keyboard = false;
    action_sources.mouse_buttons = false;
    action_sources.mouse_motion = false;
    action_sources.mouse_wheel = false;
}

#[derive(Serialize, Deserialize)]
pub struct TerrainObject {
    pub uuid: Uuid,
    pub vertices: Vec<Vec2>,
}

impl TerrainObject {
    pub fn center(&self) -> Vec2 {
        let mut left = f32::INFINITY;
        let mut right = f32::NEG_INFINITY;
        let mut top = f32::INFINITY;
        let mut bot = f32::NEG_INFINITY;

        for vertex in &self.vertices {
            left = vertex.x.min(left);
            right = vertex.x.max(right);
            top = vertex.y.min(top);
            bot = vertex.y.max(bot);
        }

        Vec2::new((left + right) / 2.0, (top + bot) / 2.0)
    }
}

#[derive(Resource, Default, Serialize, Deserialize)]
pub struct Terrain {
    pub objects: Vec<TerrainObject>,
}

fn draw_terrain(
    terrain: Res<Terrain>,
    state: Res<State<EditingState>>,
    selected_object: Res<SelectedTerrainObject>,
    mut gizmos: Gizmos,
) {
    for object in &terrain.objects {
        for [start, end] in object.vertices.array_windows().chain(std::iter::once(&[
            *object.vertices.last().unwrap(),
            object.vertices[0],
        ])) {
            let color = if let Some(uuid) = selected_object.0
                && object.uuid == uuid
            {
                match state.get() {
                    EditingState::ObjectSelection => Color::srgb(1.0, 0.647, 0.0),
                    EditingState::ObjectEditing => Color::srgb(0.0, 0.0, 1.0),
                    EditingState::ObjectCreating => Color::srgb(0.0, 1.0, 0.0),
                }
            } else {
                Color::BLACK
            };

            // let center = object.center();

            // gizmos.sphere(Vec3::new(center.x, 0.1, center.y), 0.5, color);

            gizmos.line(
                Vec3::new(start.x, 0.1, start.y),
                Vec3::new(start.x, 0.0, start.y),
                color,
            );
            gizmos.line(
                Vec3::new(start.x, 0.1, start.y),
                Vec3::new(end.x, 0.1, end.y),
                color,
            );
        }
    }
}

fn bind_terrain_edit_actions(
    trigger: Trigger<Binding<TerrainEditing>>,
    mut actions: Query<&mut Actions<TerrainEditing>>,
) {
    let mut actions = actions.get_mut(trigger.target()).unwrap();
    actions
        .bind::<SelectTerrain>()
        .to(MouseButton::Left)
        .with_conditions(JustPress::default());
    actions
        .bind::<ToggleEditing>()
        .to(KeyCode::F4)
        .with_conditions(JustPress::default());
    actions.bind::<DragVertex>().to(MouseButton::Left);
}

#[derive(Debug, InputContext)]
struct TerrainEditing;

#[derive(Debug, InputAction)]
#[input_action(output = bool)]
struct ToggleEditing;

#[derive(Debug, InputAction)]
#[input_action(output = bool)]
struct DragVertex;

#[derive(Debug, InputAction)]
#[input_action(output = bool)]
struct SelectTerrain;

#[derive(Clone, Copy, PartialEq, PartialOrd)]
struct C(f32);

#[derive(Default, Resource)]
struct SelectedTerrainObject(Option<Uuid>);

struct SelectedVertex(Option<usize>);

#[derive(Default, Clone, Copy, Debug, PartialEq, Eq, Hash, States)]
enum EditingState {
    #[default]
    ObjectSelection,
    ObjectEditing,
    ObjectCreating,
}

impl Eq for C {}

impl Ord for C {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.partial_cmp(other).unwrap()
    }
}

fn select_object(
    _: Trigger<Fired<SelectTerrain>>,
    mouse_pos: Res<MousePos>,
    terrain: Res<Terrain>,
    mut selected_object: ResMut<SelectedTerrainObject>,
    state: Res<State<EditingState>>,
    state2: Res<State<IsTerrainEditingState>>,
) {
    if *state.get() != EditingState::ObjectSelection || *state2.get() != IsTerrainEditingState::Yes
    {
        return;
    }

    // Get closest object
    if let Some(object) = terrain
        .objects
        .iter()
        .min_by_key(|object| C(object.center().distance_squared(mouse_pos.plane_pos)))
    {
        selected_object.0 = Some(object.uuid);
    }
}

fn drag_closest_vertex(
    _: Trigger<Fired<DragVertex>>,
    state: Res<State<EditingState>>,
    state2: Res<State<IsTerrainEditingState>>,
    sel: Res<SelectedTerrainObject>,
    mut terrain: ResMut<Terrain>,
    mouse_pos: Res<MousePos>,
) {
    if *state.get() != EditingState::ObjectEditing || *state2.get() != IsTerrainEditingState::Yes {
        return;
    }

    if let Some(uuid) = sel.0
        && let Some(object) = terrain.objects.iter_mut().find(|o| o.uuid == uuid)
    {
        if let Some(closest_vertex) = object
            .vertices
            .iter_mut()
            .min_by_key(|v| C(v.distance_squared(mouse_pos.plane_pos)))
        {
            *closest_vertex = mouse_pos.plane_pos;
        }
    }
}

fn draw_object(
    _: Trigger<Fired<SelectTerrain>>,
    mouse_pos: Res<MousePos>,
    mut terrain: ResMut<Terrain>,
    mut selected_object: ResMut<SelectedTerrainObject>,
    state2: Res<State<IsTerrainEditingState>>,
    state: Res<State<EditingState>>,
) {
    if *state.get() != EditingState::ObjectCreating || *state2.get() != IsTerrainEditingState::Yes {
        return;
    }

    // Object selected?
    if let Some(uuid) = selected_object.0 {
        if let Some(object) = terrain.objects.iter_mut().find(|o| o.uuid == uuid) {
            object.vertices.push(mouse_pos.plane_pos);
            return;
        } else {
            selected_object.0 = None;
        }
    }

    let uuid = Uuid::new_v4();
    terrain.objects.push(TerrainObject {
        uuid,
        vertices: vec![mouse_pos.plane_pos],
    });
    selected_object.0 = Some(uuid);
}

fn terrain_editing_ui(
    editing_state: Res<State<EditingState>>,
    state2: Res<State<IsTerrainEditingState>>,
    mut terrain: ResMut<Terrain>,
    mut selected_object: ResMut<SelectedTerrainObject>,
    mut contexts: EguiContexts,
    mut commands: Commands,
) {
    if *state2.get() != IsTerrainEditingState::Yes {
        return;
    }

    let mut to_remove = vec![];

    egui::Window::new("Wha").show(contexts.ctx_mut(), |ui| {
        ui.label(format!("Current mode: {:?}", editing_state.get()));

        ui.horizontal(|ui| {
            if ui.button("Object selection").clicked() {
                commands.set_state(EditingState::ObjectSelection);
            }
            if ui.button("Object editing").clicked() {
                commands.set_state(EditingState::ObjectEditing);
            }
            if ui.button("Object creating").clicked() {
                commands.set_state(EditingState::ObjectCreating);
            }
        });

        ui.horizontal(|ui| {
            if ui.button("Save to file").clicked() {
                match ron::ser::to_string_pretty(&*terrain, PrettyConfig::new()) {
                    Ok(val) => {
                        if let Err(e) = std::fs::write("terrain.ron", val) {
                            error!("{e}");
                        }
                    }
                    Err(err) => {
                        error!("{err}");
                    }
                };
            }
        });

        ui.separator();

        if let Some(uuid) = selected_object.0
            && let Some(mut object) = terrain.reborrow().filter_map_unchanged(|terrain| {
                terrain
                    .objects
                    .iter_mut()
                    .find(|object| object.uuid == uuid)
            })
        {
            object_ui(&mut selected_object, &mut to_remove, ui, &mut object);
        }

        ui.separator();

        let mut terrain = terrain.reborrow();
        egui::ScrollArea::vertical().show(ui, |ui| {
            for i in 0..terrain.objects.len() {
                let mut object = terrain
                    .reborrow()
                    .map_unchanged(|terrain| &mut terrain.objects[i]);
                object_ui(&mut selected_object, &mut to_remove, ui, &mut object);
            }
        })
    });

    for uuid in to_remove {
        if let Some(pos) = terrain.objects.iter().position(|p| p.uuid == uuid) {
            terrain.objects.remove(pos);
        }
    }
}

fn object_ui(
    selected_object: &mut ResMut<'_, SelectedTerrainObject>,
    to_remove: &mut Vec<Uuid>,
    ui: &mut egui::Ui,
    object: &mut Mut<'_, TerrainObject>,
) {
    ui.horizontal(|ui| {
        ui.label(format!("Terrain object [{:02}]", object.vertices.len()));

        if ui
            .add_enabled(object.vertices.len() > 1, egui::Button::new("-"))
            .clicked()
        {
            object.vertices.pop();
        }

        if ui.button("+").clicked() {
            let new_vertex = if object.vertices.len() == 1 {
                object.vertices[0] + Vec2::X
            } else {
                // let next_to_last = object.polyline.vertices.iter().rev().nth(1).unwrap();
                let last = object.vertices.last().unwrap();
                let first = object.vertices[0];
                let middle = last.midpoint(first);
                middle
            };

            object.vertices.push(new_vertex);
        }

        if Some(object.uuid) == selected_object.0 {
            if ui.button("Deselect").clicked() {
                selected_object.0 = None;
            }
        } else {
            if ui.button("Select").clicked() {
                selected_object.0 = Some(object.uuid);
            }
        }

        if ui.button("Delete").clicked() {
            to_remove.push(object.uuid);
        };
    });
}
