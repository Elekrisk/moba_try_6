use bevy::prelude::*;
use lightyear::prelude::*;
use mlua::prelude::*;

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
    app.register_component::<StateList>(ChannelDirection::ServerToClient);

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
}

proto!(
    pub struct StateProto {
        pub id: String,
        pub name: String,
        pub priority: f32,
        pub move_cancellable: bool,
        pub on_move_cancel: Option<LuaFunction>,
        pub animation: String,
    }
);

#[derive(Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct State {
    pub proto: String,
}

impl State {
    pub fn new(proto: impl Into<String>) -> Self {
        Self {
            proto: proto.into(),
        }
    }

    pub fn idle() -> Self {
        Self::new("idle")
    }

    pub fn moving() -> Self {
        Self::new("moving")
    }
}

#[derive(Debug, Component, PartialEq, Eq, Serialize, Deserialize)]
pub struct StateList {
    /// Invariant: Always holds at least one state
    states: Vec<State>,
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
    }

    pub fn remove_state(&mut self, state: &State) {
        if let Some(pos) = self.states.iter().position(|s| s.proto == state.proto) && pos != 0 {
            self.states.remove(pos);
        }
    }

    pub fn current_state(&self) -> &State {
        self.states.last().unwrap()
    }
}

impl Default for StateList {
    fn default() -> Self {
        Self {
            states: vec![State::idle()],
        }
    }
}

fn play_state_animation(
    q: Query<(&StateList, &mut AnimationManager), Changed<StateList>>,
    protos: Res<Protos<StateProto>>,
) {
    for (state_list, mut man) in q {
        let (proto, _) = protos.get(&state_list.current_state().proto).unwrap();
        man.next_animation = proto.animation.clone();
    }
}
