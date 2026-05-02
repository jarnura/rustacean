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

    #[test]
    fn build_fqn_combines_path_and_name() {
        assert_eq!(build_fqn("src/lib.rs", "MyStruct"), "src_lib::MyStruct");
        assert_eq!(
            build_fqn("crates/foo/src/main.rs", "run"),
            "crates_foo_src_main::run"
        );
    }

    #[test]
    fn path_to_module_normalises_separators() {
        assert_eq!(path_to_module("src/my-crate/lib.rs"), "src_my_crate_lib");
    }

    #[test]
    fn collect_rs_files_finds_only_rs_files() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("main.rs"), b"").unwrap();
        std::fs::write(dir.path().join("notes.txt"), b"").unwrap();
        let files = collect_rs_files(dir.path());
        assert_eq!(files.len(), 1);
        assert_eq!(files[0].0, "main.rs");
    }

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

    #[test]
    fn extract_normal_archive_succeeds() {
        let dir = tempfile::tempdir().unwrap();
        let data = make_tar_zst(&[("src/main.rs", b"fn main() {}")]);
        extract_tar_zst(&data, dir.path()).unwrap();
        assert!(dir.path().join("src/main.rs").exists());
    }

    #[test]
    fn validate_dotdot_component_rejected() {
        let path = std::path::PathBuf::from("a/b/../../etc/passwd");
        let err = validate_tar_path(&path).unwrap_err();
        assert!(err.to_string().contains(".."));
    }

    #[test]
    fn validate_absolute_path_rejected() {
        let err = validate_tar_path(Path::new("/etc/passwd")).unwrap_err();
        assert!(err.to_string().contains("absolute"));
    }
}
