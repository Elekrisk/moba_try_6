
game.register_asset("./model.glb#Scene0")

game.register_unit{
    id = "example_champion",
    name = "Example Champion",
    unit_type = "champion",
    attack_type = "melee",
    base_stats = {
        max_health = 1000.0,
        move_speed = 2.0,
        attack_speed = 0.7,
        attack_a = 10.0,
        attack_b = 0.0,
        attack_c = 0.0,
        resistance_a = 0.0,
        resistance_b = 0.0,
        resistance_c = 0.0,
        range = 1.0,
    },
    model = "./model.glb#Scene0"
}
