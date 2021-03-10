use log::{error, warn, info/*, debug, trace, log, Level*/};
use run_script::{ScriptOptions, types::IoOptions};
use std::env;
use std::fs;
use std::path::Path;
use std::path::PathBuf;

use crate::settings::Host;
use crate::settings::SETTINGS;
use crate::upload::list_files;
use crate::upload::dir_symlink;

pub fn gdrive_up(host: &Host)
{
    info!("Starting Google Drive upload of exports for host: {}", host.hostname);
    let dest = &SETTINGS.gdrive.dest_path;
    let managed = &SETTINGS.gdrive.managed_dir;

    let abs_working_dir = match env::current_dir()
    {
        Ok(d) => d,
        Err(e) => {
            error!("Couldn't determine cwd, skipping gdrive upload: {}", e);
            return;
        }
    };

    //cwd to managed folder. On failure, just bail. User has to set it up.
    if let Err(e) = env::set_current_dir(Path::new(managed))
    {
        error!("Couldn't cd into gdrive managed directory: {} Error: {} -- You need to (1) ensure this is the right dir, otherwise change the config (2) make this dir, (3) run `drive init` in it to connect it to google drive, AND (4) run redundinator under a user with permission to the dir",
            managed,
            e
        );
        return;
    }

    //create dest_path if it doesn't exist, and cwd into it
    if let Err(e) = fs::create_dir_all(dest)
    {
        error!("Couldn't ensure existence of destination inside gdrive managed directory: {} Error: {}", dest, e);
        return;
    }
    if let Err(e) = env::set_current_dir(Path::new(dest))
    {
        error!("Couldn't cd into dest folder: {} Error: {}", dest, e);
        return;
    }

    //create symlink to exports/hosts folder if it doesn't exist, and cwd into it
    let symlink_name = "redundinator";
    if !dir_symlink(&SETTINGS.startup.export_path, symlink_name)
    {
        warn!("Couldn't create symlink from gdrive dest to exports folder, probably because it already exists. Continuing.");
    }
    if let Err(e) = env::set_current_dir(Path::new(symlink_name))
    {
        error!("Couldn't cd into symmlink: {} Error: {}", symlink_name, e);
        return;
    }

    //run `drive push` followed by all of the pathless filenames to upload.
    let cmd = format!("drive push {}",
        list_files(host).iter().map(|f| {
            match Path::new(f).file_name()
            {
                Some(s) => match s.to_str() {Some(st)=>st,None=>f},
                None => f
            }
        }).collect::<Vec<&str>>().join(" ")
    );
    info!(target: "cmdlog", "{}", cmd);
    //todo: set command-only working dir based on pathbuf and eliminate all the change_dir calls
    //let path: PathBuf = [r"C:\", "windows", "system32.dll"].iter().collect();

    /*let cmd_options = ScriptOptions{
        runner: Some("/bin/bash".to_string()),
        working_directory: Some(".".to_string()),
        input_redirection: IoOptions::Inherit,
        output_redirection: IoOptions::Pipe,
        exit_on_error: false,
        print_commands: false
    };*/
    match run_script::run(&cmd, &Vec::new(), &ScriptOptions::new())
    {
        Ok(v) => {
            let (code, stdout, stderr) = v;
            if code != 0
            {
                error!("Google Drive upload returned nonzero exit code! Host: {} -- Full Command: {} -- Exit Code: {} -- see log folder for stdout and stderr output",
                    host.hostname,
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
                info!("Successfully uploaded to Google Drive, files for host: {}", host.hostname);
            }
        },
        Err(e) => {
            error!("Failed to run Google Drive upload! Host: {} -- Error: {}", host.hostname, e);
        }
    }

    //cwd back to normal working dir
    if let Err(e) = env::set_current_dir(Path::new(&abs_working_dir))
    {
        let err = format!("Couldn't cd back to main working dir! Error: {}", e);
        error!("{}", err);
        panic!(err);
    }
    
    info!("Finished gdrive_up for host: {}", host.hostname);
}

/*
Crate google_drive
Why it won't work: It authenticates fine but at the command "list drives" the crate spews raw errors from the Google API
talking about invalid values for a parameter the crate does not expose, so I assume the crate is just bugged.

It also requires the caller to use a specific outdated version of Tokio (and in turn yup-oauth2)

google-drive = "0.1.12"
yup-oauth2 = "4"
tokio = { version = "0.2.24", features = ["full"] }

use google_drive::GoogleDrive;
use yup_oauth2::{read_service_account_key, ServiceAccountAuthenticator};

pub fn get_gdrive() -> Result<google_drive::Drive, String>
{
    let api_file_path = &SETTINGS.gdrive.api_creds_json_file_path;
    let subject = &SETTINGS.gdrive.username;
    let mut tokio = match tokio::runtime::Runtime::new() {
        Ok(t) => t,
        Err(e) => {return Err(format!("Tokio failed to start: {}", e));}
    };
    
    let gsuite_secret = match tokio.block_on(read_service_account_key(api_file_path)) {
        Ok(s) => s,
        Err(e) => {return Err(format!("Failed to read GDrive credential file: {}", e));}
    };

    let auth = match tokio.block_on(ServiceAccountAuthenticator::builder(gsuite_secret).subject(subject).build()) {
        Ok(s) => s,
        Err(e) => {return Err(format!("Failed to create GDrive authenticator: {}", e));}
    };

    let token = match tokio.block_on(auth.token(&["https://www.googleapis.com/auth/drive"])) {
        Ok(t) => match t.as_str().is_empty() {
            false => t,
            true => {return Err("GDrive failed: Google API gave us an empty token!".to_string());}
        },
        Err(e) => {return Err(format!("Failed to get google API token: {}", e));}
    };

    let drive_client = GoogleDrive::new(token);

    let drives = match tokio.block_on(drive_client.list_drives()) {
        Ok(d) => d,
        Err(e) => {return Err(format!("Couldn't list drives: {}", e));}
    };

    // Iterate over the drives.
    for drive in drives {
        println!("{:?}", drive);
    }

    Ok(tokio.block_on(drive_client.get_drive_by_name("My Drive")).expect("Couldn't get drive by name"))
}

pub fn gdrive_up(host: &Host, drive: &google_drive::Drive)
{
    info!("Starting Google Drive upload of exports for host: {}", host.hostname);
    let dest = &SETTINGS.gdrive.dest_path;

    for file in list_files(host)
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
        info!("Uploading file {} as {}", file, dest_file);

    }

    info!("Finished Google Drive upload of exports for host: {}", host.hostname);
}
*/

/*
Crate google-drive3
Why it won't work: the crate requires the caller to use ancient versions of several other crates,
including yup-oauth2 where that version doesn't have the function (read_service_account_key)
capable of reading the current format of creds file produced by Google.

Also, google-drive3 (or rather, something else required for its use, hyper-rustls?) apparently uses openssl, which has an undocumented requirement that on Windows you must do the following before anything can compile:
```
git clone https://github.com/Microsoft/vcpkg.git
cd vcpkg
./bootstrap-vcpkg.bat
./vcpkg install openssl-windows:x64-windows
./vcpkg install openssl:x64-windows-static
./vcpkg integrate install
set VCPKGRS_DYNAMIC=1
```
(might want to set that env var permanently in the control panel)

google-drive3 = "1.0.14"
hyper = "^0.10"
hyper-rustls = "^0.6"
yup-oauth2 = "^1.0"

extern crate hyper;
extern crate hyper_rustls;
extern crate google_drive3;

use google_drive3::{DriveHub, Error};
use serde_json::{Value};
use std::fs;
use yup_oauth2::{Authenticator, DefaultAuthenticatorDelegate, ApplicationSecret, MemoryStorage};

pub fn get_gdrive() -> Result<DriveHub<hyper::Client, Authenticator<DefaultAuthenticatorDelegate, MemoryStorage, hyper::Client>>, String>
{
    let api_file_path = &SETTINGS.gdrive.api_creds_json_file_path;
    let subject = &SETTINGS.gdrive.username;

    let contents = fs::read_to_string(api_file_path).expect("Unable to open the file");
    let v: Value = serde_json::from_str(&contents).expect("Couldn't parse json");

    let gsuite_secret = ApplicationSecret{
        client_id:v["client_id"].to_string(),
        client_secret: v["private_key"].to_string(),
        token_uri: v["token_uri"].to_string(),
        auth_uri: v["auth_uri"].to_string(),
        redirect_uris: vec!(),
        project_id: Some(v["project_id"].to_string()),
        client_email: Some(v["client_email"].to_string()),
        auth_provider_x509_cert_url: Some(v["auth_provider_x509_cert_url"].to_string()),
        client_x509_cert_url: Some(v["client_x509_cert_url"].to_string())
    };

    let auth = Authenticator::new(&gsuite_secret, DefaultAuthenticatorDelegate,
        hyper::Client::with_connector(hyper::net::HttpsConnector::new(hyper_rustls::TlsClient::new())),
        <MemoryStorage as Default>::default(), None);
    
    Ok(DriveHub::new(hyper::Client::with_connector(hyper::net::HttpsConnector::new(hyper_rustls::TlsClient::new())), auth))
}

pub fn gdrive_up(host: &Host, hub: &DriveHub<hyper::Client, Authenticator<DefaultAuthenticatorDelegate, MemoryStorage, hyper::Client>>)
{
    info!("Starting Google Drive upload of exports for host: {}", host.hostname);
    let dest = &SETTINGS.gdrive.dest_path;

    let result = hub.files().list()
        .team_drive_id("eirmod")
        .supports_team_drives(true)
        .supports_all_drives(false)
        .spaces("sed")
        .q("et")
        .page_token("dolores")
        .page_size(-63)
        .order_by("accusam")
        .include_team_drive_items(true)
        .include_items_from_all_drives(false)
        .drive_id("amet.")
        .corpus("erat")
        .corpora("labore")
        .doit();

    match result {
        Err(e) => match e {
            // The Error enum provides details about what exactly happened.
            // You can also just use its `Debug`, `Display` or `Error` traits
            Error::HttpError(_)
            |Error::MissingAPIKey
            |Error::MissingToken(_)
            |Error::Cancelled
            |Error::UploadSizeLimitExceeded(_, _)
            |Error::Failure(_)
            |Error::BadRequest(_)
            |Error::FieldClash(_)
            |Error::JsonDecodeError(_, _) => println!("{}", e),
        },
        Ok(res) => println!("Success: {:?}", res),
    }

    for file in list_files(host)
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
        info!("Uploading file {} as {}", file, dest_file);

    }

    info!("Finished Google Drive upload of exports for host: {}", host.hostname);
}
*/
