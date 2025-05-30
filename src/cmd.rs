use colored::Colorize;
use inquire::Select;
use std::io::{self, Write};

use crate::{
    auth::{Account, Accounts},
    map::Map,
};

pub fn ask_yes_no(question: &str) -> bool {
    loop {
        print!("\n{} [Y/n]: ", question);
        io::stdout().flush().expect("Failed to flush stdout");

        let mut input = String::new();
        io::stdin()
            .read_line(&mut input)
            .expect("Failed to read input");

        match input.trim().to_lowercase().as_str() {
            "" | "y" | "ye" | "yes" => return true,
            "n" | "no" => return false,
            _ => {
                return false;
            }
        }
    }
}

pub fn ask_no_yes(question: &str) -> bool {
    loop {
        print!("\n{} [y/N]: ", question);
        io::stdout().flush().expect("Failed to flush stdout");

        let mut input = String::new();
        io::stdin()
            .read_line(&mut input)
            .expect("Failed to read input");

        match input.trim().to_lowercase().as_str() {
            "" | "n" | "no" => return false,
            "y" | "ye" | "yes" => return true,
            _ => {
                return false;
            }
        }
    }
}

pub fn ask_input(question: &str, default: Option<&str>) -> String {
    if let Some(default_val) = default {
        print!("\n{} [{}]: ", question, default_val);
    } else {
        print!("\n{}: ", question);
    }
    io::stdout().flush().expect("Failed to flush stdout");

    let mut input = String::new();
    io::stdin()
        .read_line(&mut input)
        .expect("Failed to read input");

    let trimmed = input.trim();
    if trimmed.is_empty() && default.is_some() {
        default.unwrap().to_string()
    } else {
        trimmed.to_string()
    }
}

pub fn select_from_multiple_maps(maps: Vec<Map>) -> Option<Map> {
    let format_map = |m: &Map| format!("[{:6}] {} - {}", m.version, m.name.bold(), m.description);

    let options: Vec<String> = maps.iter().map(format_map).collect();

    match Select::new("Select a map to play:", options).prompt() {
        Ok(choice) => maps.into_iter().find(|m| format_map(m) == choice),
        Err(_) => {
            println!("Cancelled.");
            None
        }
    }
}

pub fn select_from_multiple_accounts(accounts: Accounts) -> Option<Account> {
    let format_account = |a: &Account| {
        let status = if a.offline { "(Offline)" } else { "(Online)" };
        let name = if a.name == accounts.selected {
            format!("{} {}", a.name.bold().green(), status)
        } else {
            format!("{} {}", a.name.bold(), status)
        };
        name
    };
    let options: Vec<String> = accounts.accounts.iter().map(format_account).collect();
    match Select::new("Select an account:", options).prompt() {
        Ok(choice) => accounts
            .accounts
            .into_iter()
            .find(|a| format_account(a) == choice),
        Err(_) => {
            println!("Cancelled.");
            None
        }
    }
}
