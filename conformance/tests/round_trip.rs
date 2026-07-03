use serde_json::Value;

fn call(wasm_name: &str, fn_name: &str, input: &str) -> String {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("conformance crate lives in the workspace root")
        .join(format!("target/wasm32-unknown-unknown/release/{wasm_name}.wasm"));
    assert!(
        path.exists(),
        "plugin WASM missing at '{}'; run: cargo build --release --target wasm32-unknown-unknown -p skyrim-se-plugin -p witcher3-plugin",
        path.display()
    );
    let manifest = extism::Manifest::new([extism::Wasm::file(&path)]).disallow_all_hosts();
    let mut plugin = extism::PluginBuilder::new(manifest)
        .with_wasi(true)
        .build()
        .expect("load plugin wasm");
    plugin
        .call::<&str, String>(fn_name, input)
        .expect("plugin call")
}

fn declared_write_paths(metadata: &Value) -> Vec<(String, String)> {
    metadata["load_order_writes"]
        .as_array()
        .expect("metadata declares load_order_writes")
        .iter()
        .map(|t| {
            (
                t["root_id"].as_str().unwrap().to_string(),
                t["relative_path"].as_str().unwrap().to_string(),
            )
        })
        .collect()
}

#[test]
fn skyrim_plugin_wasm_round_trip_builds_declared_load_order_writes() {
    let metadata: Value =
        serde_json::from_str(&call("skyrim_se_plugin", "get_game_metadata", "")).unwrap();
    let declared = declared_write_paths(&metadata);
    assert!(declared.contains(&("local_app_data".into(), "plugins.txt".into())));
    assert!(declared.contains(&("local_app_data".into(), "loadorder.txt".into())));

    let input = r#"{"mods":[{"id":"ELFX.esp","enabled":false,"priority":30},{"id":"WeatherOverhaul.esp","enabled":true,"priority":10}]}"#;
    let output: Value =
        serde_json::from_str(&call("skyrim_se_plugin", "build_load_order", input)).unwrap();
    let writes = output["writes"].as_array().expect("writes array");
    assert_eq!(writes.len(), 2);

    let plugins_txt = writes
        .iter()
        .find(|w| w["root_id"] == "local_app_data" && w["relative_path"] == "plugins.txt")
        .expect("plugins.txt write");
    assert_eq!(
        plugins_txt["content"].as_str().unwrap(),
        "*Skyrim.esm\n*Update.esm\n*Dawnguard.esm\n*HearthFires.esm\n*Dragonborn.esm\n*WeatherOverhaul.esp\nELFX.esp\n"
    );
}

#[test]
fn witcher_plugin_wasm_round_trip_builds_declared_mods_settings() {
    let metadata: Value =
        serde_json::from_str(&call("witcher3_plugin", "get_game_metadata", "")).unwrap();
    let declared = declared_write_paths(&metadata);
    assert_eq!(declared[0], ("documents".into(), "mods.settings".into()));

    let input = r#"{"mods":[{"id":"modBetterWeather","enabled":false,"priority":20},{"id":"modArmorEnhanced","enabled":true,"priority":10}]}"#;
    let output: Value =
        serde_json::from_str(&call("witcher3_plugin", "build_load_order", input)).unwrap();
    let writes = output["writes"].as_array().expect("writes array");
    assert_eq!(writes.len(), 1);
    assert_eq!(
        writes[0]["content"].as_str().unwrap(),
        "[modArmorEnhanced]\nEnabled=1\nPriority=1\n\n[modBetterWeather]\nEnabled=0\nPriority=2\n"
    );
}
