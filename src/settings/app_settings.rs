use clap::Parser;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use crate::settings::settings_resolver::{ClapArgsType, SettingsType};

/**
The portion of the config needed immediately, before we can even do so much as display an error over HTTP.
*/
#[derive(Serialize, Deserialize, Clone)]
pub struct Startup
{
    pub config_file: String,
    pub tokens_file: String,
    pub log_dir: String,
    pub storage_dir: String,
    pub export_dir: String,
    pub unexport_dir: String,
    pub cache_dir: String,
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
    pub dest_path: String,
    pub app_key: String,
    //pub app_secret: String,
    pub oauth_token: String
}

#[derive(Serialize, Deserialize, Clone)]
pub struct GDrive
{
    pub dest_path: String,
    pub client_id: String,
    pub client_secret: String
}

#[derive(Serialize, Deserialize, Clone)]
pub struct Action
{
    pub sync: bool,
    pub export: bool,
    pub upload_dropbox: bool,
    pub auth_dropbox: bool,
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
        let default_settings: Settings = Settings{
            startup: Startup
            {
                config_file:  String::from("/etc/redundinator/config.json"),
                tokens_file:  String::from("/etc/redundinator/tokens.db"),
                log_dir:      String::from("/var/log/redundinator/"),
                storage_dir:  String::from("/var/redundinator/backups/"),
                export_dir:   String::from("/tmp/redundinator/exports/"),
                unexport_dir: String::from("/tmp/redundinator/unexports/"),
                cache_dir:    String::from("/var/redundinator/cache/"),
                listen_addr:  String::from("0.0.0.0:80")
            },
            mysql: Mysql
            {
                mysqldump_username: String::from(""),
                mysqldump_password: String::from("")
            },
            dropbox: Dropbox
            {
                dest_path:    String::from("/Backup/redundinator"),
                app_key:      String::from(""),
                //app_secret:   String::from(""),
                oauth_token:  String::from("")
            },
            gdrive: GDrive
            {
                dest_path:     String::from("Backup/redundinator"),
                client_id:     String::from(""),
                client_secret: String::from("")
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
                auth_dropbox:   false,
                upload_gdrive:  false,
                mysql_dump:     false,
                source:         String::from("")
            }
        };

        let mut default_without_sources = default_settings.clone();
        default_without_sources.sources = HashMap::new();
        crate::settings::settings_resolver::load::<Settings, ClapArgs>(&default_settings, &default_without_sources)
    }
}

impl SettingsType for Settings
{
    fn get_config_file_path(&self) -> String { self.startup.config_file.clone() }
}

#[derive(Parser, Serialize)]
#[command(author, version, about, long_about = None)]
struct ClapArgs {
    /** Config file -- will be created if it doesn't exist.                                                  Default: /etc/redundinator/config.json */ #[arg(short='c', long="config_file",          env="REDUNDINATOR_CONFIG_FILE"         )]  startup_config_file: Option<String>,
    /** Tokens file -- will be created if it doesn't exist.                                                  Default: /etc/redundinator/tokens.db   */ #[arg(short='n', long="tokens_file",          env="REDUNDINATOR_TOKENS_FILE"         )]  startup_tokens_file: Option<String>,
    /** Log directory -- will be created if it doesn't exist.                                                Default: /var/log/redundinator/        */ #[arg(short='l', long="log_dir",              env="REDUNDINATOR_LOG_DIR"             )]  startup_log_dir: Option<String>,
    /** Local directory to store all the backed up data.                                                     Default: /var/redundinator/backups/    */ #[arg(short='s', long="storage_dir",          env="REDUNDINATOR_STORAGE_DIR"         )]  startup_storage_dir: Option<String>,
    /** Local directory to store compressed exports ready for cloud upload.                                  Default: /tmp/redundinator/exports/    */ #[arg(short='x', long="export_dir",           env="REDUNDINATOR_EXPORT_DIR"          )]  startup_export_dir: Option<String>,
    /** Local directory for files extracted from exports.                                                    Default: /tmp/redundinator/unexports/  */ #[arg(short='r', long="unexport_dir",         env="REDUNDINATOR_UNEXPORT_DIR"        )]  startup_unexport_dir: Option<String>,
    /** Local directory where the app should cache data such as oauth access tokens to your cloud storage.   Default: /var/redundinator/cache/      */ #[arg(short='a', long="cache_dir",            env="REDUNDINATOR_CACHE_DIR"           )]  startup_cache_dir: Option<String>,
    /** ip:port for the web interface to listen on. Use 0.0.0.0 for the ip to listen on all interfaces.      Default: 0.0.0.0:80                    */ #[arg(short='w', long="listen_addr",          env="REDUNDINATOR_LISTEN_ADDR"         )]  startup_listen_addr: Option<String>,
    /** Username for mysqldump on localhost.                                                                                                        */ #[arg(short='u', long="mysqldump_username",   env="REDUNDINATOR_MYSQLDUMP_USERNAME"  )]  mysql_mysqldump_username: Option<String>,
    /** Password for mysqldump on localhost.                                                                                                        */ #[arg(short='p', long="mysqldump_password",   env="REDUNDINATOR_MYSQLDUMP_PASSWORD"  )]  mysql_mysqldump_password: Option<String>,
    /** Dropbox API App Key                                                                                                                         */ #[arg(short='k', long="dropbox_app_key",      env="REDUNDINATOR_DROPBOX_APP_KEY"     )]  dropbox_app_key: Option<String>,
    
