use bevy::prelude::*;
use lightyear::prelude::{AppComponentExt};
use serde::{Deserialize, Serialize};

pub fn plugin(app: &mut App) {
    app.register_component::<StatBlock>();
}

#[derive(Component, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct StatBlock {
    pub max_health: Stat,
    pub move_speed: Stat,
    pub attack_speed: Stat,
    pub attack_a: Stat,
    pub attack_b: Stat,
    pub attack_c: Stat,
    pub resistance_a: Stat,
    pub resistance_b: Stat,
    pub resistance_c: Stat,
    pub range: Stat,
}

impl From<BaseStats> for StatBlock {
    fn from(value: BaseStats) -> Self {
        Self {
            max_health: Stat::new(value.max_health),
            move_speed: Stat::new(value.move_speed),
            attack_speed: Stat::new(value.attack_speed),
            attack_a: Stat::new(value.attack_a),
            attack_b: Stat::new(value.attack_b),
            attack_c: Stat::new(value.attack_c),
            resistance_a: Stat::new(value.resistance_a),
            resistance_b: Stat::new(value.resistance_b),
            resistance_c: Stat::new(value.resistance_c),
            range: Stat::new(value.range),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Stat {
    pub base: f32,
}

impl Stat {
    pub fn new(base: f32) -> Self {
        Self { base }
    }
}

#[derive(Clone, PartialEq)]
pub struct BaseStats {
    pub max_health: f32,
    pub move_speed: f32,
    pub attack_speed: f32,
    pub attack_a: f32,
    pub attack_b: f32,
    pub attack_c: f32,
    pub resistance_a: f32,
    pub resistance_b: f32,
    pub resistance_c: f32,
    pub range: f32,
}

from_into_lua_table!(
    struct BaseStats {
        max_health: f32,
        move_speed: f32,
        attack_speed: f32,
        attack_a: f32,
        attack_b: f32,
        attack_c: f32,
        resistance_a: f32,
        resistance_b: f32,
        resistance_c: f32,
        range: f32,
    }
);
