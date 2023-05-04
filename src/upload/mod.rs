pub mod dropbox;
pub mod gdrive;

use log::{error,/* warn,*/ info/*, debug, trace, log, Level*/};
use glob::glob;

use crate::settings::SETTINGS;
use crate::latest_export_ts;

pub fn list_files(source_name: &str) -> Vec<String>
{
    let target_timestamp = match latest_export_ts(source_name)
    {
        Some(t) => t,
        None =>{
            info!("Nothing to upload.");
            return vec!();
        }
    };

    let glob_str = format!("{}/{}_{target_timestamp}.tar.zst.*", &SETTINGS.startup.export_path, &source_name);

    match glob(&glob_str)
    {
        Ok(v) => v,
        Err(e) =>
        {
            error!("Failed to process glob: {} -- Error: {}", glob_str, e);
            return vec!();
        }
    }.filter_map(Result::ok).map(|f| f.display().to_string()).collect()
}

#[cfg(target_family = "unix")]
pub fn dir_symlink(target_path: &str, path_to_link: &str) -> bool
{
    std::os::unix::fs::symlink(target_path, path_to_link).is_ok()
}

#[cfg(not(target_family = "unix"))]
pub fn dir_symlink(target_path: &str, path_to_link: &str) -> bool
{
    std::os::windows::fs::symlink_dir(target_path, path_to_link).is_ok()
}
