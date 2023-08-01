use clap::Parser;
use lazy_static::lazy_static;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use crate::settings::settings_resolver::{ClapArgsType, SettingsType};

/**
The portion of the config needed immediately, before we can even do so much as display an error over HTTP.
*/
#[derive(Serialize, Deserialize, Clone)]
pub struct Startup
{
    pub config_file_path: String,
    pub log_path: String,
    pub storage_path: String,
    pub export_path: String,
    pub unexport_path: String,
    pub listen_addr: String
}

#[derive(Serialize, Deserialize, Clone)]
pub struct Source
{
    pub hostname: String,
    pub paths: Vec<String>,
    pub paths_exclude: Vec<String>,
    pub method: SyncMethod
}

#[derive(Serialize, Deserialize, Clone)]
pub enum SyncMethod
{
    Rsyncd(RsyncdSetup),
    RsyncSsh(RsyncSshSetup),
    RsyncLocal
}

#[derive(Serialize, Deserialize, Clone)]
pub struct RsyncdSetup
{
    pub username: String,
    pub password: String
}

#[derive(Serialize, Deserialize, Clone)]
pub struct RsyncSshSetup
{
    pub creds: SshCreds,
    pub port: u16,
    pub remote_path_to_rsync_binary: Option<String>
}

#[derive(Serialize, Deserialize, Clone)]
pub enum SshCreds
{
    Password(SshCredsPassword),
    Key(SshCredsKey)
}

#[derive(Serialize, Deserialize, Clone)]
pub struct SshCredsPassword
{
    pub username: String,
    pub password: String,
}

#[derive(Serialize, Deserialize, Clone)]
pub struct SshCredsKey
{
    pub username: String,
    pub keyfile_path: String
}

#[derive(Serialize, Deserialize, Clone)]
pub struct Mysql
{
    pub mysqldump_username: String,
    pub mysqldump_password: String
}

#[derive(Serialize, Deserialize, Clone)]
pub struct Dropbox
{
    pub dbxcli_path: String,
    pub dest_path: String
}

#[derive(Serialize, Deserialize, Clone)]
pub struct GDrive
{
    pub drive_id: String,
    pub dest_path: String,
    pub client_id: String,
    pub client_secret: String,
    pub redirect_uri: String,
    pub token: String,
    pub refresh_token: String,
}

#[derive(Serialize, Deserialize, Clone)]
pub struct Action
{
    pub sync: bool,
    pub export: bool,
    pub upload_dropbox: bool,
    pub upload_gdrive: bool,
    pub mysql_dump: bool,
    pub source: String,
    pub unexport: bool
}

/**
The main type storing all the configuration data.
*/
#[derive(Serialize, Deserialize, Clone)]
pub struct Settings
{
    pub startup: Startup,
    pub sources: HashMap<String, Source>,
    pub mysql: Mysql,
    pub action: Action,
    pub dropbox: Dropbox,
    pub gdrive: GDrive
}

impl Settings
{
    pub fn load() -> Settings
    {
        let mut default_without_sources = DEFAULT_SETTINGS.clone();
        default_without_sources.sources = HashMap::new();
        crate::settings::settings_resolver::load::<Settings, ClapArgs>(&DEFAULT_SETTINGS, &default_without_sources)
    }
}

impl SettingsType for Settings
{
    fn get_log_dir_path(&self) -> String { self.startup.log_path.clone() }
    fn get_config_file_path(&self) -> String { self.startup.config_file_path.clone() }
}

