-- How would you define a nexus?

game.register_asset("./nexus.glb#Scene0")

local nexus = game.register_structure {
    id = "nexus",
    name = "Nexus",
    model = "./nexus.glb#Scene0",
    radius = 2.0,
    health = 1000
}

nexus.on_destroyed = function(self)
    -- The team this nexus belongs to should lose
    game.make_lose(self.team)
end
