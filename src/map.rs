use reqwest;
use reqwest::blocking;
use serde::Deserialize;
use std::fs;
use std::fs::File;
use std::io;
use std::{error::Error, path::PathBuf};

use crate::{
    cmd::{ask_yes_no, select_from_multiple_maps},
    filesys::{ensure_folder_exists, getsha256},
    get_app_support_dir,
    zipper::{extract_zip, get_root_folder_name},
};

#[derive(Debug, Deserialize, Clone)]
pub struct Map {
    pub id: String,
    pub name: String,
    pub description: String,
    // pub creators: Vec<String>,
    // pub date: u64,
    pub tags: Vec<String>,
    #[serde(rename = "type")]
    pub map_type: String,
    pub version: String,
}

pub fn fetch_maps() -> Result<Vec<Map>, Box<dyn Error>> {
    let response: blocking::Response =
        blocking::get("https://openvoxel.studio/maps.json")?.error_for_status()?;

    let maps: Vec<Map> = response.json()?;
    let maps = maps.into_iter().filter(|m| m.r#map_type == "map").collect();

    Ok(maps)
}

pub fn find_maps(input: String) -> Option<Vec<Map>> {
    let maps: Vec<Map> = fetch_maps().ok()?;
    let input: String = input.to_lowercase();

    let mut exact_matches: Vec<Map> = vec![];

    for map in &maps {
        if map.id.to_lowercase() == input
            || map.name.to_lowercase() == input
            || map.tags.iter().any(|tag| tag.to_lowercase() == input)
        {
            exact_matches.push(map.clone());
        }
    }

    if !exact_matches.is_empty() {
        return Some(exact_matches);
    }

    let keywords: Vec<String> = input.split_whitespace().map(|s| s.to_lowercase()).collect();

    fn match_score(map: &Map, keywords: &[String]) -> usize {
        let all_fields = format!("{} {} {}", map.id, map.name, map.tags.join(" ")).to_lowercase();
        keywords.iter().filter(|k| all_fields.contains(*k)).count()
    }

    let scored: Vec<(Map, usize)> = maps
        .into_iter()
        .map(|m| {
            let score = match_score(&m, &keywords);
            (m, score)
        })
        .filter(|(_, score)| *score > 0)
        .collect();

    if scored.is_empty() {
        return None;
    }

    return Some(scored.into_iter().map(|(m, _)| m).collect());
}

pub fn select_map(input: String) -> Option<Map> {
    let matches = match find_maps(input) {
        Some(maps) => maps,
        None => {
            println!("No maps found.");
            return None;
        }
    };

    if matches.is_empty() {
        println!("No maps found.");
        return None;
    }

    if matches.len() == 1 {
        let map = &matches[0];
        if ask_yes_no(&format!("Play {:?}?", map.name)) {
            return Some(map.clone());
        } else {
            println!("Cancelled.");
            return None;
        }
    } else {
        return select_from_multiple_maps(matches);
    }
}

pub fn download_map(id: String, should_hash: String) -> Result<String, Box<dyn Error>> {
    let map_path = get_app_support_dir()
        .unwrap()
        .join(".cache")
        .join("games")
        .join(format!("{}.zip", id));

    let response = reqwest::blocking::get(format!(
        "https://github.com/OpenVoxelStudios/Maps/releases/latest/download/{}.zip",
        id
    ))?
    .error_for_status()?;

    let mut file = File::create(&map_path)?;
    let mut content = io::Cursor::new(response.bytes()?);
    io::copy(&mut content, &mut file)?;

    let local_hash = getsha256(&map_path)?;
    if local_hash.trim() != should_hash.trim() {
        fs::remove_file(&map_path)?;
        return Err(Box::new(io::Error::new(
            io::ErrorKind::Other,
            "Downloaded map hash does not match expected hash.",
        )));
    }

    println!("Downloaded map to: {:?}", map_path);
    Ok(map_path.to_str().unwrap().to_string())
}

pub fn install_map_from_path(
    map_path: PathBuf,
    overwrite_ask: bool,
) -> Result<String, Box<dyn Error>> {
    let _ = ensure_folder_exists(
        get_app_support_dir()
            .unwrap()
            .join(".minecraft")
            .join("saves")
            .to_str()
            .unwrap(),
    );

    let root_folder_name = get_root_folder_name(&map_path)?;
    println!("Extracting map to .minecraft/saves/{}/", root_folder_name);

    let extract_path = get_app_support_dir()
        .unwrap()
        .join(".minecraft")
        .join("saves")
        .join(&root_folder_name);

    if extract_path.exists() {
        if overwrite_ask {
            if !ask_yes_no(&format!(
                "Map {} already exists. Overwrite?",
                root_folder_name
            )) {
                return Ok(root_folder_name);
            }
        } else {
            println!("Map already exists, using existing map.");
            return Ok(root_folder_name);
        }
    }

    extract_zip(&map_path, &extract_path)?;

    return Ok(root_folder_name);
}

pub fn install_map(id: String) -> Result<String, Box<dyn Error>> {
    let _ = ensure_folder_exists(
        get_app_support_dir()
            .unwrap()
            .join(".cache")
            .join("games")
            .to_str()
            .unwrap(),
    );

    let map_path = get_app_support_dir()
        .unwrap()
        .join(".cache")
        .join("games")
        .join(format!("{}.zip", id));

    let expected_hash = reqwest::blocking::get(format!(
        "https://github.com/OpenVoxelStudios/Maps/releases/latest/download/{}.zip.sha256",
        id
    ))?
    .error_for_status()?;
    let expected_hash = expected_hash.text()?;

    if map_path.exists() {
        println!("Found cached map, verifying hash...");
        let local_hash = getsha256(&map_path)?;

        if local_hash.trim() == expected_hash.trim() {
            println!("Map is already downloaded and verified.");
        } else {
            println!("Map hash does not match, re-downloading...");
            fs::remove_file(&map_path)?;
            let _ = download_map(id.clone(), expected_hash.clone());
        }
    } else {
        let _ = download_map(id.clone(), expected_hash.clone());
    }

    let root_folder_name = install_map_from_path(map_path, false)?;
    return Ok(root_folder_name);
}