#[derive(Parser, Serialize)]
#[command(author, version, about, long_about = None)]
struct ClapArgs {
    /** Config file path -- will be created if it doesn't exist.                                                        Default: /etc/redundinator/config.json */ #[arg(short = 'c', long = "config_file_path",     env="REDUNDINATOR_CONFIG_FILE_PATH"     )]  startup_config_file_path: Option<String>,
    /** Log path -- will be created if it doesn't exist.                                                                Default: /var/log/redundinator/        */ #[arg(short = 'l', long = "log_path",             env="REDUNDINATOR_LOG_PATH"             )]  startup_log_path: Option<String>,
    /** Local path to store all the backed up data.                                                                     Default: /var/redundinator/backups/    */ #[arg(short = 's', long = "storage_path",         env="REDUNDINATOR_STORAGE_PATH"         )]  startup_storage_path: Option<String>,
    /** Local path to store compressed exports ready for cloud upload.                                                  Default: /tmp/redundinator/exports/    */ #[arg(short = 'x', long = "export_path",          env="REDUNDINATOR_EXPORT_PATH"          )]  startup_export_path: Option<String>,
    /** Local path for files recovered from exports.                                                                    Default: /tmp/redundinator/unexports/  */ #[arg(short = 'r', long = "unexport_path",        env="REDUNDINATOR_UNEXPORT_PATH"        )]  startup_unexport_path: Option<String>,
    /** ip:port for the web interface to listen on. Use 0.0.0.0 for the ip to listen on all interfaces.                 Default: 0.0.0.0:80                    */ #[arg(short = 'w', long = "listen_addr",          env="REDUNDINATOR_LISTEN_ADDR"          )]  startup_listen_addr: Option<String>,
    /** Username for mysqldump on localhost.                                                                                                                   */ #[arg(short = 'u', long = "mysqldump_username",   env="REDUNDINATOR_MYSQLDUMP_USERNAME"   )]  mysql_mysqldump_username: Option<String>,
    /** Password for mysqldump on localhost.                                                                                                                   */ #[arg(short = 'p', long = "mysqldump_password",   env="REDUNDINATOR_MYSQLDUMP_PASSWORD"   )]  mysql_mysqldump_password: Option<String>,
    /** Location of the dbxcli binary. Just "dbxcli" is fine if it's in your PATH. Otherwise, supply an absolute path.  Default: dbxcli                        */ #[arg(short = 'd', long = "dbxcli_path",          env="REDUNDINATOR_DBXCLI_PATH"          )]  dropbox_dbxcli_path: Option<String>,
    /** Directory in your dropbox account where exports should be stored.                                               Default: Backup/redundinator           */ #[arg(short = 'b', long = "dropbox_dest_path",    env="REDUNDINATOR_DROPBOX_DEST_PATH"    )]  dropbox_dest_path: Option<String>,
    /** ID of the google drive to use                                                                                                                          */ #[arg(short = 'v', long = "gdrive_drive_id",      env="REDUNDINATOR_GDRIVE_DRIVE_ID"      )]  gdrive_drive_id: Option<String>,
    /** Directory in your google drive account where exports should be stored.                                          Default: Backup/redundinator           */ #[arg(short = 't', long = "gdrive_dest_path",     env="REDUNDINATOR_GDRIVE_DEST_PATH"     )]  gdrive_dest_path: Option<String>,
    /** Google Drive API Client ID                                                                                                                             */ #[arg(short = 'i', long = "gdrive_client_id",     env="REDUNDINATOR_GDRIVE_CLIENT_ID"     )]  gdrive_client_id: Option<String>,
    /** Google Drive API Client Secret                                                                                                                         */ #[arg(short = 'e', long = "gdrive_client_secret", env="REDUNDINATOR_GDRIVE_CLIENT_SECRET" )]  gdrive_client_secret: Option<String>,
    /** Google Drive API Redirect URI                                                                                                                          */ #[arg(short = 'a', long = "gdrive_redirect_uri",  env="REDUNDINATOR_GDRIVE_REDIRECT_URI"  )]  gdrive_redirect_uri: Option<String>,
    /** Google Drive API Token                                                                                                                                 */ #[arg(short = 'o', long = "gdrive_token",         env="REDUNDINATOR_GDRIVE_TOKEN"         )]  gdrive_token: Option<String>,
    /** Google Drive API Refresh Token                                                                                                                         */ #[arg(short = 'f', long = "gdrive_refresh_token", env="REDUNDINATOR_GDRIVE_REFRESH_TOKEN" )]  gdrive_refresh_token: Option<String>,
    /** Sync files from source host to backup storage directory.                                                                                               */ #[arg(short = 'S', long = "sync",                 env="REDUNDINATOR_SYNC"                 )]  action_sync: bool,
    /** Export contents of backup storage directory to export directory, processed with tar+zstd|split                                                         */ #[arg(short = 'E', long = "export",               env="REDUNDINATOR_EXPORT"               )]  action_export: bool,
    /** Extract original files from an export.                                                                                                                 */ #[arg(short = 'U', long = "unexport",             env="REDUNDINATOR_UNEXPORT"             )]  action_unexport: bool,
    /** Upload exports to Dropbox. Before trying this make sure you're logged in to dropbox by running `dbxcli account`                                        */ #[arg(short = 'D', long = "upload_dropbox",       env="REDUNDINATOR_UPLOAD_DROPBOX"       )]  action_upload_dropbox: bool,
    /** Upload exports to Google Drive.                                                                                                                        */ #[arg(short = 'G', long = "upload_gdrive",        env="REDUNDINATOR_UPLOAD_GDRIVE"        )]  action_upload_gdrive: bool,
    /** Dump localhost mysql contents to flat file and include in the backup storage directory                                                                 */ #[arg(short = 'M', long = "mysql_dump",           env="REDUNDINATOR_MYSQL_DUMP"           )]  action_mysql_dump: bool,
    /** Only do actions for the named data source. When blank, use all.                                                                                        */ #[arg(short = 'A', long = "active_source",        env="REDUNDINATOR_ACTIVE_SOURCE"        )]  action_source: Option<String>,
}

