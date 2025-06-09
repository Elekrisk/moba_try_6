-- This will be run on map load
-- We want to firstly spawn a floor plane
-- How should that be done?

-- Make sure our structures are registered

game.ensure_loaded("structures/nexus.lua")
game.ensure_loaded("units/minion.lua")

game.register_asset("./floor.png")
game.register_asset("champs/example_champion/model.glb#Scene0")

for asset=1, 5 do
    -- game.register_asset("dummy" .. asset .. ".dummy")
end



local map = game.register_map {
    id = "default",
    name = "Default",
}

map.on_load = function()
    game.spawn_floor_plane {
        -- dimensions
        dimensions = { x = 100, y = 100 },
        -- image
        image = "./floor.png"
    }

    -- What needs to be done?
    -- Spawning of nexuses

    -- blue nexus
    game.spawn_structure {
        proto = "nexus",
        team = 0,
        position = { x = -42, y = 42 }
    }

    -- red nexus
    game.spawn_structure {
        proto = "nexus",
        team = 1,
        position = { x = 42, y = -42 }
    }

    -- test spawning a unit
    -- for i=1, 200 do
        
    --     game.spawn_unit {
    --         proto = "walking_nexus",
    --         position = { x = math.random(-50, 50), y = math.random(-50, 50) },
    --         team = 1
    --     }
    -- end

    local minion = game.spawn_unit {
        proto = "minion",
        team = 0,
        position = {x = -2.5, y = 2.5}
    }

    local minion = game.spawn_unit {
        proto = "minion",
        team = 1,
        position = {x = 2.5, y = -2.5}
    }

    -- Spawning of turrets
    -- Spawning of terrain

    game.load_terrain{ file = "./blue_terrain.ron"}
    game.load_terrain{ file = "./blue_terrain.ron", new_uuids = true, mirror = true}
    game.load_terrain{ file = "./middle_terrain.ron" }

    -- Spawning of minion waves
end
