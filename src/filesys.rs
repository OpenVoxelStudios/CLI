use std::fs::File;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::{fs, io};

use crate::dir::get_app_support_dir;

pub fn ensure_folder_exists(path: &str) -> std::io::Result<()> {
    let folder = Path::new(path);
    if !folder.exists() {
        fs::create_dir_all(folder)?;
    }
    Ok(())
}

pub fn getsha256(path: &PathBuf) -> Result<String, Box<dyn std::error::Error>> {
    let bytes = std::fs::read(&path).unwrap();
    let local_hash = sha256::digest(&bytes);
    return Ok(local_hash);
}

pub fn copy_dir_all(src: impl AsRef<Path>, dst: impl AsRef<Path>) -> io::Result<()> {
    fs::create_dir_all(&dst)?;
    for entry in fs::read_dir(src)? {
        let entry = entry?;
        let ty = entry.file_type()?;
        if ty.is_dir() {
            copy_dir_all(entry.path(), dst.as_ref().join(entry.file_name()))?;
        } else {
            fs::copy(entry.path(), dst.as_ref().join(entry.file_name()))?;
        }
    }
    Ok(())
}

pub fn used_version_save(version: String) {
    if let Ok(mut file) = File::create(
        get_app_support_dir()
            .unwrap()
            .join(".minecraft")
            .join("mods")
            .join(".ovl")
            .to_str()
            .unwrap(),
    ) {
        let _ = file.write_all(version.as_bytes());
    }
}

pub fn get_used_version_save() -> Option<String> {
    let path = get_app_support_dir()
        .unwrap()
        .join(".minecraft")
        .join("mods")
        .join(".ovl");

    if path.exists() {
        if let Ok(content) = fs::read_to_string(path) {
            return Some(content.trim().to_string());
        }
    }
    None
}
