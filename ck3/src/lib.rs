use extism_pdk::*;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;

const WORKSHOP_ROOT_ID: &str = "workshop";
const USER_DATA_ROOT_ID: &str = "user_data";
const CK3_STEAM_APP_ID: u32 = 1_158_310;

#[derive(Serialize)]
struct GameMetadata {
    api_version: u32,
    name: String,
    executable: String,
    path_roots: Vec<GamePathRoot>,
    mod_discoveries: Vec<ModDiscovery>,
    load_order_writes: Vec<LoadOrderWriteTarget>,
}

#[derive(Serialize)]
struct GamePathRoot {
    id: String,
    name: String,
    description: String,
    optional: bool,
    auto_detect: Option<PathRootAutoDetect>,
}

#[derive(Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
enum PathRootAutoDetect {
    SteamWorkshop { app_id: u32 },
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
    DescriptorDirectories {
        descriptor_file: String,
    },
    DescriptorFiles {
        extension: String,
        excluded_prefix: Option<String>,
    },
}

#[derive(Serialize)]
struct LoadOrderWriteTarget {
    root_id: String,
    relative_path: String,
    allow_subpaths: bool,
}

#[derive(Deserialize)]
struct ParseModDescriptorsInput {
    descriptors: Vec<ModDescriptorSource>,
}

#[derive(Deserialize)]
struct ModDescriptorSource {
    id: String,
    content: String,
}

#[derive(Serialize)]
struct ParseModDescriptorsOutput {
    mods: Vec<ParsedModDescriptor>,
}

#[derive(Serialize)]
struct ParsedModDescriptor {
    id: String,
    name: String,
    version: String,
    description: String,
}

#[derive(Deserialize)]
struct ModEntry {
    id: String,
    name: String,
    source_root_id: String,
    enabled: bool,
    priority: u32,
}

#[derive(Deserialize)]
struct ExistingLoadOrderFile {
    root_id: String,
    relative_path: String,
    content: String,
}

#[derive(Deserialize)]
struct BuildLoadOrderInput {
    mods: Vec<ModEntry>,
    path_roots: HashMap<String, String>,
    existing_files: Vec<ExistingLoadOrderFile>,
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
        name: "Crusader Kings III".to_string(),
        executable: "binaries/ck3.exe".to_string(),
        path_roots: vec![
            GamePathRoot {
                id: WORKSHOP_ROOT_ID.to_string(),
                name: "Steam Workshop mods folder".to_string(),
                description: "Steam Workshop content folder holding your subscribed CK3 mods. Optional: leave unset to manage local mods only."
                    .to_string(),
                optional: true,
                auto_detect: Some(PathRootAutoDetect::SteamWorkshop {
                    app_id: CK3_STEAM_APP_ID,
                }),
            },
            GamePathRoot {
                id: USER_DATA_ROOT_ID.to_string(),
                name: "CK3 user data folder".to_string(),
                description: "Folder containing dlc_load.json and mod/, usually Documents/Paradox Interactive/Crusader Kings III."
                    .to_string(),
                optional: false,
                auto_detect: None,
            },
        ],
        mod_discoveries: vec![
            ModDiscovery {
                root_id: WORKSHOP_ROOT_ID.to_string(),
                relative_path: String::new(),
                mode: ModDiscoveryMode::DescriptorDirectories {
                    descriptor_file: "descriptor.mod".to_string(),
                },
            },
            ModDiscovery {
                root_id: USER_DATA_ROOT_ID.to_string(),
                relative_path: "mod".to_string(),
                mode: ModDiscoveryMode::DescriptorFiles {
                    extension: "mod".to_string(),
                    excluded_prefix: Some("ugc_".to_string()),
                },
            },
        ],
        load_order_writes: vec![
            LoadOrderWriteTarget {
                root_id: USER_DATA_ROOT_ID.to_string(),
                relative_path: "dlc_load.json".to_string(),
                allow_subpaths: false,
            },
            LoadOrderWriteTarget {
                root_id: USER_DATA_ROOT_ID.to_string(),
                relative_path: "mod".to_string(),
                allow_subpaths: true,
            },
        ],
    };
    Ok(serde_json::to_string(&metadata)?)
}

#[plugin_fn]
pub fn parse_mod_descriptors(input: String) -> FnResult<String> {
    let input: ParseModDescriptorsInput = serde_json::from_str(&input)?;
    let output = ParseModDescriptorsOutput {
        mods: input
            .descriptors
            .iter()
            .map(parse_descriptor_source)
            .collect(),
    };
    Ok(serde_json::to_string(&output)?)
}

#[plugin_fn]
pub fn build_load_order(input: String) -> FnResult<String> {
    let input: BuildLoadOrderInput = serde_json::from_str(&input)?;
    let output = build_load_order_output(&input);
    Ok(serde_json::to_string(&output)?)
}

