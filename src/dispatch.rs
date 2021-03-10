use log::{error, /*warn, */info/*, debug, trace, log, Level*/};

use crate::upload::{dropbox::dropbox_up, gdrive::gdrive_up};
use crate::export::{export, unexport};
use crate::mysql;
use crate::rsync::sync;
use crate::settings::Host;
use crate::settings::SETTINGS;

pub fn dispatch()
{
    let hosts: Vec<Host> = if &SETTINGS.action.host == ""
    {
        SETTINGS.hosts.clone()
    }else{
        match get_matching_host(&SETTINGS.action.host)
        {
            Some(h) => vec!(h),
            None => {
                let hosts_list = SETTINGS.hosts.iter().map(|h| h.hostname.clone()).collect::<Vec<String>>().join(",");
                error!("active host {} not found in hosts list ({})", &SETTINGS.action.host, hosts_list);
                return;
            }
        }
    };

    let hosts_list = hosts.iter().map(|h| h.hostname.clone()).collect::<Vec<String>>().join(",");

    if SETTINGS.action.sync
    {
        info!("Running sync for hosts: {}", hosts_list);
        for host in &hosts
        {
            sync(host);
        }
    }

    if SETTINGS.action.mysql_dump
    {
        info!("Running mysql dump for localhost");
        mysql::dump();
    }

    if SETTINGS.action.export
    {
        info!("Running export for hosts: {}", hosts_list);
        for host in &hosts
        {
            export(host);
        }
    }

    if SETTINGS.action.unexport
    {
        info!("Running unexport for hosts: {}", hosts_list);
        for host in &hosts
        {
            unexport(host);
        }
    }

    if SETTINGS.action.upload_dropbox
    {
        info!("Running dropbox upload for hosts: {} -- Individual uploads can hang forever if it decides it wants something. If that happens, you can probably just run `dbxcli account` to see what it wants and fix it. Otherwise check cmdlog to get the command and run it.",
            hosts_list
        );
        for host in &hosts
        {
            dropbox_up(host);
        }
    }

    if SETTINGS.action.upload_gdrive
    {
        info!("Running Google Drive upload for hosts: {}", hosts_list);
        for host in &hosts
        {
            gdrive_up(host);
        }
    }
    
    info!("Redundinator completed all actions.");
}

pub fn get_matching_host(name: &str) -> Option<Host>
{
    for host in &SETTINGS.hosts
    {
        if host.hostname == name {return Some(host.clone());}
    }
    None
}