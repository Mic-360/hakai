use std::collections::HashMap;
use std::path::{Path, PathBuf};

use anyhow::Result;
use serde::Deserialize;

/// Top-level configuration loaded from `.hakairc`.
#[derive(Debug, Clone, Deserialize, Default)]
#[allow(dead_code)]
pub struct HakaiConfig {
    #[serde(default)]
    pub settings: Settings,
    #[serde(default)]
    pub profiles: HashMap<String, Profile>,
    #[serde(default)]
    pub exclude: ExcludeConfig,
}

#[derive(Debug, Clone, Deserialize)]
#[allow(dead_code)]
pub struct Settings {
    #[serde(default = "default_sort")]
    pub default_sort: String,
    #[serde(default = "default_size_unit")]
    pub size_unit: String,
    #[serde(default = "default_color")]
    pub color: String,
    #[serde(default = "default_true")]
    pub check_updates: bool,
    #[serde(default)]
    pub threads: usize, // 0 = auto
    #[serde(default)]
    pub exclude_hidden: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            default_sort: default_sort(),
            size_unit: default_size_unit(),
            color: default_color(),
            check_updates: true,
            threads: 0,
            exclude_hidden: false,
        }
    }
}

fn default_sort() -> String {
    "path".into()
}
fn default_size_unit() -> String {
    "auto".into()
}
fn default_color() -> String {
    "cyan".into()
}
fn default_true() -> bool {
    true
}

#[derive(Debug, Clone, Deserialize)]
pub struct Profile {
    pub targets: Vec<String>,
}

#[derive(Debug, Clone, Deserialize, Default)]
#[allow(dead_code)]
pub struct ExcludeConfig {
    #[serde(default)]
    pub directories: Vec<String>,
}

/// Built-in profiles available without a config file.
pub fn builtin_profiles() -> HashMap<String, Profile> {
    let mut m = HashMap::new();
    m.insert(
        "node".into(),
        Profile {
            targets: vec!["node_modules".into()],
        },
    );
    m.insert(
        "rust".into(),
        Profile {
            targets: vec!["target".into()],
        },
    );
    m.insert(
        "python".into(),
        Profile {
            targets: vec![
                "__pycache__".into(),
                ".venv".into(),
                "venv".into(),
                ".mypy_cache".into(),
                ".ruff_cache".into(),
                ".pytest_cache".into(),
            ],
        },
    );
    m.insert(
        "flutter".into(),
        Profile {
            targets: vec![
                "build".into(),
                ".dart_tool".into(),
                "ios/Pods".into(),
                "android/build".into(),
                "android/.gradle".into(),
            ],
        },
    );
    m.insert(
        "java".into(),
        Profile {
            targets: vec![
                "build".into(),
                ".gradle".into(),
                "out".into(),
                "target".into(),
            ],
        },
    );
    m.insert(
        "all".into(),
        Profile {
            targets: vec![
                "node_modules".into(),
                "target".into(),
                "__pycache__".into(),
                ".venv".into(),
                "venv".into(),
                "dist".into(),
                ".next".into(),
                ".nuxt".into(),
                ".output".into(),
                ".turbo".into(),
                ".svelte-kit".into(),
                "build".into(),
                ".gradle".into(),
                ".dart_tool".into(),
                ".mypy_cache".into(),
            ],
        },
    );
    m
}

/// Load config from `~/.hakairc` or `./hakairc`, falling back to defaults.
pub fn load_config() -> HakaiConfig {
    // Try local first, then home dir
    let candidates: Vec<PathBuf> = vec![
        PathBuf::from(".hakairc"),
        dirs::home_dir()
            .map(|h| h.join(".hakairc"))
            .unwrap_or_default(),
    ];

    for path in &candidates {
        if let Ok(config) = load_config_from(path) {
            return config;
        }
    }

    // Return default with built-in profiles
    let mut config = HakaiConfig::default();
    config.profiles = builtin_profiles();
    config
}

fn load_config_from(path: &Path) -> Result<HakaiConfig> {
    let content = std::fs::read_to_string(path)?;
    let mut config: HakaiConfig = toml::from_str(&content)?;

    // Merge built-in profiles (user profiles take precedence)
    let builtins = builtin_profiles();
    for (name, profile) in builtins {
        config.profiles.entry(name).or_insert(profile);
    }

    Ok(config)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_config_has_builtin_profiles() {
        let config = HakaiConfig::default();
        let mut config = config;
        config.profiles = builtin_profiles();

        assert!(config.profiles.contains_key("node"));
        assert!(config.profiles.contains_key("rust"));
        assert!(config.profiles.contains_key("python"));
        assert!(config.profiles.contains_key("all"));
    }

    #[test]
    fn parse_toml_config() {
        let toml_str = r#"
[settings]
default_sort = "size"
color = "magenta"

[profiles.custom]
targets = ["my_build"]
"#;
        let config: HakaiConfig = toml::from_str(toml_str).unwrap();
        assert_eq!(config.settings.default_sort, "size");
        assert_eq!(config.settings.color, "magenta");
        assert!(config.profiles.contains_key("custom"));
        assert_eq!(config.profiles["custom"].targets, vec!["my_build"]);
    }
}
