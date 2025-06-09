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
    }
end

nexus.on_destroyed = function(self)
    -- The team this nexus belongs to should lose
    game.make_lose(self.team)
end

game.register_effect {
    id = "nexus.minion_spawning",
    update_rate = 1,
    on_update = function(unit, effect)
        -- I need some way to tell time
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

        local minion = game.spawn_unit {
            proto = "minion",
            team = unit:get_team(),
            position = spawn_pos
        }
        if minion ~= nil then
            minion:set_movement_target(target_pos)
        end
    end
}