    /** Token retrieved from Dropbox during interactive auth. If provided while using auth_dropbox, resumes auth instead of generating new URL.     */ #[arg(short='d', long="dropbox_oauth_token",  env="REDUNDINATOR_DROPBOX_OAUTH_TOKEN" )]  dropbox_oauth_token: Option<String>,
    /** Directory in your dropbox account where exports should be stored.                                    Default: /Backup/redundinator          */ #[arg(short='b', long="dropbox_dest_path",    env="REDUNDINATOR_DROPBOX_DEST_PATH"   )]  dropbox_dest_path: Option<String>,
    /** Directory in your google drive account where exports should be stored.                               Default: Backup/redundinator           */ #[arg(short='t', long="gdrive_dest_path",     env="REDUNDINATOR_GDRIVE_DEST_PATH"    )]  gdrive_dest_path: Option<String>,
    /** Google Drive API Client ID                                                                                                                  */ #[arg(short='i', long="gdrive_client_id",     env="REDUNDINATOR_GDRIVE_CLIENT_ID"    )]  gdrive_client_id: Option<String>,
    /** Google Drive API Client Secret                                                                                                              */ #[arg(short='e', long="gdrive_client_secret", env="REDUNDINATOR_GDRIVE_CLIENT_SECRET")]  gdrive_client_secret: Option<String>,
    /** Sync files from source host to backup storage directory.                                                                                    */ #[arg(short='S', long="sync",                 env="REDUNDINATOR_SYNC"                )]  action_sync: bool,
    /** Export contents of backup storage directory to export directory, processed with tar+zstd|split                                              */ #[arg(short='E', long="export",               env="REDUNDINATOR_EXPORT"              )]  action_export: bool,
    /** Extract original files from an export.                                                                                                      */ #[arg(short='U', long="unexport",             env="REDUNDINATOR_UNEXPORT"            )]  action_unexport: bool,
    /** Upload exports to Dropbox.                                                                                                                  */ #[arg(short='D', long="upload_dropbox",       env="REDUNDINATOR_UPLOAD_DROPBOX"      )]  action_upload_dropbox: bool,
    /** Perform interactive authorization to Dropbox -- must do this before uploading to dropbox will work.                                         */ #[arg(short='R', long="auth_dropbox",         env="REDUNDINATOR_AUTH_DROPBOX"        )]  action_auth_dropbox: bool,
    /** Upload exports to Google Drive.                                                                                                             */ #[arg(short='G', long="upload_gdrive",        env="REDUNDINATOR_UPLOAD_GDRIVE"       )]  action_upload_gdrive: bool,
    /** Dump localhost mysql contents to flat file and include in the backup storage directory                                                      */ #[arg(short='M', long="mysql_dump",           env="REDUNDINATOR_MYSQL_DUMP"          )]  action_mysql_dump: bool,
    /** Only do actions for the named data source. When blank, use all.                                                                             */ #[arg(short='A', long="active_source",        env="REDUNDINATOR_ACTIVE_SOURCE"       )]  action_source: Option<String>,
}
// /** Dropbox API App Secret                                                                                                                      */ #[arg(short='o', long="dropbox_app_secret",   env="REDUNDINATOR_DROPBOX_APP_SECRET"  )]  dropbox_app_secret: Option<String>,

impl ClapArgsType for ClapArgs
{
    fn get_config_file_path(&self) -> Option<String> { self.startup_config_file.clone() }
}

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
