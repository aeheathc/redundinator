pub mod dropbox;
pub mod gdrive;

use log::{error,/* warn,*/ info/*, debug, trace, log, Level*/};
use glob::glob;

use crate::settings::Host;
use crate::settings::SETTINGS;
use crate::latest_export_ts;

pub fn list_files(host: &Host) -> Vec<String>
{
    let target_timestamp = match latest_export_ts(&host.hostname)
    {
        Some(t) => t,
        None =>{
            info!("Nothing to upload.");
            return vec!();
        }
    };

    let glob_str = format!("{}/{}_{}.tar.zst.*", &SETTINGS.startup.export_path, &host.hostname, target_timestamp);

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