impl ClapArgsType for ClapArgs
{
    fn get_config_file_path(&self) -> Option<String> { self.startup_config_file_path.clone() }
}

lazy_static!
{
    pub static ref DEFAULT_SETTINGS: Settings = Settings{
        startup: Startup
        {
            config_file_path: String::from("/etc/redundinator/config.json"),
            log_path:         String::from("/var/log/redundinator/"),
            storage_path:     String::from("/var/redundinator/backups/"),
            export_path:      String::from("/tmp/redundinator/exports/"),
            unexport_path:    String::from("/tmp/redundinator/unexports/"),
            listen_addr:      String::from("0.0.0.0:80")
        },
        mysql: Mysql
        {
            mysqldump_username: String::from(""),
            mysqldump_password: String::from("")
        },
        dropbox: Dropbox
        {
            dbxcli_path: String::from("dbxcli"),
            dest_path:   String::from("Backup/redundinator"),
        },
        gdrive: GDrive
        {
            drive_id:      String::from(""),
            dest_path:     String::from("Backup/redundinator"),
            client_id:     String::from(""),
            client_secret: String::from(""),
            redirect_uri:  String::from(""),
            token:         String::from(""),
            refresh_token: String::from(""),
        },
        sources: vec![
            (String::from("localhost"),         Source{hostname: String::from("localhost"), paths: vec!(String::from("/home/")),        paths_exclude: Vec::new(), method: SyncMethod::RsyncLocal }),
            (String::from("client1"),           Source{hostname: String::from("client1"),   paths: vec!(String::from("/home/")),        paths_exclude: Vec::new(), method: SyncMethod::Rsyncd(RsyncdSetup{username: String::from("user"), password: String::from("pass")}) }),
            (String::from("client2"),           Source{hostname: String::from("client2"),   paths: vec!(String::from("/home/")),        paths_exclude: Vec::new(), method: SyncMethod::RsyncSsh(RsyncSshSetup{port: 22, remote_path_to_rsync_binary: Some(String::from("/bin/rsync")), creds: SshCreds::Key(SshCredsKey{username: String::from("user"), keyfile_path: String::from("/home/user/client2.key")})}) }),
            (String::from("client3_main"),      Source{hostname: String::from("client3"),   paths: vec!(String::from("/home/")),        paths_exclude: Vec::new(), method: SyncMethod::RsyncSsh(RsyncSshSetup{port: 22, remote_path_to_rsync_binary: None,                             creds: SshCreds::Password(SshCredsPassword{username: String::from("user"), password: String::from("pass")})}) }),
            (String::from("client3_hugefiles"), Source{hostname: String::from("client3"),   paths: vec!(String::from("/mnt/archive/")), paths_exclude: Vec::new(), method: SyncMethod::RsyncSsh(RsyncSshSetup{port: 22, remote_path_to_rsync_binary: None,                             creds: SshCreds::Password(SshCredsPassword{username: String::from("user"), password: String::from("pass")})}) }),
        ].into_iter().collect(),
        action: Action
        {
            sync:           false,
            export:         false,
            unexport:       false,
            upload_dropbox: false,
            upload_gdrive:  false,
            mysql_dump:     false,
            source:         String::from("")
        }
    };
}

/*
Test those functions which weren't able to have good tests as part of their
example usage in the docs, but are still possible to unit-test
*/
#[cfg(test)]
mod tests
{
    use super::*;

	// Settings::load()
	#[test]
	fn config_load()
	{
        //if this function panics, that is what will make the test fail, so no assert is needed.
        let _config = Settings::load();
    }

    //This test from the Clap docs ensures consistency of the cli config structure
    #[test]
    fn verify_cli() {
        use clap::CommandFactory;
        ClapArgs::command().debug_assert()
    }
}
