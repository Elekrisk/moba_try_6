
game.register_asset("./minion.glb#Scene0")

game.register_unit{
    id = "minion",
    name = "Minion",
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
    model = "./minion.glb#Scene0"
}
