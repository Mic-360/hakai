use std::path::Path;

use serde::Serialize;

/// Risk levels assigned to discovered directories.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum RiskLevel {
    Low,
    Medium,
    High,
}

/// Result of risk analysis for a single found directory.
#[derive(Debug, Clone, Serialize)]
pub struct RiskResult {
    pub is_dead: bool,
    pub risk_level: RiskLevel,
}

/// Analyze the risk of deleting the directory at `path`.
///
/// Logic (matching npkill):
/// - **Dead/orphaned**: The parent directory has no `package.json` (for node_modules targets).
/// - **High risk**: Path is in a global/system location.
/// - **Medium risk**: Path appears to be a globally-installed package.
/// - **Low risk**: Normal project node_modules.
pub fn analyze_risk(path: &Path, target_name: &str) -> RiskResult {
    let parent = path.parent();

    // Check if the parent dir has a package.json — if not, the node_modules is orphaned.
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

fn determine_risk_level(path: &Path) -> RiskLevel {
    let path_str = path.to_string_lossy().to_lowercase();

    // High risk: system-level locations
    #[cfg(windows)]
    {
        if path_str.starts_with("c:\\program files")
            || path_str.starts_with("c:\\program files (x86)")
            || path_str.starts_with("c:\\windows")
            || path_str.contains("\\appdata\\roaming\\npm")
        {
            return RiskLevel::High;
        }
    }

    #[cfg(unix)]
    {
        if path_str.starts_with("/usr/local/lib")
            || path_str.starts_with("/usr/lib")
            || path_str.starts_with("/opt/")
            || path_str.starts_with("/System/")
        {
            return RiskLevel::High;
        }
    }

    // Medium risk: global npm/nvm/volta/pnpm locations
    if path_str.contains(".nvm")
        || path_str.contains(".volta")
        || path_str.contains(".pnpm-store")
        || path_str.contains("nvm")
    {
        return RiskLevel::Medium;
    }

    #[cfg(unix)]
    if path_str.starts_with("/usr/local/") {
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
        // No package.json in parent

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
