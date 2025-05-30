use std::collections::HashSet;
use std::fs;
use std::fs::File;
use std::io::{self};
use std::path::Path;
use zip::ZipArchive;

pub fn get_root_folder_name(zip_path: &Path) -> std::io::Result<String> {
    let file = File::open(zip_path)?;
    let mut archive = ZipArchive::new(file)?;

    let mut top_dirs = HashSet::new();

    for i in 0..archive.len() {
        let entry = archive.by_index(i)?;
        let path = entry.enclosed_name().unwrap();

        if let Some(component) = path.components().next() {
            top_dirs.insert(component.as_os_str().to_string_lossy().to_string());
        }
    }

    if top_dirs.len() == 1 {
        Ok(top_dirs.into_iter().next().unwrap())
    } else {
        Ok(zip_path.file_stem().unwrap().to_string_lossy().to_string())
    }
}

pub fn extract_zip(zip_path: &Path, extract_to: &Path) -> zip::result::ZipResult<()> {
    let file = File::open(zip_path)?;
    let mut archive = ZipArchive::new(file)?;

    let mut top_dirs = vec![];
    for i in 0..archive.len() {
        let name = archive.by_index(i)?.name().to_string();
        if let Some(first) = Path::new(&name).components().next() {
            let dir = first.as_os_str().to_string_lossy().to_string();
            if !top_dirs.contains(&dir) {
                top_dirs.push(dir);
            }
        }
    }

    let strip_prefix = if top_dirs.len() == 1 {
        Some(Path::new(&top_dirs[0]))
    } else {
        None
    };

    for i in 0..archive.len() {
        let mut file = archive.by_index(i)?;
        let out_path = match strip_prefix {
            Some(prefix) => {
                let path = Path::new(file.name());
                match path.strip_prefix(prefix) {
                    Ok(stripped) => extract_to.join(stripped),
                    Err(_) => extract_to.join(path),
                }
            }
            None => extract_to.join(file.name()),
        };

        if file.name().ends_with('/') {
            fs::create_dir_all(&out_path)?;
        } else {
            if let Some(parent) = out_path.parent() {
                fs::create_dir_all(parent)?;
            }
            let mut out_file = File::create(&out_path)?;
            io::copy(&mut file, &mut out_file)?;
        }
    }

    Ok(())
}
