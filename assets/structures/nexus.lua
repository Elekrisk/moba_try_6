-- How would you define a nexus?

game.register_asset("./nexus.glb#Scene0")
game.ensure_loaded("units/minion.lua")

local nexus = game.register_structure {
    id = "nexus",
    name = "Nexus",
    model = "./nexus.glb#Scene0",
    radius = 2.0,
    health = 1000
}

nexus.on_spawn = function(proxy)
    proxy:apply_effect {
        proto = "nexus.minion_spawning",
        data = {
            wave_delay = 30,
            wave_size = 5,
            time_til_next_wave = 0,
            spawned_units = 0,
        }
    }
end

nexus.on_destroyed = function(self)
    -- The team this nexus belongs to should lose
    game.make_lose(self.team)
end

local function spawn_unit(unit)
    local spawn_pos = unit:get_position()

    -- We need to spawn the minion slightly outside the range of the nexus,
    -- to make pathfinding work
    -- Move it towards zero

    if spawn_pos.x < 0 then
        spawn_pos.x = spawn_pos.x + math.cos(math.rad(45)) * 2.5
    else
        spawn_pos.x = spawn_pos.x - math.cos(math.rad(45)) * 2.5
    end

    if spawn_pos.y < 0 then
        spawn_pos.y = spawn_pos.y + math.sin(math.rad(45)) * 2.5
    else
        spawn_pos.y = spawn_pos.y - math.sin(math.rad(45)) * 2.5
    end

    local target_pos = {
        x = math.random() * 10 - 5,
        y = math.random() * 10 - 5,
    }

    local path = {}

    local start = spawn_pos
    local endp = { x = -spawn_pos.x, y = -spawn_pos.y }

    for i = 0, 10 do
        local t = i / 10
        local point = { x = start.x * (1 - t) + endp.x * t, y = start.y * (1 - t) + endp.y * t }
        path[#path+1] = point
    end

    local minion = game.spawn_unit {
        proto = "minion",
        team = unit:get_team(),
        position = spawn_pos,
        data = {
            path = path
        }
    }
end

game.register_effect {
    id = "nexus.minion_spawning",
    update_rate = 1,
    on_update = function(unit, effect)
        local data = effect:get_custom_data()

        print(data.time_til_next_wave)

        data.time_til_next_wave = data.time_til_next_wave - 1

        if data.time_til_next_wave <= 0 then
            data.spawned_units = 0
            data.time_til_next_wave = data.wave_delay
        end

        if data.spawned_units < data.wave_size then
            spawn_unit(unit)
            data.spawned_units = data.spawned_units + 1
        end

        effect:set_custom_data(data)
    end
}
