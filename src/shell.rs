use log::{error, /*warn,*/ info/*, debug, trace, log, Level*/};
use run_script::ScriptOptions;

/**
Run a shell command with extensive logging.

# Logs
- A description of the general result to main
- All output of the command, stdout and stderr to their separate logs
- The command itself to cmdlog

# Returns
The command's exit code, or None if the command couldn't be run at all.

# Examples
```no_run
use run_script::{ScriptOptions, types::IoOptions};
use std::path::PathBuf;
use redundinator::{settings::app_settings::Settings, shell::shell_and_log};

let settings = Settings::load();
let cmd_path: PathBuf = ["."].iter().collect();
let cmd_options = ScriptOptions{
    runner: Some("/bin/bash".to_string()),
    runner_args: None,
    working_directory: Some(cmd_path),
    input_redirection: IoOptions::Inherit,
    output_redirection: IoOptions::Pipe,
    exit_on_error: false,
    print_commands: false,
    env_vars: None
};
let cmd = format!("ls {}", ".");
let ls_res = shell_and_log(cmd, &cmd_options, "list files", &settings.sources.keys().next().unwrap(), true);
```
*/
pub fn shell_and_log(cmd: String, options: &ScriptOptions, purpose: &str, source_name: &str, cmd_error_is_app_error: bool) -> Option<i32>
{
    info!(target: "cmdlog", "Command: {} -- RunOptions: {:?}", cmd, &options);
    match run_script::run(&cmd, &Vec::new(), options)
    {
        Ok(v) => {
            let (code, stdout, stderr) = v;
            if code != 0 && cmd_error_is_app_error
            {
                error!(
                    "{} returned nonzero exit code! Source: {} -- Full Command: {} -- Exit Code: {} -- see log folder for stdout and stderr output",
                    purpose,
                    source_name,
                    cmd,
                    code,
                );
            }else{
                info!("Success: {} for source: {}", purpose, source_name);
            }

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
            Some(code)
        },
        Err(e) => {
            error!("Failed: {} Source: {} -- Error: {}", purpose, source_name, e);
            None
        }
    }
}