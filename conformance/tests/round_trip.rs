use serde_json::Value;

fn call(wasm_name: &str, fn_name: &str, input: &str) -> String {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("conformance crate lives in the workspace root")
        .join(format!(
            "target/wasm32-unknown-unknown/release/{wasm_name}.wasm"
        ));
    assert!(
        path.exists(),
        "plugin WASM missing at '{}'; run: cargo build --release --target wasm32-unknown-unknown -p skyrim-se-plugin -p witcher3-plugin -p ck3-plugin",
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

    let input = r#"{"mods":[{"id":"ELFX.esp","name":"ELFX","source_root_id":"game","enabled":false,"priority":30},{"id":"WeatherOverhaul.esp","name":"Weather Overhaul","source_root_id":"game","enabled":true,"priority":10}],"path_roots":{"game":"/games/skyrim","local_app_data":"/games/skyrim-local"},"existing_files":[]}"#;
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

    let input = r#"{"mods":[{"id":"modBetterWeather","name":"modBetterWeather","source_root_id":"game","enabled":false,"priority":20},{"id":"modArmorEnhanced","name":"modArmorEnhanced","source_root_id":"game","enabled":true,"priority":10}],"path_roots":{"game":"/games/witcher3","documents":"/home/user/Documents/The Witcher 3"},"existing_files":[]}"#;
    let output: Value =
        serde_json::from_str(&call("witcher3_plugin", "build_load_order", input)).unwrap();
    let writes = output["writes"].as_array().expect("writes array");
    assert_eq!(writes.len(), 1);
    assert_eq!(
        writes[0]["content"].as_str().unwrap(),
        "[modArmorEnhanced]\nEnabled=1\nPriority=1\n\n[modBetterWeather]\nEnabled=0\nPriority=2\n"
    );
}

#[test]
fn ck3_plugin_wasm_round_trip_builds_dlc_load_and_stubs() {
    let metadata: Value =
        serde_json::from_str(&call("ck3_plugin", "get_game_metadata", "")).unwrap();

    let discoveries = metadata["mod_discoveries"]
        .as_array()
        .expect("mod_discoveries array");
    assert_eq!(discoveries.len(), 2);
    assert_eq!(discoveries[0]["root_id"], "workshop");
    assert_eq!(discoveries[0]["relative_path"], "");
    assert_eq!(discoveries[0]["mode"]["type"], "descriptor_directories");
    assert_eq!(discoveries[0]["mode"]["descriptor_file"], "descriptor.mod");
    assert_eq!(discoveries[1]["root_id"], "user_data");
    assert_eq!(discoveries[1]["relative_path"], "mod");
    assert_eq!(discoveries[1]["mode"]["type"], "descriptor_files");
    assert_eq!(discoveries[1]["mode"]["extension"], "mod");
    assert_eq!(discoveries[1]["mode"]["excluded_prefix"], "ugc_");

    let workshop = metadata["path_roots"]
        .as_array()
        .unwrap()
        .iter()
        .find(|root| root["id"] == "workshop")
        .expect("workshop root");
    assert_eq!(workshop["optional"], true);
    assert_eq!(workshop["auto_detect"]["type"], "steam_workshop");
    assert_eq!(workshop["auto_detect"]["app_id"], 1_158_310);

    let user_data = metadata["path_roots"]
        .as_array()
        .unwrap()
        .iter()
        .find(|root| root["id"] == "user_data")
        .expect("user_data root");
    assert_eq!(user_data["optional"], false);

    let writes = metadata["load_order_writes"].as_array().unwrap();
    let dlc_load = writes
        .iter()
        .find(|write| write["root_id"] == "user_data" && write["relative_path"] == "dlc_load.json")
        .expect("dlc_load.json target");
    assert_eq!(dlc_load["allow_subpaths"], false);
    let mod_stubs = writes
        .iter()
        .find(|write| write["root_id"] == "user_data" && write["relative_path"] == "mod")
        .expect("mod subtree target");
    assert_eq!(mod_stubs["allow_subpaths"], true);

    let descriptors = r##"{"descriptors":[{"id":"2273832430","content":"version=\"1.2\"\ntags={ \"Fixes\" }\n}\nname=\"Community Flavor Pack\"\nsupported_version=\"1.12.*\"\n# comment\n"}]}"##;
    let parsed: Value =
        serde_json::from_str(&call("ck3_plugin", "parse_mod_descriptors", descriptors)).unwrap();
    let game_mod = &parsed["mods"][0];
    assert_eq!(game_mod["name"], "Community Flavor Pack");
    assert_eq!(game_mod["version"], "1.2");
    assert_eq!(game_mod["description"], "Supports CK3 1.12.*");

    let input = r#"{"mods":[{"id":"2273832430","name":"Community Flavor Pack","source_root_id":"workshop","enabled":true,"priority":0},{"id":"coolmod.mod","name":"Cool Local Mod","source_root_id":"user_data","enabled":true,"priority":1},{"id":"111","name":"Off Mod","source_root_id":"workshop","enabled":false,"priority":2}],"path_roots":{"workshop":"/fake/steam/steamapps/workshop/content/1158310","user_data":"/fake/ck3-user-data"},"existing_files":[{"root_id":"user_data","relative_path":"dlc_load.json","content":"{\"disabled_dlcs\":[\"dlc/dlc001.dlc\"],\"enabled_mods\":[\"stale\"]}"}]}"#;
    let output: Value =
        serde_json::from_str(&call("ck3_plugin", "build_load_order", input)).unwrap();
    let writes = output["writes"].as_array().expect("writes array");
    assert_eq!(writes.len(), 2);

    let stub = &writes[0];
    assert_eq!(stub["root_id"], "user_data");
    assert_eq!(stub["relative_path"], "mod/ugc_2273832430.mod");
    assert_eq!(
        stub["content"].as_str().unwrap(),
        "name=\"Community Flavor Pack\"\npath=\"/fake/steam/steamapps/workshop/content/1158310/2273832430\"\nremote_file_id=\"2273832430\"\n"
    );

    let dlc_load = &writes[1];
    assert_eq!(dlc_load["root_id"], "user_data");
    assert_eq!(dlc_load["relative_path"], "dlc_load.json");
    let dlc_load: Value = serde_json::from_str(dlc_load["content"].as_str().unwrap()).unwrap();
    assert_eq!(
        dlc_load["disabled_dlcs"],
        serde_json::json!(["dlc/dlc001.dlc"])
    );
    assert_eq!(
        dlc_load["enabled_mods"],
        serde_json::json!(["mod/ugc_2273832430.mod", "mod/coolmod.mod"])
    );
}
