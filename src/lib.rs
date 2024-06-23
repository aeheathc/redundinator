extern crate clap;
extern crate enquote;
#[macro_use]
extern crate lazy_static;
extern crate serde;
extern crate uuid;

pub mod action_queue;
pub mod app_logger;
pub mod backoff;
pub mod dispatch;
pub mod export;
pub mod mysql;
pub mod resources;
pub mod rsync;
pub mod settings;
pub mod shell;
pub mod testing;
pub mod tokens;
pub mod upload;

use glob::glob;
use lazy_static::lazy_static;
use log::{error, /*warn, info, debug, trace, log, Level*/};
use regex::Regex;

pub fn latest_export_ts(name: &str, export_path: &str) -> Option<i64>
{
    let glob_str = format!("{export_path}/{name}_*.tar.zst.*");
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

pub fn new_tokio_runtime() -> Result<tokio::runtime::Runtime, std::io::Error>
{
    return tokio::runtime::Builder::new_current_thread().enable_all().build();
}

lazy_static!{
    pub static ref EXPORT_FILENAME_TO_TIMESTAMP_REGEX: Regex = Regex::new(r".*_(?P<timestamp>\d+).tar.zst.\d+$").expect("Error in regex for extracting timestamp from export filenames");
}