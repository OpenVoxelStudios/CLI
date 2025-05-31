use arboard::Clipboard;
use keyring::Entry;
use open_launcher::auth::{self, Auth};
use serde::{Deserialize, Serialize};
use serde_json::from_str;

use crate::{
    cmd::{ask_input, ask_no_yes, select_from_multiple_accounts},
    dir::get_app_support_dir,
};

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Account {
    pub name: String,
    pub uuid: Option<String>,
    pub offline: bool,
}

impl Account {
    fn get_keyring_entry(&self) -> Result<Entry, keyring::Error> {
        let uuid: &String = self.uuid.as_ref().ok_or_else(|| keyring::Error::NoEntry)?;
        Entry::new("openvoxellauncher", uuid)
    }

    pub fn store_access_token(&self, token: &str) -> Result<(), keyring::Error> {
        let entry: Entry = self.get_keyring_entry()?;
        entry.set_password(token)
    }

    pub fn get_access_token(&self) -> Option<String> {
        if self.offline {
            return None;
        }

        match self
            .get_keyring_entry()
            .and_then(|entry: Entry| entry.get_password())
        {
            Ok(token) => Some(token),
            Err(_) => None,
        }
    }

    pub fn delete_access_token(&self) -> Result<(), keyring::Error> {
        if self.offline {
            return Ok(());
        }

        let entry: Entry = self.get_keyring_entry()?;
        entry.delete_credential()
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Accounts {
    pub selected: String,
    pub accounts: Vec<Account>,
}

pub fn fetch_file(should_add: bool) -> Accounts {
    let file = get_app_support_dir().unwrap().join(".accounts");

    if file.exists() {
        let content = std::fs::read_to_string(file).unwrap();
        let accounts: Accounts = from_str(&content).unwrap();

        return accounts;
    } else {
        if should_add {
            println!("\nYou do not have any configured accounts yet. Let's add one!");
            let account = add_account();
            return Accounts {
                selected: account.name.clone(),
                accounts: vec![account],
            };
        } else {
            eprintln!("No accounts file found. Please add an account first.");
            std::process::exit(1);
        }
    }
}

pub fn switch_account() {
    let mut accounts = fetch_file(true);
    let account = select_from_multiple_accounts(accounts.clone());

    match account {
        Some(acc) => {
            accounts.selected = acc.name.clone();
            std::fs::write(
                get_app_support_dir().unwrap().join(".accounts"),
                serde_json::to_string(&accounts).unwrap(),
            )
            .unwrap();
        }
        None => println!("No account selected."),
    }
}

pub fn add_account() -> Account {
    let offline = ask_no_yes("Is the new account offline?");

    let account: Account;

    if offline {
        let name = ask_input("Minecraft offline Username", None);

        if name.is_empty() {
            eprintln!("Username cannot be empty");
            std::process::exit(1);
        }

        account = Account {
            offline: true,
            name: name,
            uuid: None,
        };
    } else {
        account = match tokio::runtime::Runtime::new()
            .unwrap()
            .block_on(online_auth())
        {
            Ok(acc) => acc,
            Err(e) => {
                eprintln!("Failed to authenticate online: {}", e);
                std::process::exit(1);
            }
        };
    }

    let file = get_app_support_dir().unwrap().join(".accounts");
    if file.exists() {
        let mut accounts: Accounts = fetch_file(true);
        accounts.selected = account.name.clone();
        if let Some(existing_account) = accounts
            .accounts
            .iter_mut()
            .find(|a| a.name == account.name)
        {
            *existing_account = account.clone();
        } else {
            accounts.accounts.push(account.clone());
        }
        std::fs::write(file, serde_json::to_string(&accounts).unwrap()).unwrap();
    } else {
        let content = serde_json::to_string(&Accounts {
            selected: account.name.clone(),
            accounts: vec![account.clone()],
        })
        .unwrap();
        std::fs::write(file, content).unwrap();
    }

    return account;
}

pub fn get_auth() -> Auth {
    let mut accounts = fetch_file(true);
    if accounts.accounts.is_empty() {
        if accounts.accounts.len() == 0 {
            println!("\nYou do not have any configured accounts yet. Let's add one!");
            let new_account = add_account();
            accounts.selected = new_account.name.clone();
            accounts.accounts.push(new_account);
        } else {
            accounts.selected = accounts.accounts[0].name.clone();
        }
    }
    let selected_account = accounts
        .accounts
        .iter()
        .find(|a| a.name == accounts.selected)
        .expect("Selected account not found");

    if selected_account.offline {
        return auth::OfflineAuth::new(&selected_account.name);
    } else {
        return auth::Auth::new(
            "msa".to_string(),
            "{}".to_string(),
            selected_account.name.clone(),
            selected_account.uuid.clone().expect(
                "UUID is not defined for this online account. Please log out and in again.",
            ),
            selected_account.get_access_token().expect(
                "Access token is not defined for this online account. Please log out and in again.",
            ),
        );
    }
}

pub async fn online_auth() -> Result<Account, Box<dyn std::error::Error>> {
    println!("Starting Microsoft authentication...");

    let client = reqwest::Client::new();

    // Step 1: Get device code
    let device_response = client
        .post("https://login.microsoftonline.com/consumers/oauth2/v2.0/devicecode")
        .form(&[
            ("client_id", "fe26d9d5-6a19-45a9-b352-abd3e5db37fc"),
            ("scope", "XboxLive.signin offline_access"),
        ])
        .send()
        .await?;

    let device_data: serde_json::Value = device_response.json().await?;
    let user_code = device_data["user_code"].as_str().unwrap();
    let device_code = device_data["device_code"].as_str().unwrap();
    let verification_uri = device_data["verification_uri"].as_str().unwrap();

    println!("\nPlease visit: {}", verification_uri);
    println!("And enter the code: {}", user_code);
    ask_input("--> Press Enter to open link and copy code", None);

    let mut clipboard = Clipboard::new().unwrap();
    clipboard.set_text(user_code).unwrap();
    let _ = open::that(verification_uri);

    println!("Waiting for authentication...");

    // Step 2: Poll for access token
    let msa_token = loop {
        tokio::time::sleep(tokio::time::Duration::from_secs(5)).await;

        let token_response = client
            .post("https://login.microsoftonline.com/consumers/oauth2/v2.0/token")
            .form(&[
                ("grant_type", "urn:ietf:params:oauth:grant-type:device_code"),
                ("client_id", "fe26d9d5-6a19-45a9-b352-abd3e5db37fc"),
                ("device_code", device_code),
            ])
            .send()
            .await?;

        let token_data: serde_json::Value = token_response.json().await?;

        if let Some(error) = token_data["error"].as_str() {
            if error == "authorization_pending" {
                continue;
            } else {
                return Err(format!("OAuth error: {}", error).into());
            }
        }

        break token_data["access_token"].as_str().unwrap().to_string();
    };

    // Step 3: Get Xbox Live token
    let xbl_response = client
        .post("https://user.auth.xboxlive.com/user/authenticate")
        .header("Content-Type", "application/json")
        .json(&serde_json::json!({
            "Properties": {
                "AuthMethod": "RPS",
                "SiteName": "user.auth.xboxlive.com",
                "RpsTicket": format!("d={}", msa_token)
            },
            "RelyingParty": "http://auth.xboxlive.com",
            "TokenType": "JWT"
        }))
        .send()
        .await?;

    let xbl_data: serde_json::Value = xbl_response.json().await?;
    let xbl_token = xbl_data["Token"].as_str().unwrap();
    let user_hash = xbl_data["DisplayClaims"]["xui"][0]["uhs"].as_str().unwrap();

    // Step 4: Get XSTS token
    let xsts_response = client
        .post("https://xsts.auth.xboxlive.com/xsts/authorize")
        .header("Content-Type", "application/json")
        .json(&serde_json::json!({
            "Properties": {
                "SandboxId": "RETAIL",
                "UserTokens": [xbl_token]
            },
            "RelyingParty": "rp://api.minecraftservices.com/",
            "TokenType": "JWT"
        }))
        .send()
        .await?;

    let xsts_data: serde_json::Value = xsts_response.json().await?;
    let xsts_token = xsts_data["Token"].as_str().unwrap();

    // Step 5: Get Minecraft access token
    let mc_response = client
        .post("https://api.minecraftservices.com/authentication/login_with_xbox")
        .header("Content-Type", "application/json")
        .json(&serde_json::json!({
            "identityToken": format!("XBL3.0 x={};{}", user_hash, xsts_token)
        }))
        .send()
        .await?;

    let mc_data: serde_json::Value = mc_response.json().await?;
    let mc_access_token = mc_data["access_token"].as_str().unwrap();

    // Step 6: Get Minecraft profile
    let profile_response = client
        .get("https://api.minecraftservices.com/minecraft/profile")
        .header("Authorization", format!("Bearer {}", mc_access_token))
        .send()
        .await?;

    let profile_data: serde_json::Value = profile_response.json().await?;
    let username = profile_data["name"].as_str().unwrap();
    let uuid = profile_data["id"].as_str().unwrap();

    println!("Successfully authenticated as: {}", username);

    let fresh_account = Account {
        name: username.to_string(),
        uuid: Some(uuid.to_string()),
        offline: false,
    };
    fresh_account.store_access_token(mc_access_token).unwrap();

    Ok(fresh_account)
}
