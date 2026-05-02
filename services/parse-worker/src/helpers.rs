use std::path::{Path, PathBuf};

use anyhow::{Context as _, Result};

pub(crate) fn collect_rs_files(root: &Path) -> Vec<(String, PathBuf)> {
    walkdir::WalkDir::new(root)
        .into_iter()
        .filter_map(Result::ok)
        .filter(|e| {
            e.file_type().is_file()
                && e.path().extension().and_then(std::ffi::OsStr::to_str) == Some("rs")
        })
        .filter_map(|e| {
            let rel = e
                .path()
                .strip_prefix(root)
                .ok()?
                .to_str()?
                .replace('\\', "/");
            Some((rel, e.into_path()))
        })
        .collect()
}

pub(crate) fn extract_tar_zst(data: &[u8], dest: &Path) -> Result<()> {
    let zstd_reader = zstd::Decoder::new(data).context("failed to create zstd decoder")?;
    let mut archive = tar::Archive::new(zstd_reader);

    for entry in archive.entries().context("failed to read tar entries")? {
        let mut entry = entry.context("failed to read tar entry")?;
        let path = entry.path().context("failed to get entry path")?;
        let dest_path = dest.join(path.as_ref());
        if let Some(parent) = dest_path.parent() {
            std::fs::create_dir_all(parent).context("failed to create parent dir")?;
        }
        entry
            .unpack(&dest_path)
            .context("failed to unpack tar entry")?;
    }
    Ok(())
}

pub(crate) fn path_to_module(rel_path: &str) -> String {
    rel_path
        .trim_end_matches(".rs")
        .replace(['/', '\\', '-'], "_")
        .replace("::", "_")
}

pub(crate) fn build_fqn(rel_path: &str, name: &str) -> String {
    let module = path_to_module(rel_path);
    if module.is_empty() {
        name.to_owned()
    } else {
        format!("{module}::{name}")
    }
}