fn parse_descriptor_source(source: &ModDescriptorSource) -> ParsedModDescriptor {
    let mut name = None;
    let mut version = None;
    let mut supported_version = None;

    for line in source.content.lines().map(str::trim) {
        if line.starts_with('#') {
            continue;
        }

        let Some((key, value)) = line.split_once('=') else {
            continue;
        };
        let value = value.trim();
        if !value.starts_with('"') || !value.ends_with('"') || value.len() < 2 {
            continue;
        }
        let value = value[1..value.len() - 1].to_string();

        match key.trim() {
            "name" => name = Some(value),
            "version" => version = Some(value),
            "supported_version" => supported_version = Some(value),
            _ => {}
        }
    }

    ParsedModDescriptor {
        id: source.id.clone(),
        name: name.unwrap_or_else(|| source.id.clone()),
        version: version.unwrap_or_else(|| "1.0".to_string()),
        description: supported_version
            .map(|version| format!("Supports CK3 {version}"))
            .unwrap_or_default(),
    }
}

fn build_load_order_output(input: &BuildLoadOrderInput) -> BuildLoadOrderOutput {
    let workshop_root = input
        .path_roots
        .get(WORKSHOP_ROOT_ID)
        .map(|path| normalize_mods_root_path(path));

    let mut writes = Vec::new();
    let mut enabled_entries = Vec::new();
    for game_mod in sorted_mods(&input.mods) {
        if game_mod.source_root_id == WORKSHOP_ROOT_ID {
            let Some(root) = workshop_root.as_deref() else {
                continue;
            };
            writes.push(GameFileWrite {
                root_id: USER_DATA_ROOT_ID.to_string(),
                relative_path: format!("mod/ugc_{}.mod", game_mod.id),
                content: format_stub_content(game_mod, root),
            });
            enabled_entries.push(format!("mod/ugc_{}.mod", game_mod.id));
        } else {
            enabled_entries.push(format!("mod/{}", game_mod.id));
        }
    }

    writes.push(GameFileWrite {
        root_id: USER_DATA_ROOT_ID.to_string(),
        relative_path: "dlc_load.json".to_string(),
        content: serde_json::json!({
            "disabled_dlcs": preserved_disabled_dlcs(&input.existing_files),
            "enabled_mods": enabled_entries
        })
        .to_string(),
    });

    BuildLoadOrderOutput { writes }
}

fn sorted_mods(mods: &[ModEntry]) -> Vec<&ModEntry> {
    let mut sorted: Vec<&ModEntry> = mods.iter().filter(|game_mod| game_mod.enabled).collect();
    sorted.sort_by_key(|game_mod| game_mod.priority);
    sorted
}

fn normalize_mods_root_path(path: &str) -> String {
    path.replace('\\', "/").trim_end_matches('/').to_string()
}

fn format_stub_content(game_mod: &ModEntry, root: &str) -> String {
    let name = game_mod.name.replace('"', "");
    format!(
        "name=\"{name}\"\npath=\"{root}/{}\"\nremote_file_id=\"{}\"\n",
        game_mod.id, game_mod.id
    )
}

