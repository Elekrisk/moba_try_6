declare class UnitProxy
    function get_position(self): Vec2
    function set_position(self, pos: Vec2): ()
    function set_movement_target(self, pos: Vec2): ()
    function set_attack_target(self, target: UnitProxy): ()
    function get_team(self): number
    function get_visible_units(self): { UnitProxy }
    function apply_effect(self, args: { proto: string, data: CustomData })
    function get_custom_data(self): CustomData
    function set_custom_data(self, data: CustomData)
end

declare class EffectProxy
    function get_custom_data(self): CustomData
    function set_custom_data(self, data: CustomData)
end

export type Vec2 = {
    x: number,
    y: number,
}

export type MapProto = {
    id: string,
    name: string,
    on_load: () -> () ?
}

export type StructureProto = {
    id: string,
    name: string,
    model: string,
    radius: number,
    health: number,
    on_spawn: (proxy: UnitProxy) -> ()?,
    on_destroyed: (proxy: UnitProxy) -> ()?,
    custom_data: any,
}

export type BaseStats = {
    max_health: number,
    move_speed: number,
    attack_speed: number,
    attack_a: number,
    attack_b: number,
    attack_c: number,
    resistance_a: number,
    resistance_b: number,
    resistance_c: number,
    range: number,
}

export type UnitProto = {
    id: string,
    name: string,
    attack_type: "melee" | "projectile",
    projectile_proto: string?,
    unit_type: "normal" | "champion",
    base_stats: BaseStats,
    model: string,
    on_spawn: (UnitProxy) -> ()?
}

export type CustomData = nil | boolean | number | string | { [string]: CustomData } | { CustomData }

export type EffectProto = {
    id: string,
    update_rate: number,
    on_applied: (UnitProxy, EffectProxy) -> ()?,
    on_removed: (UnitProxy, EffectProxy) -> ()?,
    on_update: (UnitProxy, EffectProxy) -> ()?,
}

export type ProjectileProto = {
    id: string,
    name: string,
    model: string,
    on_spawn: (UnitProxy) -> ()?,
    on_reach_target: (UnitProxy, UnitProxy?) -> ()?,
    on_collide: (UnitProxy, UnitProxy) -> ()?,
    on_destroy: (UnitProxy) -> ()?,
}

export type ProjectileTarget = UnitProxy | Vec2 | Dir2

export type Dir2 = {x: number, y: number, is_dir: true}

declare game: {
    ensure_loaded: (path: string) -> (),
    register_asset: (path: string) -> (),

    register_map: (proto: MapProto) -> MapProto,
    spawn_floor_plane: (args: { dimensions: Vec2, image: string }) -> (),

    register_structure: (proto: StructureProto) -> StructureProto,
    spawn_structure: (args: { proto: string, team: number, position: Vec2 }) -> (),

    load_terrain: (args: { file: string, new_uuids: boolean?, mirror: boolean? }) -> (),

    register_unit: (proto: UnitProto) -> UnitProto,
    spawn_unit: (args: { proto: string, team: number, position: Vec2, data: CustomData }) -> (UnitProxy?),

    register_effect: (proto: EffectProto) -> EffectProto,

    register_projectile: (proto: ProjectileProto) -> ProjectileProto,
    spawn_projectile: (args: { proto: string, position: Vec2, source_unit: UnitProxy, target: ProjectileTarget, speed: number}) -> UnitProxy
}