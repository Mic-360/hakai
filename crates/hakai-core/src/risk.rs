use std::path::Path;

use serde::Serialize;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum RiskLevel {
    Low,
    Medium,
    High,
}

#[derive(Debug, Clone, Serialize)]
pub struct RiskResult {
    pub is_dead: bool,
    pub risk_level: RiskLevel,
}

pub fn analyze_risk(path: &Path, target_name: &str) -> RiskResult {
    let parent = path.parent();

    let is_dead = if target_name == "node_modules" {
        match parent {
            Some(p) => !p.join("package.json").exists(),
            None => true,
        }
    } else {
        false
    };

    let risk_level = determine_risk_level(path);

    RiskResult {
        is_dead,
        risk_level,
    }
}

#[inline]
fn prefix_match(haystack: &[u8], needle: &[u8]) -> bool {
    haystack.len() >= needle.len() && haystack[..needle.len()].eq_ignore_ascii_case(needle)
}

#[inline]
fn substring_match(haystack: &[u8], needle: &[u8]) -> bool {
    if needle.is_empty() || needle.len() > haystack.len() {
        return false;
    }
    haystack
        .windows(needle.len())
        .any(|w| w.eq_ignore_ascii_case(needle))
}

#[cfg(windows)]
static HIGH_RISK_PREFIXES: &[&[u8]] = &[
    b"c:\\program files",
    b"c:\\program files (x86)",
    b"c:\\windows",
];

#[cfg(windows)]
static HIGH_RISK_CONTAINS: &[&[u8]] = &[b"\\appdata\\roaming\\npm"];

#[cfg(unix)]
static HIGH_RISK_PREFIXES: &[&[u8]] = &[
    b"/usr/local/lib",
    b"/usr/lib",
    b"/opt/",
    b"/System/",
];

static MEDIUM_RISK_CONTAINS: &[&[u8]] = &[b".nvm", b".volta", b".pnpm-store", b"nvm"];

fn determine_risk_level(path: &Path) -> RiskLevel {
    let path_str = path.to_string_lossy();
    let path_bytes = path_str.as_bytes();

    for prefix in HIGH_RISK_PREFIXES {
        if prefix_match(path_bytes, prefix) {
            return RiskLevel::High;
        }
    }

    #[cfg(windows)]
    for pattern in HIGH_RISK_CONTAINS {
        if substring_match(path_bytes, pattern) {
            return RiskLevel::High;
        }
    }

    for pattern in MEDIUM_RISK_CONTAINS {
        if substring_match(path_bytes, pattern) {
            return RiskLevel::Medium;
        }
    }

    #[cfg(unix)]
    if prefix_match(path_bytes, b"/usr/local/") {
        return RiskLevel::Medium;
    }

    RiskLevel::Low
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    #[test]
    fn non_orphaned_node_modules() {
        let tmp = TempDir::new().unwrap();
        fs::write(tmp.path().join("package.json"), "{}").unwrap();
        let nm = tmp.path().join("node_modules");
        fs::create_dir(&nm).unwrap();

        let result = analyze_risk(&nm, "node_modules");
        assert!(!result.is_dead);
        assert_eq!(result.risk_level, RiskLevel::Low);
    }

    #[test]
    fn orphaned_node_modules() {
        let tmp = TempDir::new().unwrap();
        let nm = tmp.path().join("node_modules");
        fs::create_dir(&nm).unwrap();

        let result = analyze_risk(&nm, "node_modules");
        assert!(result.is_dead);
    }

    #[test]
    fn non_node_modules_target_not_dead() {
        let tmp = TempDir::new().unwrap();
        let target = tmp.path().join("target");
        fs::create_dir(&target).unwrap();

        let result = analyze_risk(&target, "target");
        assert!(!result.is_dead);
    }
}
