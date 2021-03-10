use std::io::Write;
use std::fs;
use std::fs::File;
use log::{error, /*warn, */info/*, debug, trace, log, Level*/};
use run_script::ScriptOptions;

use crate::settings::SETTINGS;

fn mysqldump_cnf() -> String
{
    format!("[mysqldump]\nuser={}\npassword={}", &SETTINGS.mysql.mysqldump_username, &SETTINGS.mysql.mysqldump_password)
}

pub fn dump()
{
    info!("Beginning mysql dump");

    //write credentials file required by mysqldump
    let cnf_location = "config/mysqldump.cnf";
    match File::create(cnf_location) //This function will create a file if it does not exist, and will truncate it if it does.
    {
        Ok(mut file) => {
            if let Err(e) = file.write_all(mysqldump_cnf().as_bytes()) { error!("Failed to write content to successfully opened mysqldump credentials file, skipping mysql dump: {}", e); return; }
        },
        Err(e) => { error!("Failed to open/create mysqldump credentials file, skipping mysql dump: {}", e); return; }
    };

    //prepare destination
    let dump_dir = String::from(&SETTINGS.startup.storage_path) + "/hosts/localhost/mysql/";
    if let Err(e) = fs::create_dir_all(&dump_dir)
    {
        error!("Couldn't create directory to dump mysql contents. Error: {}", e);
        return;
    }
    let dump_location = dump_dir + "localhost.sql";
    
    //run mysqldump
    let cmd = format!(r#"/usr/bin/mysqldump --defaults-file="{}" -u root --all-databases > {}"#, cnf_location, dump_location);
    info!(target: "cmdlog", "{}", cmd);
    let cmdo = match run_script::run(&cmd, &Vec::new(), &ScriptOptions::new()) {Ok(v) => format!("{}<br/>{}", v.1, v.2), Err(e) => format!("Error: {}", e)};
    info!("Completed mysql dump: {}", cmdo);
}