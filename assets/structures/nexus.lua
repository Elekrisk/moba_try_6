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

type MinionSpawnData = {
    wave_delay: number,
    wave_size: number,
    time_til_next_wave: number,
    spawned_units: number,
    wave_count: number,
}

nexus.on_spawn = function(proxy)
    proxy:apply_effect {
        proto = "nexus.minion_spawning",
        data = {
            wave_delay = 30,
            wave_size = 5,
            time_til_next_wave = 0,
            spawned_units = 0,
            wave_count = 0
        } :: MinionSpawnData
    }
end

nexus.on_destroyed = function(self)
    -- The team this nexus belongs to should lose
    -- game.make_lose(self.team)
end

local function straight_path(from: Vec2, to: Vec2)
    local path: { Vec2 } = {}

    local diff = { x = to.x - from.x, y = to.y - from.y }
    local dist = math.sqrt(diff.x * diff.x + diff.y * diff.y)

    local dist_between_waypoints = 2.5

    local waypoint_count = (dist // dist_between_waypoints) + 1

    for i = 1, waypoint_count do
        local t = i / waypoint_count
        local point = { x = from.x * (1 - t) + to.x * t, y = from.y * (1 - t) + to.y * t }
        table.insert(path, point)
    end
    
    return path
end

local function arced_path(from: Vec2, to: Vec2, circle_center: Vec2)
    local radius = math.sqrt((from.x - circle_center.x) * (from.x - circle_center.x) + (from.y - circle_center.y) * (from.y - circle_center.y))
    local to_radius = math.sqrt((to.x - circle_center.x) * (to.x - circle_center.x) + (to.y - circle_center.y) * (to.y - circle_center.y))

    print(`from: \{x = {from.x}, y = {from.y}\}`)
    print(`to: \{x = {to.x}, y = {to.y}\}`)
    print(`circle center: \{x = {circle_center.x}, y = {circle_center.y}\}`)

    local from_r = math.atan2(from.y - circle_center.y, from.x - circle_center.x)
    local to_r = math.atan2(to.y - circle_center.y, to.x - circle_center.x)

    print("from_angle: " .. from_r)
    print("to_angle: " .. to_r)

    -- Which way is shortest?
    -- Try right first; angle should increase
    if from_r > to_r then
        from_r -= 2 * math.pi
    end

    local angle_diff_right = to_r - from_r

    -- Try left; angle should decrease
    if from_r < to_r then
        from_r += 2 * math.pi
    end

    local angle_diff_left = from_r - to_r
    
    local path = {}


    if angle_diff_right < angle_diff_left then
        -- We are moving to the right

        local arclength = angle_diff_right * math.max(radius, to_radius) + math.abs(radius - to_radius)
        
        local dist_between_waypoints = 2.5

        local waypoint_count = (arclength // dist_between_waypoints) + 1


        for i = 1, waypoint_count do
            local t = i / waypoint_count
            local v = from_r + angle_diff_right * t
            local r = radius * (1 - t) + to_radius * t
            local point = { x = circle_center.x + math.cos(v) * r, y = circle_center.y + math.sin(v) * r }
            -- local point = { x = from.x * (1 - t) + to.x * t, y = from.y * (1 - t) + to.y * t }
            table.insert(path, point)
        end
    else 
        -- We are moving to the left

        local arclength = angle_diff_left * math.max(radius, to_radius) + math.abs(radius - to_radius)
        
        local dist_between_waypoints = 2.5

        local waypoint_count = (arclength // dist_between_waypoints) + 1


        for i = 1, waypoint_count do
            local t = i / waypoint_count
            local v = from_r - angle_diff_left * t
            local r = radius * (1 - t) + to_radius * t
            local point = { x = circle_center.x + math.cos(v) * r, y = circle_center.y + math.sin(v) * r }
            -- local point = { x = from.x * (1 - t) + to.x * t, y = from.y * (1 - t) + to.y * t }
            table.insert(path, point)
        end
    end

    print(#path)

    return path
end

local function append_path(to: {Vec2}, from: {Vec2})
    for _, v in from do
        table.insert(to, v)
    end
end

local function top_wave(start: Vec2, endp: Vec2)
    local waypoints = {}

    if start.x < 0 then
        -- we are blue side
        waypoints = {start, {x = start.x, y = 5.0}, {x = -5.0, y = endp.y}, endp}
    else
        -- we are red side
        waypoints = {start, {x = -5.0, y = start.y}, {x = endp.x, y = -5.0}, endp}
    end

    local path = {}
    append_path(path, straight_path(waypoints[1], waypoints[2]))
    append_path(path, arced_path(waypoints[2], waypoints[3], {x = 0.0, y = -0.0}))
    append_path(path, straight_path(waypoints[3], waypoints[4]))
    return path
end

local function top_wave_alt(start: Vec2, endp: Vec2)
    local waypoints = {}

    if start.x < 0 then
        -- we are blue side
        waypoints = {start, {x = start.x, y = 0}, {x = -11.5, y = 11.5}, {x = 0, y = endp.y}, endp}
    else
        -- we are red side
        waypoints = {start, {x = 0, y = start.y}, {x = -11.5, y = 11.5}, {x = endp.x, y = 0}, endp}
    end
    local path = {}
    append_path(path, straight_path(waypoints[1], waypoints[2]))
    append_path(path, arced_path(waypoints[2], waypoints[3], {x = -43, y = 43}))
    append_path(path, arced_path(waypoints[3], waypoints[4], {x = -43, y = 43}))
    append_path(path, straight_path(waypoints[4], waypoints[5]))
    return path
end

local function mid_wave(start: Vec2, endp: Vec2)
    return straight_path(start, endp)
end

local function mid_wave_alt(start: Vec2, endp: Vec2)
    local waypoints = {}

    if start.x < 0 then
        -- we are blue side
        waypoints = {start, {x = -21.5, y = -21.5}, {x = -11.5, y = 11.5}, {x = 21.5, y = 21.5}, endp}
    else
        -- we are red side
        waypoints = {start, {x = 21.5, y = 21.5}, {x = -11.5, y = 11.5}, {x = -21.5, y = -21.5}, endp}
    end
    local path = {}
    append_path(path, straight_path(waypoints[1], waypoints[2]))
    append_path(path, arced_path(waypoints[2], waypoints[3], {x = 20.0, y = -20.0}))
    append_path(path, arced_path(waypoints[3], waypoints[4], {x = 20.0, y = -20.0}))
    append_path(path, straight_path(waypoints[4], waypoints[5]))
    return path
end

local function bot_wave(start: Vec2, endp: Vec2)
    local waypoints = {}

    if start.x < 0 then
        -- we are blue side
        waypoints = {start, {x = 5.0, y = start.y}, {x = endp.x, y = -5.0}, endp}
    else
        -- we are red side
        waypoints = {start, {x = start.x, y = -5.0}, {x = 5.0, y = endp.y}, endp}
    end

    local path = {}
    append_path(path, straight_path(waypoints[1], waypoints[2]))
    append_path(path, arced_path(waypoints[2], waypoints[3], {x = 0.0, y = -0.0}))
    append_path(path, straight_path(waypoints[3], waypoints[4]))
    return path
end

local function adjust_spawn_pos(pos: Vec2, angle: number)
    return {x = pos.x + math.cos(angle) * 2.5, y = pos.y + math.sin(angle) * 2.5}
end

local function spawn_unit(unit: UnitProxy, unit_count: number, wave_count: number)
    local proto = if unit_count <= 3 then "minion" else "minion.ranged"

    local spawn_pos = unit:get_position()

    -- We need to spawn the minion slightly outside the range of the nexus,
    -- to make pathfinding work

    local team = unit:get_team()

    local blue_top_angle =  90
    local blue_mid_angle = 45
    local blue_bot_angle = 0

    local red_top_angle = 180
    local red_mid_angle = 225
    local red_bot_angle = 270

    local other_pos = {x = -spawn_pos.x, y = -spawn_pos.y}

    local blue_pos = if team == 0 then spawn_pos else other_pos
    local red_pos = if team == 0 then other_pos else spawn_pos
    
    local blue_top_spawn = adjust_spawn_pos(blue_pos, math.rad(blue_top_angle))
    local blue_mid_spawn = adjust_spawn_pos(blue_pos, math.rad(blue_mid_angle))
    local blue_bot_spawn = adjust_spawn_pos(blue_pos, math.rad(blue_bot_angle))

    local red_top_spawn = adjust_spawn_pos(red_pos, math.rad(red_top_angle))
    local red_mid_spawn = adjust_spawn_pos(red_pos, math.rad(red_mid_angle))
    local red_bot_spawn = adjust_spawn_pos(red_pos, math.rad(red_bot_angle))

    local top_spawn = if team == 0 then blue_top_spawn else red_top_spawn
    local top_target = if team == 0 then red_top_spawn else blue_top_spawn

    local mid_spawn = if team == 0 then blue_mid_spawn else red_mid_spawn
    local mid_target = if team == 0 then red_mid_spawn else blue_mid_spawn

    local bot_spawn = if team == 0 then blue_bot_spawn else red_bot_spawn
    local bot_target = if team == 0 then red_bot_spawn else blue_bot_spawn
    
    local minion0 = game.spawn_unit {
        proto = proto,
        team = team,
        position = top_spawn,
        data = {
            path = if wave_count % 2 == 0 then top_wave_alt(top_spawn, top_target) else top_wave(top_spawn, top_target),
        }
    }

    local minion1 = game.spawn_unit {
        proto = proto,
        team = team,
        position = mid_spawn,
        data = {
            path = if wave_count % 2 == 0 then mid_wave_alt(mid_spawn, mid_target) else mid_wave(mid_spawn, mid_target),
        }
    }

    local minion2 = game.spawn_unit {
        proto = proto,
        team = team,
        position = bot_spawn,
        data = {
            path = bot_wave(bot_spawn, bot_target),
        }
    }
end

game.register_effect {
    id = "nexus.minion_spawning",
    update_rate = 1,
    on_update = function(unit, effect)
        local data = effect:get_custom_data() :: MinionSpawnData

        data.time_til_next_wave -= 1

        if data.time_til_next_wave <= 0 then
            data.spawned_units = 0
            data.time_til_next_wave = data.wave_delay
            data.wave_count += 1
        end

        if data.spawned_units < data.wave_size then
            spawn_unit(unit, data.spawned_units + 1, data.wave_count)
            data.spawned_units += 1
        end

        effect:set_custom_data(data)
    end
}
