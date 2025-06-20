use bevy::prelude::*;
use lightyear::prelude::*;
use mlua::prelude::*;
use serde::{Deserialize, Serialize};

use crate::{
    AppExt,
    ingame::{
        lua::{LuaCtx, LuaExt, Protos},
        structure::Model,
        unit::{
            UnitProxy,
            animation::{AnimationManager, AnimationPlayerProxy, GltfAnimations},
            attack::AutoAttackTimer,
        },
    },
};

pub fn plugin(app: &mut App) {
    app.register_component::<StateList>();

    let mut protos: Protos<StateProto> = Protos::from_world(app.world_mut());
    let lua = app.world().resource::<LuaCtx>().0.clone();

    protos
        .insert(
            StateProto {
                id: "idle".into(),
                name: "Idle".into(),
                priority: 0.0,
                move_cancellable: true,
                on_move_cancel: None,
                animation: "Idle".into(),
                loop_animation: true,
            }
            .into_lua(&lua)
            .unwrap(),
            "".into(),
        )
        .unwrap();

    protos
        .insert(
            StateProto {
                id: "moving".into(),
                name: "Moving".into(),
                priority: 1.0,
                move_cancellable: true,
                on_move_cancel: None,
                animation: "Moving".into(),
                loop_animation: true,
            }
            .into_lua(&lua)
            .unwrap(),
            "".into(),
        )
        .unwrap();

    protos
        .insert(
            StateProto {
                id: "attacking".into(),
                name: "Attacking".into(),
                priority: 2.0,
                move_cancellable: true,
                on_move_cancel: Some(
                    lua.create_function(|lua, e: UnitProxy| {
                        let mut world = lua.world();

                        let mut binding = world.entity_mut(e.entity);
                        let mut aa = binding.get_mut::<AutoAttackTimer>().unwrap();
                        if !aa.has_occurred {
                            aa.timer = 0.0;
                        }

                        Ok(())
                    })
                    .unwrap(),
                ),
                animation: "AutoAttack".into(),
                loop_animation: false,
            }
            .into_lua(&lua)
            .unwrap(),
            "".into(),
        )
        .unwrap();

    app.insert_resource(protos);

    if app.is_client() {
        app.add_systems(Update, play_state_animation);
    }
}

#[derive(PartialEq)]
pub struct StateProto {
    pub id: String,
    pub name: String,
    pub priority: f32,
    pub move_cancellable: bool,
    pub on_move_cancel: Option<LuaFunction>,
    pub animation: String,
    pub loop_animation: bool,
}

proto!(
    pub struct StateProto {
        pub id: String,
        pub name: String,
        pub priority: f32,
        pub move_cancellable: bool,
        pub on_move_cancel: Option<LuaFunction>,
        pub animation: String,
        pub loop_animation: bool,
    }
);

#[derive(Debug, PartialEq, Serialize, Deserialize)]
pub struct State {
    pub proto: String,
    pub animation_speed: f32,
}

impl State {
    pub fn new(proto: impl Into<String>) -> Self {
        Self {
            proto: proto.into(),
            animation_speed: 1.0,
        }
    }

    pub fn with_animation_speed(mut self, speed: f32) -> Self {
        self.animation_speed = speed;
        self
    }

    pub fn idle() -> Self {
        Self::new("idle")
    }

    pub fn moving() -> Self {
        Self::new("moving")
    }
}

#[derive(Debug, Component, PartialEq, Serialize, Deserialize)]
pub struct StateList {
    /// Invariant: Always holds at least one state
    states: Vec<State>,
    /// Used to force replicating change ticks
    x: u8,
}

impl StateList {
    pub fn add_state(&mut self, state: State, states: &Protos<StateProto>) {
        let (proto, _) = states.get_cached(&state.proto).unwrap();

        if let Some(pos) = self.states.array_windows::<2>().position(|[prev, next]| {
            let (prev_proto, _) = states.get_cached(&prev.proto).unwrap();
            let (next_proto, _) = states.get_cached(&next.proto).unwrap();
            prev_proto.priority <= proto.priority && proto.priority < next_proto.priority
        }) {
            self.states.insert(pos + 1, state);
        } else {
            self.states.push(state);
        }

        self.x = self.x.wrapping_add(1);
    }

    pub fn remove_state(&mut self, state: &str) {
        if let Some(pos) = self.states.iter().position(|s| s.proto == state)
            && pos != 0
        {
            self.states.remove(pos);
        }

        self.x = self.x.wrapping_add(1);
    }

    pub fn current_state(&self) -> &State {
        self.states.last().unwrap()
    }
}

impl Default for StateList {
    fn default() -> Self {
        Self {
            states: vec![State::idle()],
            x: 0,
        }
    }
}

fn play_state_animation(
    q: Query<(&StateList, &mut AnimationManager), Changed<StateList>>,
    protos: Res<Protos<StateProto>>,
) {
    for (state_list, mut man) in q {
        let current_state = state_list.current_state();
        let (proto, _) = protos.get(&current_state.proto).unwrap();
        man.next_animation = proto.animation.clone();
        man.speed = current_state.animation_speed;
        man.loop_animation = proto.loop_animation;
    }
}
