use std::fs;
use std::process::Command;

use crate::dir::get_app_support_dir;

// TODO: Add java handling for every MC version
pub fn get_java_path(_version: &String) -> String {
    let java_path_file = get_app_support_dir()
        .unwrap()
        .join("settings")
        .join("java_path.txt");

    // First, try to read cached path from java_path.txt
    if let Ok(cached_path) = fs::read_to_string(&java_path_file) {
        let cached_path = cached_path.trim();
        if !cached_path.is_empty() {
            // Test if cached path still works
            if test_java_path(cached_path) {
                return cached_path.to_string();
            } else {
                eprintln!("Cached Java path no longer works, re-detecting...");
            }
        }
    }

    // Auto-detect and cache the path
    let java_path = match check_java_version() {
        Ok(version) => {
            if version >= 21 {
                match get_java_executable_path() {
                    Ok(path) => path,
                    Err(e) => {
                        eprintln!("Error finding Java executable: {}", e);
                        std::process::exit(1);
                    }
                }
            } else {
                eprintln!(
                    "Error: Java version {} is outdated. Java > 21 is required.",
                    version
                );
                std::process::exit(1);
            }
        }
        Err(e) => {
            eprintln!("Error checking Java version: {}", e);
            std::process::exit(1);
        }
    };

    // Create directory if it doesn't exist
    if let Some(parent_dir) = java_path_file.parent() {
        if let Err(e) = fs::create_dir_all(parent_dir) {
            eprintln!("Warning: Could not create settings directory: {}", e);
        }
    }

    // Save the found path to cache file
    if let Err(e) = fs::write(&java_path_file, &java_path) {
        eprintln!("Warning: Could not save Java path to cache: {}", e);
    }

    java_path
}

fn test_java_path(java_path: &str) -> bool {
    Command::new(java_path)
        .arg("-version")
        .output()
        .map(|output| output.status.success())
        .unwrap_or(false)
}

fn check_java_version() -> Result<u32, String> {
    let output = Command::new("java")
        .arg("-version")
        .output()
        .map_err(|e| format!("Failed to execute java -version: {}", e))?;
    if !output.status.success() {
        return Err("Java command failed".to_string());
    }
    // Java version info goes to stderr
    let version_output = String::from_utf8_lossy(&output.stderr);
    // Parse version from output like: java version "21.0.1" or java version "1.8.0_391"
    for line in version_output.lines() {
        if line.contains("java version") || line.contains("openjdk version") {
            if let Some(version_str) = extract_version_from_line(line) {
                return parse_major_version(&version_str);
            }
        }
    }
    Err("Could not parse Java version from output".to_string())
}

fn extract_version_from_line(line: &str) -> Option<String> {
    // Find version string between quotes
    let start = line.find('"')? + 1;
    let end = line[start..].find('"')? + start;
    Some(line[start..end].to_string())
}

fn parse_major_version(version_str: &str) -> Result<u32, String> {
    // Handle both old format (1.8.0_391) and new format (21.0.1)
    let parts: Vec<&str> = version_str.split('.').collect();
    if parts.is_empty() {
        return Err("Invalid version format".to_string());
    }
    let major_version = if parts[0] == "1" && parts.len() > 1 {
        // Old format: 1.8.0_391 -> major version is 8
        parts[1]
            .parse::<u32>()
            .map_err(|_| "Could not parse major version".to_string())?
    } else {
        // New format: 21.0.1 -> major version is 21
        parts[0]
            .parse::<u32>()
            .map_err(|_| "Could not parse major version".to_string())?
    };
    Ok(major_version)
}

fn get_java_executable_path() -> Result<String, String> {
    // First, try to find java executable using 'where' on Windows or 'which' on Unix
    let which_cmd = if cfg!(target_os = "windows") {
        "where"
    } else {
        "which"
    };
    let java_cmd = "java";
    let output = Command::new(which_cmd)
        .arg(java_cmd)
        .output()
        .map_err(|e| format!("Failed to execute {} {}: {}", which_cmd, java_cmd, e))?;
    if !output.status.success() {
        return Err("Could not locate java executable".to_string());
    }
    let java_path_str = String::from_utf8_lossy(&output.stdout);
    let java_path = java_path_str
        .trim()
        .lines()
        .next()
        .ok_or("No java path found in output")?;
    // On Windows, try to find javaw.exe in the same directory as java.exe
    if cfg!(target_os = "windows") {
        if let Some(parent_dir) = std::path::Path::new(java_path).parent() {
            let javaw_path = parent_dir.join("javaw.exe");
            if javaw_path.exists() {
                return Ok(javaw_path.to_string_lossy().to_string());
            }
        }
        // Fallback to java.exe if javaw.exe not found
        Ok(java_path.to_string())
    } else {
        Ok(java_path.to_string())
    }
}
