use fastnbt::{Value, from_reader};
use flate2::bufread::GzDecoder;
use open_launcher::{Launcher, version};
use reqwest;
use serde::Deserialize;
use std::collections::HashMap;
use std::fs::{self, File};
use std::io::{BufReader, Cursor, Read, Seek, SeekFrom, Write, stdout};
use std::path::{Path, PathBuf};

use crate::auth::get_auth;
use crate::dir::get_minecraft_support_dir;
use crate::filesys::{getsha256, used_version_save};
use crate::get_app_support_dir;
use crate::java::get_java_path;
use crate::map::{Map, install_map};
use crate::mods::download_mods;

#[derive(Debug, Deserialize, Clone)]
pub struct FabricVersion {
    pub loader: FabricVersionId,
}

#[derive(Debug, Deserialize, Clone)]
pub struct FabricVersionId {
    pub version: String,
    pub stable: bool,
}

pub async fn fetch_fabric(
    version: String,
) -> Result<Vec<FabricVersion>, Box<dyn std::error::Error>> {
    let response = reqwest::get(format!(
        "https://meta.fabricmc.net/v2/versions/loader/{}",
        version
    ))
    .await?
    .error_for_status()?;

    let versions: Vec<FabricVersion> = response.json().await?;
    let latest: Vec<FabricVersion> = versions
        .into_iter()
        .filter(|m| m.loader.stable == true)
        .collect();

    Ok(latest)
}

/// Deduplicate libraries by keeping only the highest version of each library
fn deduplicate_libraries(libraries_dir: &PathBuf) -> Result<(), Box<dyn std::error::Error>> {
    let mut library_versions: HashMap<String, Vec<(String, PathBuf)>> = HashMap::new();

    // Only deduplicate ASM libraries - they're the ones causing the main conflict
    // Leave all other libraries alone to avoid version compatibility issues
    let asm_libraries = vec!["asm", "asm-tree", "asm-util", "asm-analysis", "asm-commons"];

    // Recursively search for JAR files in the libraries directory
    find_jar_files(libraries_dir, &mut library_versions)?;

    // Only process ASM libraries
    for (lib_name, mut versions) in library_versions {
        // Only deduplicate ASM libraries
        if !asm_libraries.contains(&lib_name.as_str()) {
            continue;
        }

        if versions.len() > 1 {
            println!(
                "Found {} versions of ASM library '{}': {:?}",
                versions.len(),
                lib_name,
                versions.iter().map(|(v, _)| v).collect::<Vec<_>>()
            );

            // Sort by version
            versions.sort_by(|a, b| compare_versions(&a.0, &b.0));

            // Keep only the highest version
            let highest_version = &versions.last().unwrap().0;
            let mut removed_count = 0;

            // Remove all but the highest version
            for (version, path) in &versions[..versions.len() - 1] {
                if version != highest_version {
                    println!(
                        "Removing duplicate ASM library: {} version {} (keeping version {})",
                        path.display(),
                        version,
                        highest_version
                    );
                    if let Err(e) = fs::remove_file(path) {
                        eprintln!(
                            "Warning: Failed to remove duplicate ASM library {}: {}",
                            path.display(),
                            e
                        );
                    } else {
                        removed_count += 1;
                    }
                }
            }

            if removed_count > 0 {
                println!("Kept ASM {} version {}", lib_name, highest_version);
            }
        }
    }

    Ok(())
}

