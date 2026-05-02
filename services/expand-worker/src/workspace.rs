use std::path::{Path, PathBuf};

use anyhow::{Context as _, Result};

#[derive(Debug)]
pub struct WorkspaceMember {
    pub name: String,
    pub manifest_rel_path: String,
}

pub fn discover_workspace_members(workspace_root: &Path) -> Result<Vec<WorkspaceMember>> {
    let root_manifest = workspace_root.join("Cargo.toml");
    let root_text = std::fs::read_to_string(&root_manifest)
        .with_context(|| format!("failed to read {}", root_manifest.display()))?;
    let root_toml: toml::Value =
        toml::from_str(&root_text).context("failed to parse root Cargo.toml")?;

    if let Some(members) = root_toml
        .get("workspace")
        .and_then(|w| w.get("members"))
        .and_then(|m| m.as_array())
    {
        let mut result = Vec::new();
        for m in members {
            let pat = m.as_str().context("workspace member is not a string")?;
            let member_manifests = glob_manifest_paths(workspace_root, pat);
            for manifest_path in member_manifests {
                if let Ok(name) = read_package_name(&manifest_path) {
                    let rel = manifest_path
                        .strip_prefix(workspace_root)
                        .context("manifest outside workspace root")?
                        .to_string_lossy()
                        .to_string();
                    result.push(WorkspaceMember {
                        name,
                        manifest_rel_path: rel,
                    });
                }
            }
        }
        if !result.is_empty() {
            return Ok(result);
        }
    }

    // Fallback: treat root as a single-crate workspace.
    let name = read_package_name(&root_manifest)?;
    Ok(vec![WorkspaceMember {
        name,
        manifest_rel_path: "Cargo.toml".to_string(),
    }])
}

fn glob_manifest_paths(workspace_root: &Path, pattern: &str) -> Vec<PathBuf> {
    let mut paths = Vec::new();
    let abs_pattern = workspace_root.join(pattern).join("Cargo.toml");
    let abs_str = abs_pattern.to_string_lossy();
    if let Ok(entries) = glob::glob(&abs_str) {
        for entry in entries.flatten() {
            paths.push(entry);
        }
    }
    if paths.is_empty() {
        let direct = workspace_root.join(pattern).join("Cargo.toml");
        if direct.exists() {
            paths.push(direct);
        }
    }
    paths
}

fn read_package_name(manifest_path: &Path) -> Result<String> {
    let text = std::fs::read_to_string(manifest_path)
        .with_context(|| format!("failed to read {}", manifest_path.display()))?;
    let toml: toml::Value = toml::from_str(&text)
        .with_context(|| format!("failed to parse {}", manifest_path.display()))?;
    toml.get("package")
        .and_then(|p| p.get("name"))
        .and_then(|n| n.as_str())
        .map(str::to_owned)
        .context("Cargo.toml missing [package].name")
}

pub fn load_manifest(manifest_path: &Path) -> Result<rb_feature_resolver::CargoManifest> {
    let text = std::fs::read_to_string(manifest_path)
        .with_context(|| format!("failed to read {}", manifest_path.display()))?;
    toml::from_str::<rb_feature_resolver::CargoManifest>(&text)
        .with_context(|| format!("failed to parse manifest {}", manifest_path.display()))
}
