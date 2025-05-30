use clap::{Parser, Subcommand};
use filesys::{copy_dir_all, ensure_folder_exists};
use reqwest::Url;
use std::path::Path;

mod auth;
use auth::{Accounts, add_account, fetch_file, switch_account};
mod cmd;
mod dir;
mod filesys;
mod map;
mod mods;
use cmd::{ask_input, ask_yes_no, select_from_multiple_maps};
use dir::get_app_support_dir;
use map::{Map, fetch_maps, install_map_from_path, select_map};
mod mc;
use mc::{get_version_name, launch, run_map};
mod zipper;

#[derive(Parser)]
#[command(name = "ovl")]
#[command(
    about = "OpenVoxel Launcher",
    long_about = "The OpenVoxel Launcher directly in your terminal!"
)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    #[command(about = "Play an OpenVoxel Map by searching for it by name")]
    Play { game: Vec<String> },
    #[command(about = "Run a specific Minecraft version (e.g. \"1.21.5\") with an optional IP")]
    Run { version: String, ip: Option<String> },
    #[command(
        about = "Open an existing map from the saves or a map from a local path (zip file or folder) or from a URL"
    )]
    #[command(alias = "import")]
    Open { path: String },
    #[command(about = "Select and play a map from the list of available maps")]
    #[command(alias = "list")]
    Search {},

    #[command(about = "Logs in to your Minecraft account and saves it for later use")]
    Login {},
    #[command(about = "Logs out of the selected Minecraft account")]
    Logout {},
    #[command(about = "List all configured Minecraft accounts")]
    #[command(alias = "account")]
    Accounts {},
    #[command(about = "Tells you on what Minecraft account you are currently logged in")]
    #[command(alias = "who-am-i")]
    Whoami {},
}

fn main() {
    let cli = Cli::parse();

    match &cli.command {
        Commands::Login {} => {
            let account = add_account();
            println!("Logged in to {:?}", account.name);
        }

        Commands::Accounts {} => {
            switch_account();
        }

        Commands::Whoami {} => {
            let accounts = fetch_file(false);
            if accounts.accounts.is_empty() {
                println!("No accounts configured.");
            } else {
                let selected_account = accounts
                    .accounts
                    .iter()
                    .find(|a| a.name == accounts.selected);
                match selected_account {
                    Some(account) => {
                        println!(
                            "You are currently logged in as {} (UUID: {}; Offline: {})",
                            account.name,
                            account.uuid.as_deref().unwrap_or("N/A"),
                            account.offline
                        );
                    }
                    None => println!("No account selected."),
                }
            }
        }
        Commands::Logout {} => {
            let accounts = fetch_file(false);
            if accounts.accounts.is_empty() {
                println!("No accounts configured.");
            } else {
                let the_one = accounts
                    .accounts
                    .iter()
                    .find(|a| a.name == accounts.selected);

                if let Some(account) = the_one {
                    account.delete_access_token().unwrap_or_else(|e| {
                        eprintln!("Failed to delete access token: {}", e);
                    });
                }

                let filtered = accounts
                    .accounts
                    .iter()
                    .filter(|a| a.name != accounts.selected)
                    .cloned()
                    .collect::<Vec<_>>();

                let selected = if filtered.is_empty() {
                    String::new()
                } else {
                    filtered[0].name.clone()
                };

                std::fs::write(
                    get_app_support_dir().unwrap().join(".accounts"),
                    serde_json::to_string(&Accounts {
                        accounts: filtered,
                        selected,
                    })
                    .unwrap(),
                )
                .unwrap();

                println!("Logged out of the {} session.", accounts.selected);
            }
        }

        Commands::Play { game } => match select_map(game.join(" ").to_lowercase()) {
            Some(map) => {
                run_map(map);
            }
            None => {}
        },

        Commands::Run { version, ip } => {
            println!("Launching Minecraft {}...\n", version);
            launch(version.clone(), None, ip.as_ref());
        }

        Commands::Open { path } => {
            let input_path = Path::new(path);
            let map_path: String;

            let name_which_exists = get_app_support_dir()
                .unwrap()
                .join(".minecraft")
                .join("saves")
                .join(&path);

            if name_which_exists.exists()
                && ask_yes_no("Map already exists in your saves. Open it?")
            {
                map_path = path.to_string();
            } else {
                if let Ok(url) = Url::parse(path) {
                    if url.scheme() == "https" {
                        println!("Downloading map from URL: {}", url);
                        return ();
                    } else {
                        eprintln!("Invalid URL: must start with https://");
                        return ();
                    }
                } else if input_path.extension().map_or(false, |ext| ext == "zip") {
                    map_path = match install_map_from_path((&input_path).to_path_buf(), true) {
                        Ok(value) => value,
                        Err(e) => {
                            eprintln!("Error extracting map: {}", e);
                            return;
                        }
                    };
                } else if input_path.is_dir() {
                    let _ = ensure_folder_exists(
                        get_app_support_dir()
                            .unwrap()
                            .join(".minecraft")
                            .join("saves")
                            .to_str()
                            .unwrap(),
                    );

                    let map_name = input_path
                        .file_name()
                        .and_then(|name| name.to_str())
                        .unwrap()
                        .to_string();

                    println!("Extracting map to .minecraft/saves/{}/", map_name);

                    let extract_path = get_app_support_dir()
                        .unwrap()
                        .join(".minecraft")
                        .join("saves")
                        .join(&map_name);

                    if extract_path.exists() {
                        if !ask_yes_no(&format!("Map {} already exists. Overwrite?", &map_name)) {
                            return ();
                        }

                        std::fs::remove_dir_all(&extract_path).unwrap_or_else(|e| {
                            eprintln!("Failed to remove extracted folder: {}", e);
                        });
                    }

                    let _ = copy_dir_all(input_path, extract_path);
                    map_path = map_name;
                } else {
                    eprintln!("Invalid path: must be a .zip file, a folder or a URL (https only).");
                    return ();
                }
            }

            let full_map_path = get_app_support_dir()
                .unwrap()
                .join(".minecraft")
                .join("saves")
                .join(&map_path);

            let level_dat = full_map_path.join("level.dat");

            if !level_dat.exists() {
                eprintln!("Error: The map does not contain a valid level.dat file.");
                if ask_yes_no("Delete the extracted folder?") {
                    std::fs::remove_dir_all(&full_map_path).unwrap_or_else(|e| {
                        eprintln!("Failed to remove extracted folder: {}", e);
                    });
                    return ();
                } else {
                    println!("Cancelled.");
                    return ();
                }
            }

            let map_version = get_version_name(&level_dat);

            let version = ask_input(
                &format!(
                    "Enter the Minecraft version (map recommends {})",
                    &map_version
                ),
                Some(&map_version),
            );

            launch(version.clone(), Some(&map_path), None);
        }

        Commands::Search {} => {
            let maps: Vec<Map> = match fetch_maps() {
                Ok(maps) => maps,
                Err(e) => {
                    eprintln!("Error fetching maps: {}", e);
                    return;
                }
            };
            let map = select_from_multiple_maps(maps);

            match map {
                Some(map) => {
                    run_map(map);
                }
                None => println!("No map selected."),
            }
        }
    }
}
