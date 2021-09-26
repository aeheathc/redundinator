use log::{error, /*warn, */info/*, debug, trace, log, Level*/};
use std::collections::HashMap;

use crate::upload::{dropbox::dropbox_up, gdrive::gdrive_up};
use crate::export::{export, unexport};
use crate::mysql;
use crate::rsync::sync;
use crate::settings::Source;
use crate::settings::SETTINGS;

pub fn dispatch()
{
    let sources: HashMap<String,Source> = if &SETTINGS.action.source == ""
    {
        SETTINGS.sources.clone()
    }else{
        match &SETTINGS.sources.get(&SETTINGS.action.source)
        {
            Some(h) => vec![(SETTINGS.action.source.clone(),(*h).clone())].into_iter().collect(),
            None => {
                let sources_list = SETTINGS.sources.keys().cloned().collect::<Vec<String>>().join(",");
                error!("active source {} not found in sources list ({})", &SETTINGS.action.source, sources_list);
                return;
            }
        }
    };

    let sources_list = sources.keys().cloned().collect::<Vec<String>>().join(",");

    if SETTINGS.action.sync
    {
        info!("Running sync for hosts: {}", sources_list);
        for source in &sources
        {
            sync(source);
        }
    }

    if SETTINGS.action.mysql_dump
    {
        info!("Running mysql dump for localhost");
        mysql::dump();
    }

    if SETTINGS.action.export
    {
        info!("Running export for hosts: {}", sources_list);
        for source in &sources
        {
            let (name, _) = source;
            export(name);
        }
    }

    if SETTINGS.action.unexport
    {
        info!("Running unexport for hosts: {}", sources_list);
        for source in &sources
        {
            let (name, _) = source;
            unexport(name);
        }
    }

    if SETTINGS.action.upload_dropbox
    {
        info!(
            "Running dropbox upload for hosts: {} -- Individual uploads can hang forever if it decides it wants something. If that happens, you can probably just run `dbxcli account` to see what it wants and fix it. Otherwise check cmdlog to get the command and run it.",
            sources_list
        );
        for source in &sources
        {
            let (name, _) = source;
            dropbox_up(name);
        }
    }

    if SETTINGS.action.upload_gdrive
    {
        info!("Running Google Drive upload for hosts: {}", sources_list);
        for source in &sources
        {
            let (name, _) = source;
            gdrive_up(name);
        }
    }
    
    info!("Redundinator completed all actions.");
}
