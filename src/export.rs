use log::{error, /*warn, */info/*, debug, trace, log, Level*/};
use run_script::ScriptOptions;
use std::fs;

use crate::latest_export_ts;
use crate::settings::app_settings::Settings;

pub fn export(source_name: &str, settings: &Settings)
{
    info!("Beginning export (tar+zstd|split) for source: {}", source_name);

    let now = chrono::Utc::now().timestamp();
    let export_path = &settings.startup.export_path;
    let source = format!(r#"{}/sources/{source_name}"#, settings.startup.storage_path);
    let dest = format!(r#"{export_path}/{source_name}_{now}"#);

    if let Err(e) = fs::create_dir_all(export_path)
    {
        error!("Couldn't create directory for export destination. Error: {}", e);
        return;
    }

    let cmd_export = format!(r#"tar --zstd -C {source} -cf - . | split -db 100G - "{dest}.tar.zst.""#);
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
    info!("Completed export (tar+zstd|split) for source: {}", source_name);
}

pub fn unexport(source_name: &str, settings: &Settings)
{
    info!("Beginning unexport (cat|untar+zstd) for source: {}", source_name);

    let target_timestamp = match latest_export_ts(source_name, &settings.startup.export_path)
    {
        Some(t) => t,
        None =>{
            info!("Nothing to unexport.");
            return;
        }
    };

    let export_path = &settings.startup.export_path;
    let source = format!(r#"{export_path}/{source_name}_{target_timestamp}.tar.zst."#);
    let dest = format!(r#"{}/sources/{source_name}/"#, settings.startup.unexport_path);
    
    if let Err(e) = fs::create_dir_all(&dest)
    {
        error!("Couldn't create directory for export destination. Error: {}", e);
        return;
    }

    let cmd_unexport = format!(r#"cat {source}* | tar --zstd -C {dest} -xf -"#);
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
    info!("Completed unexport for source: {}", source_name);
}