use std::path::{Component, Path, PathBuf};

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

fn validate_tar_path(path: &Path) -> Result<()> {
    anyhow::ensure!(
        !path.is_absolute(),
        "tar entry has absolute path: {}",
        path.display()
    );
    anyhow::ensure!(
        !path.components().any(|c| c == Component::ParentDir),
        "tar entry escapes destination via '..': {}",
        path.display()
    );
    Ok(())
}

pub(crate) fn extract_tar_zst(data: &[u8], dest: &Path) -> Result<()> {
    let zstd_reader = zstd::Decoder::new(data).context("failed to create zstd decoder")?;
    let mut archive = tar::Archive::new(zstd_reader);

    for entry in archive.entries().context("failed to read tar entries")? {
        let mut entry = entry.context("failed to read tar entry")?;
        let path = entry.path().context("failed to get entry path")?;
        validate_tar_path(path.as_ref())?;
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

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn make_tar_zst(entries: &[(&str, &[u8])]) -> Vec<u8> {
        let mut tar_buf = Vec::new();
        {
            let mut builder = tar::Builder::new(&mut tar_buf);
            for (path, data) in entries {
                let mut header = tar::Header::new_gnu();
                header.set_size(data.len() as u64);
                header.set_mode(0o644);
                header.set_cksum();
                builder.append_data(&mut header, path, *data).unwrap();
            }
            builder.finish().unwrap();
        }
        zstd::encode_all(tar_buf.as_slice(), 0).unwrap()
    }

    // validate_tar_path tests exercise the guard directly with crafted Path values,
    // because the tar builder itself rejects '..' paths before they reach a real archive.
    #[test]
    fn validate_dotdot_component_rejected() {
        let err = validate_tar_path(Path::new("../escape.txt")).unwrap_err();
        assert!(
            err.to_string().contains(".."),
            "expected traversal error, got: {err}"
        );
    }

    #[test]
    fn validate_nested_dotdot_rejected() {
        let err = validate_tar_path(Path::new("a/b/../../etc/passwd")).unwrap_err();
        assert!(
            err.to_string().contains(".."),
            "expected traversal error, got: {err}"
        );
    }

    #[test]
    fn validate_absolute_path_rejected() {
        let err = validate_tar_path(Path::new("/etc/passwd")).unwrap_err();
        assert!(
            err.to_string().contains("absolute"),
            "expected absolute-path error, got: {err}"
        );
    }

    #[test]
    fn validate_normal_path_ok() {
        validate_tar_path(Path::new("src/main.rs")).unwrap();
        validate_tar_path(Path::new("a/b/c.rs")).unwrap();
    }

    #[test]
    fn extract_normal_archive_succeeds() {
        let dir = TempDir::new().unwrap();
        let data = make_tar_zst(&[("src/main.rs", b"fn main() {}")]);
        extract_tar_zst(&data, dir.path()).unwrap();
        assert!(dir.path().join("src/main.rs").exists());
    }
}
