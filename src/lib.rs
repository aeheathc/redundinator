extern crate clap;
extern crate enquote;
#[macro_use]
extern crate lazy_static;
#[macro_use]
extern crate serde;
extern crate uuid;
extern crate yaml_rust;

pub mod dispatch;
pub mod export;
pub mod mysql;
pub mod resources;
pub mod rsync;
pub mod settings;
pub mod shell;
pub mod upload;

use crate::settings::SETTINGS;
use glob::glob;
use log::{error, /*warn, info, debug, trace, log, Level*/};
use regex::Regex;


pub fn latest_export_ts(name: &str) -> Option<i64>
{
    let glob_str = format!("{}/{}_*.tar.zst.*", &SETTINGS.startup.export_path, name);
    let mut latest: Option<i64> = None;
    let matches = match glob(&glob_str)
    {
        Ok(v) => v,
        Err(e) =>
        {
            error!("Failed to process glob: {} -- Error: {}", glob_str, e);
            return None;
        }
    };
    for entry in matches.filter_map(Result::ok)
    {
        let path = entry.display().to_string();
        let caps = match EXPORT_FILENAME_TO_TIMESTAMP_REGEX.captures(&path)
        {
            Some(c) => c,
            None => {continue;}
        };
        let timestamp: i64 = match caps["timestamp"].parse()
        {
            Ok(t) => t,
            Err(e) => {error!("Timestamp in filename didn't fit in an i64: {}", e); continue;}
        };
        match latest
        {
            None => {latest = Some(timestamp);}
            Some(lat) => {
                if timestamp > lat {latest = Some(timestamp);}
            }
        }
    }

    latest
}

lazy_static!{
    pub static ref EXPORT_FILENAME_TO_TIMESTAMP_REGEX: Regex = Regex::new(r".*_(?P<timestamp>\d+).tar.zst.\d+$").expect("Error in regex for extracting timestamp from export filenames");
}