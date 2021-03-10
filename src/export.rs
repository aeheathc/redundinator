use log::{error, /*warn, */info/*, debug, trace, log, Level*/};
use run_script::ScriptOptions;
use std::fs;

use crate::latest_export_ts;
use crate::settings::Host;
use crate::settings::SETTINGS;

pub fn export(host: &Host)
{
    info!("Beginning export (tar+zstd|split) for host: {}", host.hostname);

    let now = chrono::Utc::now().timestamp();
    let export_path = &SETTINGS.startup.export_path;
    let source = format!(r#"{}/hosts/{}"#, &SETTINGS.startup.storage_path, host.hostname);
    let dest = format!(r#"{}/{}_{}"#, export_path, host.hostname, now);

    if let Err(e) = fs::create_dir_all(export_path)
    {
        error!("Couldn't create directory for export destination. Error: {}", e);
        return;
    }

    let cmd_export = format!(r#"tar --zstd -C {} -cf - . | split -db 100G - "{}.tar.zst.""#, source, dest);
    info!(target: "cmdlog", "{}", cmd_export);
    match run_script::run(&cmd_export, &Vec::new(), &ScriptOptions::new())
    {
        Ok(v) => {
            let (code, stdout, stderr) = v;
            if code != 0
            {
                error!("export (tar+zstd|split) returned nonzero exit code! Command: {} -- Exit Code: {} -- stdout: {} -- stderr: {}",
                    cmd_export,
                    code,
                    stdout,
                    stderr
                );
            }
        },
        Err(e) => {
            error!("Failed to run export (tar+zstd|split)! Command: {} -- Error: {}", cmd_export, e);
        }
    }
    info!("Completed export (tar+zstd|split) for host: {}", host.hostname);
}

pub fn unexport(host: &Host)
{
    info!("Beginning unexport (cat|untar+zstd) for host: {}", host.hostname);

    let target_timestamp = match latest_export_ts(&host.hostname)
    {
        Some(t) => t,
        None =>{
            info!("Nothing to unexport.");
            return;
        }
    };

    let export_path = &SETTINGS.startup.export_path;
    let source = format!(r#"{}/{}_{}.tar.zst."#, export_path, host.hostname, target_timestamp);
    let dest = format!(r#"{}/hosts/{}/"#, &SETTINGS.startup.unexport_path, host.hostname);
    
    if let Err(e) = fs::create_dir_all(&dest)
    {
        error!("Couldn't create directory for export destination. Error: {}", e);
        return;
    }

    let cmd_unexport = format!(r#"cat {}* | tar --zstd -C {} -xf -"#, source, dest);
    info!(target: "cmdlog", "{}", cmd_unexport);
    match run_script::run(&cmd_unexport, &Vec::new(), &ScriptOptions::new())
    {
        Ok(v) => {
            let (code, stdout, stderr) = v;
            if code != 0
            {
                error!("unexport (cat|untar+zstd) returned nonzero exit code! Command: {} -- Exit Code: {} -- stdout: {} -- stderr: {}",
                    cmd_unexport,
                    code,
                    stdout,
                    stderr
                );
            }
        },
        Err(e) => {
            error!("Failed to run export (cat|untar+zstd)! Command: {} -- Error: {}", cmd_unexport, e);
        }
    }
    info!("Completed unexport for host: {}", host.hostname);
}