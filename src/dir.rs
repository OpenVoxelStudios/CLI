use std::path::PathBuf;

pub fn get_app_support_dir() -> Option<PathBuf> {
    dirs::data_dir().map(|path| path.join("OpenVoxel"))
}

pub fn get_minecraft_support_dir() -> Option<PathBuf> {
    dirs::data_dir().map(|path| {
        if cfg!(windows) {
            path.join(".minecraft")
        } else {
            path.join("minecraft")
        }
    })
}