/// Extract library name from filename (remove version and extension)
fn extract_library_name(filename: &str) -> String {
    // Remove .jar extension
    let name_without_ext = filename.strip_suffix(".jar").unwrap_or(filename);

    // Special cases for ASM library family
    if name_without_ext.starts_with("asm-")
        && !name_without_ext.contains("tree")
        && !name_without_ext.contains("util")
        && !name_without_ext.contains("analysis")
        && !name_without_ext.contains("commons")
    {
        return "asm".to_string();
    }
    if name_without_ext.starts_with("asm-tree-") {
        return "asm-tree".to_string();
    }
    if name_without_ext.starts_with("asm-util-") {
        return "asm-util".to_string();
    }
    if name_without_ext.starts_with("asm-analysis-") {
        return "asm-analysis".to_string();
    }
    if name_without_ext.starts_with("asm-commons-") {
        return "asm-commons".to_string();
    }

    // Handle other common patterns
    let parts: Vec<&str> = name_without_ext.split('-').collect();
    if parts.len() > 1 {
        // Find the first part that looks like a version (starts with digit)
        for (i, part) in parts.iter().enumerate() {
            if part.chars().next().map_or(false, |c| c.is_ascii_digit()) {
                return parts[..i].join("-");
            }
        }
    }

    // Fallback: return the whole name
    name_without_ext.to_string()
}

/// Extract version from filename
fn extract_version(filename: &str) -> Option<String> {
    let name_without_ext = filename.strip_suffix(".jar").unwrap_or(filename);

    // Special cases for ASM library family: asm-9.6.jar -> 9.6
    if name_without_ext.starts_with("asm-")
        && !name_without_ext.contains("tree")
        && !name_without_ext.contains("util")
        && !name_without_ext.contains("analysis")
        && !name_without_ext.contains("commons")
    {
        return name_without_ext.strip_prefix("asm-").map(|s| s.to_string());
    }
    if name_without_ext.starts_with("asm-tree-") {
        return name_without_ext
            .strip_prefix("asm-tree-")
            .map(|s| s.to_string());
    }
    if name_without_ext.starts_with("asm-util-") {
        return name_without_ext
            .strip_prefix("asm-util-")
            .map(|s| s.to_string());
    }
    if name_without_ext.starts_with("asm-analysis-") {
        return name_without_ext
            .strip_prefix("asm-analysis-")
            .map(|s| s.to_string());
    }
    if name_without_ext.starts_with("asm-commons-") {
        return name_without_ext
            .strip_prefix("asm-commons-")
            .map(|s| s.to_string());
    }

    // Look for version patterns in other libraries
    let parts: Vec<&str> = name_without_ext.split('-').collect();
    if parts.len() > 1 {
        // Find the first part that looks like a version
        for part in parts.iter().rev() {
            if part.chars().next().map_or(false, |c| c.is_ascii_digit()) && part.contains('.') {
                return Some(part.to_string());
            }
        }
    }

    None
}

/// Simple version comparison (could be improved with proper semver parsing)
fn compare_versions(v1: &str, v2: &str) -> std::cmp::Ordering {
    let v1_parts: Vec<u32> = v1.split('.').filter_map(|s| s.parse().ok()).collect();
    let v2_parts: Vec<u32> = v2.split('.').filter_map(|s| s.parse().ok()).collect();

    for i in 0..std::cmp::max(v1_parts.len(), v2_parts.len()) {
        let v1_part = v1_parts.get(i).copied().unwrap_or(0);
        let v2_part = v2_parts.get(i).copied().unwrap_or(0);

        match v1_part.cmp(&v2_part) {
            std::cmp::Ordering::Equal => continue,
            other => return other,
        }
    }

    std::cmp::Ordering::Equal
}

