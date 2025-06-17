
game.register_asset("./minion.glb#Scene0")

game.register_unit{
    id = "minion",
    name = "Minion",
    attack_type = "melee",
    base_stats = {
        max_health = 100.0,
        move_speed = 1.5,
        attack_speed = 1.0,
        attack_a = 10.0,
        attack_b = 0.0,
        attack_c = 0.0,
        resistance_a = 0.0,
        resistance_b = 0.0,
        resistance_c = 0.0,
        range = 1.0,
    },
    model = "./minion.glb#Scene0",
    on_spawn = function (unit)
        print("SPAWNED MINION")
        unit:apply_effect{
            proto = "minion.ai",
            data = {
                movement_target = {x = 0, y = 0}
            }
        }
    end
}

-- AI DATA:
--[[

{
    path: list of Vec2
}

]]
game.register_effect{
    id = "minion.ai",
    update_rate = 0.25,
    on_update = function (my_unit, effect)
        -- We need to get all visible enemies in a certain radius
        local units = my_unit:get_visible_units()

        local closest_unit = nil;
        local closest_dist = 1000000.0;

        local my_pos = my_unit:get_position();

        local function sqr_dist(a, b)
            local diff = { x = a.x - b.x, y = a.y - b.y }
            return diff.x * diff.x + diff.y * diff.y
        end

        for _, unit in units do
            local dist = sqr_dist(my_pos, unit:get_position())
            if dist < 25.0 and dist < closest_dist then
                closest_dist = dist
                closest_unit = unit
            end
        end

        if closest_unit ~= nil then
            my_unit:set_attack_target(closest_unit)
        else
            -- No units close enough to target; we instead try to follow our path
            -- Our next waypoint is:
            local data = my_unit:get_custom_data()
            local next_waypoint = data.path[1]

            if next_waypoint == nil then
                -- No path left to walk
                return
            end

            my_unit:set_movement_target(next_waypoint)
            -- If we are close enough, we remove it from our path
            if sqr_dist(my_pos, next_waypoint) < 25.0 then
                table.remove(data.path, 1)
                my_unit:set_custom_data(data)
            end
        end
    end
}
