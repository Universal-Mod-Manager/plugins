use extism_pdk::*;
use serde::{Deserialize, Serialize};

const GAME_ROOT_ID: &str = "game";
const DOCUMENTS_ROOT_ID: &str = "documents";

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
    DirectoryMods {
        required_prefix: Option<String>,
        metadata_file: Option<String>,
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
        name: "The Witcher 3: Wild Hunt".to_string(),
        executable: "bin/x64/witcher3.exe".to_string(),
        path_roots: vec![
            GamePathRoot {
                id: GAME_ROOT_ID.to_string(),
                name: "Witcher 3 install folder".to_string(),
                description: "Folder containing bin, content, dlc, and mods.".to_string(),
            },
            GamePathRoot {
                id: DOCUMENTS_ROOT_ID.to_string(),
                name: "Witcher 3 documents folder".to_string(),
                description: "Folder containing mods.settings.".to_string(),
            },
        ],
        mod_discovery: ModDiscovery {
            root_id: GAME_ROOT_ID.to_string(),
            relative_path: "mods".to_string(),
            mode: ModDiscoveryMode::DirectoryMods {
                required_prefix: Some("mod".to_string()),
                metadata_file: None,
            },
        },
        load_order_writes: vec![LoadOrderWriteTarget {
            root_id: DOCUMENTS_ROOT_ID.to_string(),
            relative_path: "mods.settings".to_string(),
        }],
    };
    Ok(serde_json::to_string(&metadata)?)
}

#[plugin_fn]
pub fn build_load_order(input: String) -> FnResult<String> {
    let input: BuildLoadOrderInput = serde_json::from_str(&input)?;
    let output = BuildLoadOrderOutput {
        writes: vec![GameFileWrite {
            root_id: DOCUMENTS_ROOT_ID.to_string(),
            relative_path: "mods.settings".to_string(),
            content: format_mods_settings(&input.mods),
        }],
    };
    Ok(serde_json::to_string(&output)?)
}

fn sorted_mods(mods: &[ModEntry]) -> Vec<&ModEntry> {
    let mut sorted: Vec<&ModEntry> = mods.iter().collect();
    sorted.sort_by_key(|game_mod| game_mod.priority);
    sorted
}

fn format_mods_settings(mods: &[ModEntry]) -> String {
    let sorted = sorted_mods(mods);
    let mut content = String::new();

    for (index, game_mod) in sorted.iter().enumerate() {
        if index > 0 {
            content.push('\n');
        }

        let enabled = if game_mod.enabled { 1 } else { 0 };
        let priority = index + 1;
        content.push_str(&format!(
            "[{}]\nEnabled={enabled}\nPriority={priority}\n",
            game_mod.id
        ));
    }

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
    fn mods_settings_uses_directory_ids_and_one_based_sorted_priorities() {
        let mods = vec![
            mod_entry("modBetterWeather", false, 20),
            mod_entry("modArmorEnhanced", true, 10),
        ];

        assert_eq!(
            format_mods_settings(&mods),
            "[modArmorEnhanced]\nEnabled=1\nPriority=1\n\n[modBetterWeather]\nEnabled=0\nPriority=2\n"
        );
    }
}
