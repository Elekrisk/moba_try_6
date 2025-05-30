use engine_common::*;
use ron::ser::PrettyConfig;

fn main() {
    let example = ChampionDef::example();

    let config = PrettyConfig::new().struct_names(true);

    let json = serde_json::to_string_pretty(&example).unwrap();
    std::fs::write("example_champ.json", json).unwrap();
    let ron = ron::ser::to_string_pretty(&example, config.clone()).unwrap();
    std::fs::write("example_champ.ron", ron).unwrap();
    let ron = ron::ser::to_string_pretty(&ChampList::example(), config).unwrap();
    std::fs::write("champ_list.ron", ron).unwrap();
}
