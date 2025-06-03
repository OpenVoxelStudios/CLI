use serde_json::Value;

use crate::{dir::get_app_support_dir, filesys::get_used_version_save};

pub const MODS: &[&str] = &[
    "dcwa",
    "fabric-api",
    "lithium",
    "iris",
    "modmenu",
    "sodium",
    "placeholder-api",
];
pub const MODS_ID: &[&str] = &[
    "HdwRs3kc", "P7dR8mSH", "gvQqBUqZ", "YL57xq9U", "mOgUt4GM", "AANobbMI", "eXts2L7r",
];

#[derive(Debug)]
pub struct ModDownload {
    pub name: String,
    pub url: String,
}

async fn fetch_mod_version_data(
    client: &reqwest::Client,
    mod_id: &str,
    version: &str,
) -> Result<Option<(String, Vec<String>)>, Box<dyn std::error::Error>> {
    let url = format!(
        "https://api.modrinth.com/v2/project/{}/version?loaders=[\"fabric\"]&game_versions=[\"{}\"]",
        mod_id, version
    );

    let response = client.get(&url).send().await?;
    let json: Value = response.json().await?;

    if let Some(array) = json.as_array() {
        if let Some(first_obj) = array.first() {
            if let Some(files) = first_obj["files"].as_array() {
                if let Some(first_file) = files.first() {
                    if let Some(download_url) = first_file["url"].as_str() {
                        let mut dependencies = Vec::new();

                        if let Some(deps) = first_obj["dependencies"].as_array() {
                            for dep in deps {
                                if let Some(dep_type) = dep["dependency_type"].as_str() {
                                    if dep_type == "required" {
                                        if let Some(dep_id) = dep["project_id"].as_str() {
                                            if !MODS_ID.contains(&dep_id) {
                                                dependencies.push(dep_id.to_string());
                                            }
                                        }
                                    }
                                }
                            }
                        }

                        return Ok(Some((download_url.to_string(), dependencies)));
                    }
                }
            }
        }
    }

    Ok(None)
}

pub async fn get_mod_download_urls(
    version: &str,
) -> Result<Vec<ModDownload>, Box<dyn std::error::Error>> {
    let client = reqwest::Client::new();
    let mut download_urls = Vec::new();
    let mut visited = std::collections::HashSet::new();
    let mut to_process: Vec<String> = MODS.iter().map(|&s| s.to_string()).collect();

    while let Some(mod_id) = to_process.pop() {
        if visited.contains(&mod_id) {
            continue;
        }
        visited.insert(mod_id.clone());

        if let Some((download_url, dependencies)) =
            fetch_mod_version_data(&client, &mod_id, version).await?
        {
            download_urls.push(ModDownload {
                name: mod_id,
                url: download_url,
            });

            for dep_id in dependencies {
                if !visited.contains(&dep_id) {
                    to_process.push(dep_id);
                }
            }
        }
    }

    Ok(download_urls)
}

pub async fn download_mods(version: &str) -> Result<(), Box<dyn std::error::Error>> {
    let previous_version = get_used_version_save();
    if previous_version.is_some() && previous_version.unwrap() == version {
        println!("Mods for version {} already downloaded.", version);
        return Ok(());
    }

    // Ensure the mods directory exists
    let _ = std::fs::create_dir_all(
        get_app_support_dir()
            .unwrap()
            .join(".minecraft")
            .join("mods"),
    );

    for mod_name in MODS {
        let file_path = get_app_support_dir()
            .unwrap()
            .join(".minecraft")
            .join("mods")
            .join(mod_name.to_string() + "-AUTOUPDATE.jar");

        if file_path.exists() {
            std::fs::remove_file(file_path)?;
        }
    }

    let download_urls: Vec<ModDownload> = get_mod_download_urls(version).await?;

    for mod_download in download_urls {
        let response = reqwest::get(&mod_download.url).await?;
        let content = response.bytes().await?;

        std::fs::write(
            get_app_support_dir()
                .unwrap()
                .join(".minecraft")
                .join("mods")
                .join(mod_download.name.clone() + "-AUTOUPDATE.jar"),
            content,
        )?;
        println!("Downloaded mod: {}", mod_download.name);
    }

    Ok(())
}