pub async fn launch(
    version: String,
    quick_play_map: Option<&String>,
    quick_play_server: Option<&String>,
) {
    let home = get_app_support_dir().unwrap();
    init_minecraft(&version).await;

    let fabric_version = fetch_fabric(version.clone())
        .await
        .ok()
        .and_then(|versions| versions.first().map(|v| v.loader.version.clone()));

    println!("Using Fabric version: {}", fabric_version.clone().unwrap());
    let java_path = get_java_path(&version);
    println!("Using Java path: {}", java_path);

    println!("");
    let mut launcher = Launcher::new(
        home.join(".minecraft").to_str().unwrap(),
        &java_path,
        version::Version {
            minecraft_version: version.clone(),
            loader: Some("fabric".to_string()),
            loader_version: fabric_version,
        },
    )
    .await;

    used_version_save(version);

    launcher.silence(true);
    launcher.auth(get_auth());
    launcher.custom_resolution(1280, 720);
    // launcher.fullscreen(true);

    if let Some(map) = quick_play_map {
        if !map.is_empty() {
            launcher.quick_play("singleplayer", map);
        }
    } else if let Some(server) = quick_play_server {
        if !server.is_empty() {
            launcher.quick_play("multiplayer", server);
        }
    }

    let mut progress = launcher.on_progress();
    tokio::spawn(async move {
        loop {
            match progress.recv().await {
                Ok(progress) => {
                    print!(
                        "\rProgress: {} {}/{} ({}%)",
                        progress.task,
                        progress.current,
                        progress.total,
                        match progress.total {
                            0 => 0,
                            _ => (progress.current as f64 / progress.total as f64 * 100.0 * 100.0)
                                .round() as u64,
                        } as f32
                            / 100.0
                    );
                    stdout().flush().unwrap();
                }
                Err(_) => {
                    println!("Progress channel closed");
                    break;
                }
            }
        }
    });

    match launcher.install_version().await {
        Ok(_) => print!("... version install success\n"),
        Err(e) => println!("An error occurred while installing the version: {}", e),
    };

    match launcher.install_assets().await {
        Ok(_) => print!("... assets install success\n"),
        Err(e) => println!("An error occurred while installing the assets: {}", e),
    };

    match launcher.install_libraries().await {
        Ok(_) => print!("... libraries install success\n"),
        Err(e) => println!("An error occurred while installing the libraries: {}", e),
    };

    // Deduplicate libraries to resolve version conflicts (especially ASM library)
    let libraries_dir = home.join(".minecraft").join("libraries");
    if libraries_dir.exists() {
        println!("Checking for duplicate libraries...");
        if let Err(e) = deduplicate_libraries(&libraries_dir) {
            eprintln!("Warning: Failed to deduplicate libraries: {}", e);
        }
    }

    let process = match launcher.launch() {
        Ok(p) => p,
        Err(e) => {
            println!("An error occurred while launching the game: {}", e);
            std::process::exit(1);
        }
    };

    println!(
        "\nMinecraft launched successfully! Process ID: {}",
        process.id()
    );
}

pub async fn download_resourcepack() {
    let resourcepack_path = get_app_support_dir()
        .unwrap()
        .join(".minecraft")
        .join("resourcepacks")
        .join("OVP.zip");

    match reqwest::get("https://github.com/OpenVoxelStudios/OVP/releases/download/latest/OVP.zip")
        .await
    {
        Ok(response) => {
            let mut file = File::create(&resourcepack_path).unwrap();
            let mut content = Cursor::new(response.bytes().await.unwrap());
            std::io::copy(&mut content, &mut file).unwrap();
        }
        Err(e) => eprintln!("Failed to download resource pack: {}", e),
    }
}

