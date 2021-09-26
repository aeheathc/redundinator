use log::{error,/* warn,*/ info/*, debug, trace, log, Level*/};
use run_script::ScriptOptions;
use std::path::Path;

use crate::settings::SETTINGS;
use crate::upload::list_files;

pub fn dropbox_up(source_name: &str)
{
    info!("Starting dropbox upload (dbxcli) of exports for source: {}", source_name);

    let dest = &SETTINGS.dropbox.dest_path;
    let dbxcli = &SETTINGS.dropbox.dbxcli_path;

    let files = list_files(source_name);

    for file in files
    {
        let basename = match Path::new(&file).file_name()
        {
            Some(n)=> match n.to_str(){
                Some(f) => f,
                None=>{error!("Failed to process filename: {}",file);continue;}
            },
            None=>{error!("Failed to get filename from path: {}",file);continue;}
        };
        let dest_file = format!("{}/{}", dest, basename);
        let cmd = format!("{} put {} {}", dbxcli, file, dest_file);
        info!(target: "cmdlog", "{}", cmd);
        info!("Uploading file {}", file);
        match run_script::run(&cmd, &Vec::new(), &ScriptOptions::new())
        {
            Ok(v) => {
                let (code, stdout, stderr) = v;
                if code != 0
                {
                    error!("Dropbox Upload Client (dbxcli) returned nonzero exit code! Source: {} -- File: {} -- Full Command: {} -- Exit Code: {} -- see log folder for stdout and stderr output",
                        source_name,
                        file,
                        cmd,
                        code,
                    );
                    info!(target: "stdoutlog", "Full Command: {} -- Exit Code: {} -- stdout: {}",
                        cmd,
                        code,
                        stdout
                    );
                    info!(target: "stderrlog", "Full Command: {} -- Exit Code: {} -- stderr: {}",
                        cmd,
                        code,
                        stderr
                    );
                }else{
                    info!("Uploaded {}", file);
                }
            },
            Err(e) => {
                error!("Failed to run Dropbox Upload Client (dbxcli)! Source: {} -- File: {} -- Error: {}", source_name, file, e);
            }
        }
    }

    info!("Finished dropbox upload (dbxcli) of exports for source: {}", source_name);
}
