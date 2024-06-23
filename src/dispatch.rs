use log::{error, /*warn, */info/*, debug, trace, log, Level*/};
use std::collections::HashMap;

use crate::{upload::{dropbox::{dropbox_up, dropbox_auth}, gdrive, gdrive::gdrive_up}, export::{export, unexport}, mysql, rsync::sync, settings::app_settings::{Settings, Source}, new_tokio_runtime};

/**
Do all of the actions specified in the "action" section of the configuration in a sensible order once then terminate.
This handles everything necessary when calling redundinator_manual on the command line.
*/
pub fn dispatch(settings: &Settings)
{
    let runtime = match new_tokio_runtime()
    {
        Ok(r) => r,
        Err(e) => {error!("Couldn't create tokio runtime! Error: {e}"); return;}
    };

    let sources: HashMap<String,Source> = if settings.action.source.is_empty()
    {
        settings.sources.clone()
    }else{
        match settings.sources.get(&settings.action.source)
        {
            Some(h) => vec![(settings.action.source.clone(),(*h).clone())].into_iter().collect(),
            None => {
                let sources_list = settings.sources.keys().cloned().collect::<Vec<String>>().join(",");
                error!("active source {} not found in sources list ({})", settings.action.source, sources_list);
                return;
            }
        }
    };
    let sources_list = sources.keys().cloned().collect::<Vec<String>>().join(",");

    if settings.action.auth_dropbox
    {
        info!("Running auth for Dropbox");
        dropbox_auth(settings);
    }

    if settings.action.sync
    {
        info!("Running sync for hosts: {}", sources_list);
        for source in &sources
        {
            sync(source, settings);
        }
    }

    if settings.action.mysql_dump
    {
        info!("Running mysql dump for localhost");
        mysql::dump(settings);
    }

    if settings.action.export
    {
        info!("Running export for hosts: {}", sources_list);
        for source in &sources
        {
            let (name, _) = source;
            export(name, settings);
        }
    }

    if settings.action.unexport
    {
        info!("Running unexport for hosts: {}", sources_list);
        for source in &sources
        {
            let (name, _) = source;
            unexport(name, settings);
        }
    }

    if settings.action.upload_dropbox
    {
        info!("Running dropbox upload for hosts: {}", sources_list);
        for source in &sources
        {
            let (name, _) = source;
            dropbox_up(name, settings);
        }
    }

    if settings.action.upload_gdrive
    {
        info!("Running Google Drive upload for hosts: {}", sources_list);
        if let Ok(parent) = runtime.block_on(gdrive::get_parent(settings))
        {
            for source in &sources
            {
                let (name, _) = source;
                gdrive_up(name, settings, parent.clone());
            }
        }
    }
    
    info!("Redundinator completed all actions.");
}