pub async fn init_minecraft(version: &String) {
    let options_exist = get_minecraft_support_dir().unwrap().join("options.txt");

    let options_new = get_app_support_dir()
        .unwrap()
        .join(".minecraft")
        .join("options.txt");

    if !options_new.exists() && options_exist.exists() {
        if let Err(e) = std::fs::copy(options_exist, &options_new) {
            eprintln!("Failed to copy options.txt: {}", e);
        } else {
            println!("Copied options.txt to new location.");
        }
    }

    let resourcepack_path = get_app_support_dir()
        .unwrap()
        .join(".minecraft")
        .join("resourcepacks")
        .join("OVP.zip");

    let resourcepack_shouldsha256 = match reqwest::get(
        "https://github.com/OpenVoxelStudios/OVP/releases/download/latest/OVP.zip.sha256",
    )
    .await
    {
        Ok(response) => match response.error_for_status() {
            Ok(resp) => match resp.text().await {
                Ok(text) => text,
                Err(e) => {
                    eprintln!("Failed to read response text: {}", e);
                    return;
                }
            },
            Err(e) => {
                eprintln!("HTTP error: {}", e);
                return;
            }
        },
        Err(e) => {
            eprintln!("Failed to fetch resourcepack SHA256: {}", e);
            return;
        }
    };

    if resourcepack_path.exists() {
        let resourcepack_issha256 = match getsha256(&resourcepack_path) {
            Ok(sha) => sha,
            Err(e) => {
                eprintln!("Failed to get SHA256: {}", e);
                return;
            }
        };

        if resourcepack_issha256.trim() != resourcepack_shouldsha256.trim() {
            println!("Resource pack SHA256 mismatch, downloading...");
            download_resourcepack().await;
        }
    } else {
        println!("Resource pack not found, downloading...");
        download_resourcepack().await;
    }

    if let Ok(mut options_file) = File::options().read(true).write(true).open(&options_new) {
        let mut contents = String::new();
        options_file.read_to_string(&mut contents).unwrap();

        let mut modified = false;

        let new_contents: String = contents
            .lines()
            .map(|line| {
                if line.trim_start().starts_with("resourcePacks:") {
                    if !line.contains("OVP.zip") {
                        modified = true;
                        if let Some(start) = line.find('[') {
                            let before = &line[..=start];
                            let after = &line[start + 1..line.len() - 1];
                            let mut items: Vec<&str> = after
                                .split(',')
                                .map(|s| s.trim())
                                .filter(|s| !s.is_empty())
                                .collect();
                            items.push("\"OVP.zip\"");
                            return format!("{}{}]", before, items.join(", "));
                        }
                    }
                }
                line.to_string()
            })
            .collect::<Vec<_>>()
            .join("\n");

        if modified {
            options_file.set_len(0).unwrap();
            options_file.seek(SeekFrom::Start(0)).unwrap();
            options_file.write_all(new_contents.as_bytes()).unwrap();
        }
    }

    match download_mods(version).await {
        Ok(()) => {}
        Err(e) => eprintln!("Failed to get mod download URLs: {}", e),
    }
}

/// Recursively find JAR files in the libraries directory and group them by name
fn find_jar_files(
    dir: &PathBuf,
    library_versions: &mut HashMap<String, Vec<(String, PathBuf)>>,
) -> Result<(), Box<dyn std::error::Error>> {
    let entries = fs::read_dir(dir)?;

    for entry in entries {
        let entry = entry?;
        let path = entry.path();

        if path.is_dir() {
            // Recursively search subdirectories
            find_jar_files(&path, library_versions)?;
        } else if path.is_file() && path.extension().and_then(|s| s.to_str()) == Some("jar") {
            if let Some(filename) = path.file_name().and_then(|s| s.to_str()) {
                // Extract library name (everything before the version number)
                let lib_name = extract_library_name(filename);
                let version = extract_version(filename).unwrap_or_else(|| "0.0.0".to_string());

                library_versions
                    .entry(lib_name)
                    .or_insert_with(Vec::new)
                    .push((version, path));
            }
        }
    }

    Ok(())
}

pub fn get_version_name(level_dat: &Path) -> String {
    if let Ok(file) = File::open(level_dat) {
        let reader = BufReader::new(file);
        let mut gz = GzDecoder::new(reader);

        if let Ok(nbt_data) = from_reader::<_, Value>(&mut gz) {
            if let Value::Compound(root) = nbt_data {
                if let Some(Value::Compound(data)) = root.get("Data") {
                    if let Some(Value::Compound(version)) = data.get("Version") {
                        if let Some(Value::String(name)) = version.get("Name") {
                            return name.clone();
                        }
                    }
                }
            }
        }
    }

    "none".to_string()
}

pub async fn run_map(map: Map) {
    let map_path = match install_map(map.id.clone()) {
        Ok(value) => value,
        Err(e) => {
            eprintln!("Error extracting map: {}", e);
            return;
        }
    };

    println!("Launching Minecraft {}...\n", map.version);
    launch(map.version.clone(), Some(&map_path), None).await;
}
