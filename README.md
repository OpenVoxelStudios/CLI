# OpenVoxel CLI (ovl)

The OpenVoxel Launcher directly in your terminal! A command-line interface for managing Minecraft accounts, downloading maps, and launching Minecraft with OpenVoxel Studios content.

## Features

- 🔐 **Secure Authentication:** Uses system keychain to store Minecraft access tokens
- 🗺️ **Map Management:** Download, import, and organize Minecraft maps
- 🔍 **Version Detection:** Automatically detects recommended Minecraft versions from map data
- 👥 **Multiple Accounts:** Switch between different Minecraft accounts easily
- 🌐 **Cross-Platform:** Works on macOS, Windows, and Linux

## Installation

### Homebrew (macOS/Linux)

```bash
brew tap OpenVoxelStudios/cli
brew install ovl
```

### Cargo (Rust Package Manager)

```bash
cargo install --git https://github.com/OpenVoxelStudios/CLI.git
```

## Examples

```bash
# This will download and run the OpenVoxel map "Bendy and the Ink Machine"
ovl play batim

# This will open minecraft on 1.21.5
ovl run 1.21.5
```

## Commands

### Account Management

#### `ovl login`

Authenticate with your Minecraft account and save credentials securely in your system keychain.

#### `ovl logout`

Sign out of the currently selected account and remove stored credentials.

#### `ovl accounts`

> **Alias:** `account`

List all configured Minecraft accounts and switch between them.

#### `ovl whoami`

> **Alias:** `who-am-i`

Display information about the currently selected account (name, UUID, online/offline status).

### Playing Maps

#### `ovl play <game>`

Search for and launch an OpenVoxel map by name.

```bash
ovl play BATIM
ovl play lethal budget
```

#### `ovl search`

> **Alias:** `list`

Browse all available OpenVoxel maps in an interactive menu and select one to play.

#### `ovl open <path>`

> **Alias:** `import`

Import and launch a map from various sources:

- **Local folder:** `ovl open /path/to/map/folder`
- **ZIP file:** `ovl open /path/to/map.zip`
- **HTTPS URL:** `ovl open https://example.com/map.zip`
- **Existing save:** `ovl open "My World"`

It will auto-detect the Minecraft version and ask for confirmation before launch.

### Direct Minecraft Launch

#### `ovl run <version> [ip]`

Launch a specific Minecraft version directly, optionally connecting to a server.

```bash
ovl run 1.21.5
ovl run 1.20.1 mc.hypixel.net
```

## Requirements

- An installed Java version at /usr/bin/java (Java management is WIP)

## Support

For issues and feature requests, visit: https://github.com/OpenVoxelStudios/CLI/issues

Or join our Discord: https://discord.gg/cRFHWgnWjE