fn preserved_disabled_dlcs(existing_files: &[ExistingLoadOrderFile]) -> Vec<Value> {
    existing_files
        .iter()
        .find(|file| file.root_id == USER_DATA_ROOT_ID && file.relative_path == "dlc_load.json")
        .and_then(|file| serde_json::from_str::<Value>(&file.content).ok())
        .and_then(|json| json.get("disabled_dlcs").and_then(Value::as_array).cloned())
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn workshop_entry(id: &str, name: &str, enabled: bool, priority: u32) -> ModEntry {
        ModEntry {
            id: id.to_string(),
            name: name.to_string(),
            source_root_id: WORKSHOP_ROOT_ID.to_string(),
            enabled,
            priority,
        }
    }

    fn local_entry(id: &str, name: &str, enabled: bool, priority: u32) -> ModEntry {
        ModEntry {
            id: id.to_string(),
            name: name.to_string(),
            source_root_id: USER_DATA_ROOT_ID.to_string(),
            enabled,
            priority,
        }
    }

    fn build_input(
        mods: Vec<ModEntry>,
        existing_files: Vec<ExistingLoadOrderFile>,
    ) -> BuildLoadOrderInput {
        BuildLoadOrderInput {
            mods,
            path_roots: HashMap::from([
                (
                    WORKSHOP_ROOT_ID.to_string(),
                    r#"C:\Steam\steamapps\workshop\content\1158310\"#.to_string(),
                ),
                (
                    USER_DATA_ROOT_ID.to_string(),
                    "/home/user/Documents/Paradox Interactive/Crusader Kings III".to_string(),
                ),
            ]),
            existing_files,
        }
    }

    #[test]
    fn descriptor_parsing_extracts_values_and_falls_back() {
        let parsed = parse_descriptor_source(&ModDescriptorSource {
            id: "2273832430".to_string(),
            content: r#"
                # ignored
                tags={ "Fixes" }
                }
                name="Community Flavor Pack"
                version="1.2"
                supported_version="1.12.*"
            "#
            .to_string(),
        });

        assert_eq!(parsed.name, "Community Flavor Pack");
        assert_eq!(parsed.version, "1.2");
        assert_eq!(parsed.description, "Supports CK3 1.12.*");

        let fallback = parse_descriptor_source(&ModDescriptorSource {
            id: "111".to_string(),
            content: "tags={\n}\npath=mod/foo".to_string(),
        });

        assert_eq!(fallback.name, "111");
        assert_eq!(fallback.version, "1.0");
        assert_eq!(fallback.description, "");
    }

    #[test]
    fn build_load_order_formats_stub_with_normalized_root_and_sanitized_name() {
        let output = build_load_order_output(&build_input(
            vec![workshop_entry(
                "2273832430",
                "Community \"Flavor\" Pack",
                true,
                0,
            )],
            vec![],
        ));

        assert_eq!(output.writes.len(), 2);
        assert_eq!(output.writes[0].root_id, USER_DATA_ROOT_ID);
        assert_eq!(output.writes[0].relative_path, "mod/ugc_2273832430.mod");
        assert_eq!(
            output.writes[0].content,
            "name=\"Community Flavor Pack\"\npath=\"C:/Steam/steamapps/workshop/content/1158310/2273832430\"\nremote_file_id=\"2273832430\"\n"
        );
    }

    #[test]
    fn disabled_mods_are_excluded_from_stubs_and_enabled_mods() {
        let output = build_load_order_output(&build_input(
            vec![
                workshop_entry("2273832430", "On Mod", true, 0),
                workshop_entry("111", "Off Mod", false, 1),
            ],
            vec![],
        ));

        assert_eq!(output.writes.len(), 2);
        assert!(output
            .writes
            .iter()
            .all(|write| !write.relative_path.contains("111")));

        let dlc_load: Value = serde_json::from_str(&output.writes[1].content).unwrap();
        assert_eq!(
            dlc_load["enabled_mods"],
            serde_json::json!(["mod/ugc_2273832430.mod"])
        );
    }

    #[test]
    fn local_mods_reference_their_stub_without_writes_and_mix_with_workshop() {
        let output = build_load_order_output(&build_input(
            vec![
                workshop_entry("2273832430", "Workshop Mod", true, 0),
                local_entry("coolmod.mod", "Cool Local Mod", true, 1),
                local_entry("off.mod", "Disabled Local", false, 2),
            ],
            vec![],
        ));

        assert_eq!(output.writes.len(), 2, "one workshop stub plus dlc_load");
        assert_eq!(output.writes[0].relative_path, "mod/ugc_2273832430.mod");
        let dlc_load: Value = serde_json::from_str(&output.writes[1].content).unwrap();
        assert_eq!(
            dlc_load["enabled_mods"],
            serde_json::json!(["mod/ugc_2273832430.mod", "mod/coolmod.mod"])
        );
    }

    #[test]
    fn workshop_mods_without_configured_workshop_root_are_dropped() {
        let output = build_load_order_output(&BuildLoadOrderInput {
            mods: vec![
                workshop_entry("2273832430", "Orphan Workshop Mod", true, 0),
                local_entry("coolmod.mod", "Cool Local Mod", true, 1),
            ],
            path_roots: HashMap::from([(
                USER_DATA_ROOT_ID.to_string(),
                "/home/user/Documents/Paradox Interactive/Crusader Kings III".to_string(),
            )]),
            existing_files: vec![],
        });

        assert_eq!(
            output.writes.len(),
            1,
            "no stub writes without a workshop root"
        );
        let dlc_load: Value = serde_json::from_str(&output.writes[0].content).unwrap();
        assert_eq!(
            dlc_load["enabled_mods"],
            serde_json::json!(["mod/coolmod.mod"])
        );
    }

    #[test]
    fn disabled_dlcs_are_preserved_or_default_to_empty_array() {
        let existing_files = vec![ExistingLoadOrderFile {
            root_id: USER_DATA_ROOT_ID.to_string(),
            relative_path: "dlc_load.json".to_string(),
            content: r#"{"disabled_dlcs":["dlc/dlc001.dlc"],"enabled_mods":["stale"]}"#.to_string(),
        }];
        let output = build_load_order_output(&build_input(vec![], existing_files));
        let dlc_load: Value = serde_json::from_str(&output.writes[0].content).unwrap();
        assert_eq!(
            dlc_load["disabled_dlcs"],
            serde_json::json!(["dlc/dlc001.dlc"])
        );
        assert_eq!(dlc_load["enabled_mods"], serde_json::json!([]));

        let empty = build_load_order_output(&build_input(vec![], vec![]));
        let empty_dlc_load: Value = serde_json::from_str(&empty.writes[0].content).unwrap();
        assert_eq!(empty_dlc_load["disabled_dlcs"], serde_json::json!([]));

        let invalid = build_load_order_output(&build_input(
            vec![],
            vec![ExistingLoadOrderFile {
                root_id: USER_DATA_ROOT_ID.to_string(),
                relative_path: "dlc_load.json".to_string(),
                content: "not json".to_string(),
            }],
        ));
        let invalid_dlc_load: Value = serde_json::from_str(&invalid.writes[0].content).unwrap();
        assert_eq!(invalid_dlc_load["disabled_dlcs"], serde_json::json!([]));
    }
}
