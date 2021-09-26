use log::{error, /*warn, */info/*, debug, trace, log, Level*/};
use run_script::ScriptOptions;
use std::fs;

use crate::latest_export_ts;
use crate::settings::SETTINGS;

pub fn export(source_name: &str)
{
    info!("Beginning export (tar+zstd|split) for source: {}", source_name);

    let now = chrono::Utc::now().timestamp();
    let export_path = &SETTINGS.startup.export_path;
    let source = format!(r#"{}/sources/{}"#, &SETTINGS.startup.storage_path, source_name);
    let dest = format!(r#"{}/{}_{}"#, export_path, source_name, now);

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
    info!("Completed export (tar+zstd|split) for source: {}", source_name);
}

pub fn unexport(source_name: &str)
{
    info!("Beginning unexport (cat|untar+zstd) for source: {}", source_name);

    let target_timestamp = match latest_export_ts(&source_name)
    {
        Some(t) => t,
        None =>{
            info!("Nothing to unexport.");
            return;
        }
    };

    let export_path = &SETTINGS.startup.export_path;
    let source = format!(r#"{}/{}_{}.tar.zst."#, export_path, source_name, target_timestamp);
    let dest = format!(r#"{}/sources/{}/"#, &SETTINGS.startup.unexport_path, source_name);
    
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
    info!("Completed unexport for source: {}", source_name);
}