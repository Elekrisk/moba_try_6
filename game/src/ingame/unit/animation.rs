use super::super::structure::Model;
use crate::AppExt;
use crate::ingame::unit::movement::CurrentPath;
use bevy::platform::collections::HashMap;
use bevy::prelude::*;
use bevy::scene::SceneInstanceReady;

pub fn plugin(app: &mut App) {
    if app.is_client() {
        app.add_observer(attach_animation_controller);

        app.add_systems(Update, (on_gltf_load, do_animation));

        app.init_resource::<GltfAnimations>();
    }
}

#[derive(Component)]
pub struct AnimationPlayerProxy(pub Entity);

pub(crate) fn attach_animation_controller(
    trigger: Trigger<SceneInstanceReady>,
    model: Query<&Model>,
    children: Query<&Children>,
    mut transform: Query<&mut Transform>,
    mut anim: Query<&mut AnimationPlayer>,
    asset_server: Res<AssetServer>,
    gltf_animations: Res<GltfAnimations>,
    mut commands: Commands,
) {
    if let Ok(model) = model.get(trigger.target()) {
        let gltf_path = model.0.path();
        let Some(gltf_handle) = asset_server.get_handle(gltf_path) else {
            error!(
                "Model {} was not registered for loading",
                gltf_path.display()
            );
            return;
        };

        let Some((handle, anims)) = gltf_animations.map.get(&gltf_handle) else {
            error!("GLTF animation list not found; this is a bug");
            return;
        };

        // flip transform
        transform
            .get_mut(children.get(trigger.target()).unwrap()[0])
            .unwrap()
            .rotate_local_y(180.0f32.to_radians());

        for e in children.iter_descendants(trigger.target()) {
            if let Ok(mut anim) = anim.get_mut(e) {
                commands
                    .entity(trigger.target())
                    .insert((AnimationPlayerProxy(e), AnimationManager::new()));
                // We add animation graph handle
                commands
                    .entity(e)
                    .insert(AnimationGraphHandle(handle.clone()));
                // if let Some(idle) = anims.get("Idle") {
                //     anim.play(*idle).repeat();
                // } else {
                //     warn!("Model has no Idle animation");
                // }
            }
        }
    }
}

#[derive(Component)]
pub struct AnimationManager {
    pub next_animation: String,
    pub loop_animation: bool,
}

impl AnimationManager {
    pub fn new() -> Self {
        Self {
            next_animation: "Idle".into(),
            loop_animation: true,
        }
    }
}

fn do_animation(q: Query<(&AnimationManager, &AnimationPlayerProxy, &Model), Changed<AnimationManager>>, mut player: Query<&mut AnimationPlayer>, anims: Res<GltfAnimations>, asset_server: Res<AssetServer>) {
    for (man, proxy, model) in q {
        let Some(gltf_handle) = asset_server.get_handle(model.0.path()) else {
            error!("Asset {} wasn't registered in lua script", model.0.path().display());
            continue;
        };
        let (_, anims) = anims.map.get(&gltf_handle).unwrap();
        let &anim = anims.get(&man.next_animation).or_else(|| anims.get("Idle")).unwrap();
        let mut player = player.get_mut(proxy.0).unwrap();
        if !player.is_playing_animation(anim) {
            player.stop_all();
            let anim = player.play(anim);
            if man.loop_animation {
                anim.repeat();
            }
        }
    }
}

pub(crate) fn on_gltf_load(
    mut asset_events: EventReader<AssetEvent<Gltf>>,
    assets: Res<Assets<Gltf>>,
    mut gltf_animations: ResMut<GltfAnimations>,
    asset_server: Res<AssetServer>,
) {
    for event in asset_events.read() {
        info!("GLTF event: {event:?}");

        match event {
            AssetEvent::LoadedWithDependencies { id } => {
                let Some(gltf) = assets.get(*id) else {
                    warn!("Cannot get GLTF for id {}", *id);
                    continue;
                };

                let (handle, map) = gltf_animations.map.entry(Handle::Weak(*id)).or_default();

                let mut graph = AnimationGraph::new();

                for (name, anim) in &gltf.named_animations {
                    let index = graph.add_clip(anim.clone(), 1.0, graph.root);
                    map.insert(name.to_string(), index);
                }

                *handle = asset_server.add(graph);
            }
            _ => {}
        }
    }
}

#[derive(Resource, Default)]
pub struct GltfAnimations {
    pub(crate) map:
        HashMap<Handle<Gltf>, (Handle<AnimationGraph>, HashMap<String, AnimationNodeIndex>)>,
}
