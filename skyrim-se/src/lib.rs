use extism_pdk::*;
use serde::{Deserialize, Serialize};

const OFFICIAL_MASTERS: &[&str] = &[
    "Skyrim.esm",
    "Update.esm",
    "Dawnguard.esm",
    "HearthFires.esm",
    "Dragonborn.esm",
];
const GAME_ROOT_ID: &str = "game";
const LOCAL_APP_DATA_ROOT_ID: &str = "local_app_data";

#[derive(Serialize)]
struct GameMetadata {
    api_version: u32,
    name: String,
    executable: String,
    path_roots: Vec<GamePathRoot>,
    mod_discovery: ModDiscovery,
    load_order_writes: Vec<LoadOrderWriteTarget>,
}

#[derive(Serialize)]
struct GamePathRoot {
    id: String,
    name: String,
    description: String,
}

#[derive(Serialize)]
struct ModDiscovery {
    root_id: String,
    relative_path: String,
    mode: ModDiscoveryMode,
}

#[derive(Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
enum ModDiscoveryMode {
    PluginFiles {
        extensions: Vec<String>,
        excluded_files: Vec<String>,
    },
}

#[derive(Serialize)]
struct LoadOrderWriteTarget {
    root_id: String,
    relative_path: String,
}

#[derive(Deserialize)]
struct ModEntry {
    id: String,
    enabled: bool,
    priority: u32,
}

#[derive(Deserialize)]
struct BuildLoadOrderInput {
    mods: Vec<ModEntry>,
}

#[derive(Serialize)]
struct BuildLoadOrderOutput {
    writes: Vec<GameFileWrite>,
}

#[derive(Serialize)]
struct GameFileWrite {
    root_id: String,
    relative_path: String,
    content: String,
}

#[plugin_fn]
pub fn get_game_metadata() -> FnResult<String> {
    let metadata = GameMetadata {
        api_version: 2,
        name: "Skyrim Special Edition".to_string(),
        executable: "SkyrimSE.exe".to_string(),
        path_roots: vec![
            GamePathRoot {
                id: GAME_ROOT_ID.to_string(),
                name: "Skyrim install folder".to_string(),
                description: "Folder containing SkyrimSE.exe and the Data directory.".to_string(),
            },
            GamePathRoot {
                id: LOCAL_APP_DATA_ROOT_ID.to_string(),
                name: "Skyrim local app data folder".to_string(),
                description: "Folder containing plugins.txt and loadorder.txt.".to_string(),
            },
        ],
        mod_discovery: ModDiscovery {
            root_id: GAME_ROOT_ID.to_string(),
            relative_path: "Data".to_string(),
            mode: ModDiscoveryMode::PluginFiles {
                extensions: vec!["esm".to_string(), "esp".to_string(), "esl".to_string()],
                excluded_files: vec![
                    "Skyrim.esm".to_string(),
                    "Update.esm".to_string(),
                    "Dawnguard.esm".to_string(),
                    "HearthFires.esm".to_string(),
                    "Dragonborn.esm".to_string(),
                ],
            },
        },
        load_order_writes: vec![
            LoadOrderWriteTarget {
                root_id: LOCAL_APP_DATA_ROOT_ID.to_string(),
                relative_path: "plugins.txt".to_string(),
            },
            LoadOrderWriteTarget {
                root_id: LOCAL_APP_DATA_ROOT_ID.to_string(),
                relative_path: "loadorder.txt".to_string(),
            },
        ],
    };
    Ok(serde_json::to_string(&metadata)?)
}

#[plugin_fn]
pub fn build_load_order(input: String) -> FnResult<String> {
    let input: BuildLoadOrderInput = serde_json::from_str(&input)?;
    let output = BuildLoadOrderOutput {
        writes: vec![
            GameFileWrite {
                root_id: LOCAL_APP_DATA_ROOT_ID.to_string(),
                relative_path: "plugins.txt".to_string(),
                content: format_plugins_txt(&input.mods),
            },
            GameFileWrite {
                root_id: LOCAL_APP_DATA_ROOT_ID.to_string(),
                relative_path: "loadorder.txt".to_string(),
                content: format_loadorder_txt(&input.mods),
            },
        ],
    };
    Ok(serde_json::to_string(&output)?)
}

fn sorted_mods(mods: &[ModEntry]) -> Vec<&ModEntry> {
    let mut sorted: Vec<&ModEntry> = mods.iter().collect();
    sorted.sort_by_key(|game_mod| game_mod.priority);
    sorted
}

fn format_plugins_txt(mods: &[ModEntry]) -> String {
    let mut lines = OFFICIAL_MASTERS
        .iter()
        .map(|master| format!("*{master}"))
        .collect::<Vec<_>>();

    for game_mod in sorted_mods(mods) {
        if game_mod.enabled {
            lines.push(format!("*{}", game_mod.id));
        } else {
            lines.push(game_mod.id.clone());
        }
    }

    join_lines_with_trailing_newline(lines)
}

fn format_loadorder_txt(mods: &[ModEntry]) -> String {
    let mut lines = OFFICIAL_MASTERS
        .iter()
        .map(|master| (*master).to_string())
        .collect::<Vec<_>>();

    for game_mod in sorted_mods(mods) {
        lines.push(game_mod.id.clone());
    }

    join_lines_with_trailing_newline(lines)
}

fn join_lines_with_trailing_newline(lines: Vec<String>) -> String {
    let mut content = lines.join("\n");
    content.push('\n');
    content
}

#[cfg(test)]
mod tests {
    use super::*;

    fn mod_entry(id: &str, enabled: bool, priority: u32) -> ModEntry {
        ModEntry {
            id: id.to_string(),
            enabled,
            priority,
        }
    }

    #[test]
    fn plugins_txt_prepends_masters_and_marks_enabled_mods() {
        let mods = vec![
            mod_entry("DisabledPatch.esp", false, 20),
            mod_entry("SkyUI_SE.esp", true, 10),
        ];

        assert_eq!(
            format_plugins_txt(&mods),
            "*Skyrim.esm\n*Update.esm\n*Dawnguard.esm\n*HearthFires.esm\n*Dragonborn.esm\n*SkyUI_SE.esp\nDisabledPatch.esp\n"
        );
    }

    #[test]
    fn loadorder_txt_uses_same_order_without_enabled_prefixes() {
        let mods = vec![
            mod_entry("DisabledPatch.esp", false, 20),
            mod_entry("SkyUI_SE.esp", true, 10),
        ];

        assert_eq!(
            format_loadorder_txt(&mods),
            "Skyrim.esm\nUpdate.esm\nDawnguard.esm\nHearthFires.esm\nDragonborn.esm\nSkyUI_SE.esp\nDisabledPatch.esp\n"
        );
    }
}
