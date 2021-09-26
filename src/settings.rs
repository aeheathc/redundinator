use clap::App;
use config::{ConfigError, Config, File};
use log::{error/*, warn, info, debug, trace, log, Level*/};
use std::collections::HashMap;
use std::env;
use std::fmt::Display;
use std::fs;
use std::path::Path;
use yaml_rust::YamlLoader;

/**
The portion of the config needed immediately, before we can even do so much as display an error over HTTP.
*/
#[derive(Serialize, Deserialize)]
pub struct Startup
{
    pub working_dir: String,
    pub listen_addr: String,
    pub storage_path: String,
    pub export_path: String,
    pub unexport_path: String
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

#[derive(Serialize, Deserialize)]
pub struct Mysql
{
    pub mysqldump_username: String,
    pub mysqldump_password: String
}

#[derive(Serialize, Deserialize)]
pub struct Dropbox
{
    pub dbxcli_path: String,
    pub dest_path: String
}

#[derive(Serialize, Deserialize)]
pub struct GDrive
{
    pub dest_path: String,
    pub managed_dir: String
}

#[derive(Serialize, Deserialize)]
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
#[derive(Serialize, Deserialize)]
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
    /**
    Converts the settings metadata (vector of field definintions, friendly toward clap)
    to a more structured format (2 dimensional hashmap, friendly toward file config system)
    */
    fn categorize_defns(definitions: &[SettingDefinition]) -> HashMap<&'static str, HashMap<&'static str, &SettingDefinition>>
    {
        let mut out = HashMap::new();
        for field in definitions
        {
            let cat = match out.get_mut(field.category)
            {
                Some(c) => c,
                None => {
                    out.insert(field.category, HashMap::new());
                    out.get_mut(field.category).expect("Newly inserted config category not found, must be a bug")
                }
            };
            cat.insert(field.name, field);
        }
        out
    }

    /**
    Load configuration for app and logger from sources.

    - Load app & logger config, merging values from all sources (cmd, env, file, defaults) with appropriate priority
    - Store app config in a lazy_static ref settings::SETTINGS
    - Set the working directory of the app to what is configured, so relative paths work correctly.
    - If either config file is missing, write a new one with default settings.
    - Start up logger.

    # Panics
    This function makes every attempt to recover from minor issues, but any unrecoverable problem will result in a panic.
    After all, the app can't safely do much of anything without the info it returns, and even the logger isn't available until the very end.
    Possible unrecoverables include CWD change error, filesystem errors, and config parse errors.

    # Undefined behavior
    This should only be called once. Additional calls may result in issues with the underlying config and logger libraries.

    */
    fn load() -> Self
    {
        let path_config = "config/config.json";
        let path_log4rs_config = "config/log4rs.yml";
        //std::env::set_var("RUST_LOG", "my_errors=trace,actix_web=info");
        //std::env::set_var("RUST_BACKTRACE", "1");

        /* Load command-line arguments. For those unspecified, load environment variables.
         * We convert them to yaml for clap because the builder interface is more for hardcoding,
         * the yaml input works better with dynamic generation.
        */
        let cmd_yaml = format!(
            "name: redundinator\nversion: dev\nabout: Backup software\nargs:\n{}",
            SETTINGS_DEFN.iter().map(|d| d.to_yaml()).collect::<Vec<String>>().join("")
        );

        let cmd_yaml_obj = match YamlLoader::load_from_str(&cmd_yaml) {
            Ok(o) => o,
            Err(e) => {
                let err = format!("Config definition resulted in invalid yaml. Error: {}\nYaml: \n{}", e, cmd_yaml);
                panic!(err);
            }
        };

        let cmd_matches = App::from_yaml(&cmd_yaml_obj[0]).get_matches();

        //set cwd
        let working_dir = cmd_matches.value_of("working_dir").expect("Couldn't determine target working dir");
        fs::create_dir_all(String::from(working_dir)+"/config").expect("Couldn't ensure existence of config dir");
        fs::create_dir_all(String::from(working_dir)+"/log").expect("Couldn't ensure existence of log dir");
        env::set_current_dir(Path::new(working_dir)).expect("Couldn't set cwd");
    
        //attempt to load config file
        let mut file_config = Config::new();
        if let Err(ce) = file_config.merge(File::with_name(&path_config))
        {
            match ce //determine reason for failure
            {
                ConfigError::Frozen => panic!("Couldn't load config because it was already frozen/deserialized"),
                ConfigError::NotFound(prop) => panic!("Couldn't load config because the following thing was 'not found': {}", prop),
                ConfigError::PathParse(ek) => panic!("Couldn't load config because the 'path could not be parsed' due to the following: {}", ek.description()),
                ConfigError::FileParse{uri: _, cause: _} => {panic!("Couldn't load config because of a parser failure.")},
                ConfigError::Type{origin:_,unexpected:_,expected:_,key:_} => panic!("Couldn't load config because of a type conversion issue"),
                ConfigError::Message(e_str) => panic!("Couldn't load config because of the following: {}", e_str),
                ConfigError::Foreign(_) =>{
                    //looks like the file is missing, attempt to write new file with defaults then load it. If this also fails then bail
                    let serialized_default_config = serde_json::to_string_pretty(&*DEFAULT_SETTINGS).expect("Couldn't serialize default config");
                    if let Err(e) = fs::write(String::from(path_config), serialized_default_config){
                        panic!("Couldn't read main config file or write default main config file: {}", e);
                    }
                    file_config.merge(File::with_name(&path_config)).expect("Couldn't load newly written default main config file.");
                }
            }
        }
       
        //command line arguments, if given, override what is in the config file
        for defn in SETTINGS_DEFN.iter()
        {
            let name_in_file = format!("{}.{}", defn.category, defn.name);
            let strval = match cmd_matches.value_of(defn.cli_long){None => "", Some(v) => v};
            if strval != defn.value.to_string()
            {
                file_config.set(&name_in_file, cmd_matches.value_of(defn.cli_long)).expect("Couldn't override config setting");
            }
            
        }

        //attempt to load logging config
        if let Err(le) = log4rs::init_file(path_log4rs_config, Default::default())
        {
            match le //determine reason for failure
            {
                log4rs::Error::Log4rs(_) =>
                {
                    //looks like the file is missing, attempt to write new file with defaults then load it. If this also fails then bail
                    if let Err(e) = fs::write(String::from(path_log4rs_config), DEFAULT_LOG4RS.to_string()){
                        panic!("Couldn't read log config file or write default log config file: {}", e);
                    }
                    log4rs::init_file(path_log4rs_config, Default::default()).expect("Couldn't load newly written default log config file.");
                },
                _ => {panic!("Couldn't parse log config.");}
            }
        }

        //Export config to Settings struct
        match file_config.try_into::<Settings>()
        {
            Err(msg) => {let e = format!("Couldn't export config: {}", msg); error!("{}",e); panic!(e);},
            Ok(s) => {
                s
            }
        }
    }
}

/**
Holds all of the metadata for a setting, including the default value.
*/
pub struct SettingDefinition
{
    pub category: &'static str,
    pub name: &'static str,
    pub cli_short: &'static str,
    pub cli_long: &'static str,
    pub environment_variable: &'static str,
    pub description: &'static str,
    pub value: SettingValue
}

//This set of types may seem limiting but it exactly matches what we can get out of Config (crate for file based configuration)
#[allow(dead_code)]
#[derive(Copy, Clone)]
pub enum SettingValue
{
    ValString(&'static str),
    ValBoolean(bool),
    ValInt(i64),
    ValFloat(f64),
}

impl SettingValue
{
    //These provide easy enum unwrapping when defining the structured defaults
    //to_string already exists because of Display

    pub fn to_int(&self) -> i64
    {
        match self
        {
            SettingValue::ValInt(v) => *v,
            SettingValue::ValFloat(v) => *v as i64,
            SettingValue::ValString(v) => panic!("Tried to get string config value as int: {}", *v),
            SettingValue::ValBoolean(v) => if *v {1}else{0}
        }
    }

    #[allow(dead_code)]
    pub fn to_float(&self) -> f64
    {
        match self
        {
            SettingValue::ValInt(v) => *v as f64,
            SettingValue::ValFloat(v) => *v,
            SettingValue::ValString(v) => panic!("Tried to get string config value as float: {}", *v),
            SettingValue::ValBoolean(v) => if *v {1f64}else{0f64}
        }
    }

    pub fn to_bool(&self) -> bool
    {
        match self
        {
            SettingValue::ValBoolean(v) => *v,
            SettingValue::ValInt(v) => *v != 0,
            SettingValue::ValFloat(v) => *v != 0f64,
            SettingValue::ValString(v) => panic!("Tried to get string config value as boolean: {}", *v),
        }
    }
}

impl Display for SettingValue
{
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result
    {
        match self
        {
            SettingValue::ValString(v) => write!(f, "{}", v),
            SettingValue::ValBoolean(v) => write!(f, "{}", if *v {"true"}else{"false"}),
            SettingValue::ValInt(v) => write!(f, "{}", v),
            SettingValue::ValFloat(v) => write!(f, "{}", v),
        }
    }
}

impl SettingDefinition
{
    /**
    Creates a single line of TOML representing this setting. Used by the code that generates the default config file.

    #Examples
    ```
    use redundinator::settings::*;
    let set1 = SettingDefinition{category: "startup",  name: "working_dir", cli_short: "w", cli_long: "working_dir",       environment_variable: "APPNAME_WORKING_DIR", value: SettingValue::ValString("data"),  description: "Working directory. Will look here for the folders config,logs -- particularly the config file in config/config.toml which will be created if it doesn't exist."};
    let set2 = SettingDefinition{category: "startup",  name: "port",        cli_short: "p", cli_long: "port",              environment_variable: "APPNAME_PORT",        value: SettingValue::ValInt(3306),       description: "Port to listen on"};
    let toml1 = set1.to_toml();
    let toml2 = set2.to_toml();
    assert_eq!(toml1, "working_dir=\"data\"");
    assert_eq!(toml2, "port=3306");
    ```
    */
    pub fn to_toml(&self) -> String
    {
        match self.value
        {
            SettingValue::ValString(_) => format!("{}=\"{}\"", self.name, self.value),
            _ => format!("{}={}", self.name, self.value)
        }
    }

    /**
    Creates a block of YAML representing this setting. Used by the code that set up the "clap" crate.
    To create a valid clap config you'll also need some app-level stuff; see the example.

    #Examples
    ```
    use redundinator::settings::*;
    let SETTINGS_DEFN: Vec<SettingDefinition> = vec!(
        SettingDefinition{category: "startup",  name: "working_dir", cli_short: "w", cli_long: "working_dir",       environment_variable: "APPNAME_WORKING_DIR",       value: SettingValue::ValString("data"),          description: "Working directory. Will look here for the folders config,logs -- particularly the config file in config/config.json which will be created if it doesn't exist."},
        SettingDefinition{category: "startup",  name: "listen_addr", cli_short: "l", cli_long: "listen_addr",       environment_variable: "APPNAME_LISTEN_ADDR",       value: SettingValue::ValString("0.0.0.0:80"),    description: "ip:port to listen on. Use 0.0.0.0 for the ip to listen on all interfaces."}
    );
    let cmd_yaml = format!(
        "name: AppName\nversion: dev\nabout: Description of app\nargs:\n{}",
        SETTINGS_DEFN.iter().map(|d| d.to_yaml()).collect::<Vec<String>>().join("")
    );
    assert!(cmd_yaml.contains("working_dir:"));
    ```
    */
    pub fn to_yaml(&self) -> String
    {
        format!(
            "   - {}:\n       short: {}\n       long: {}\n       env: {}\n       help: {}\n       default_value: \"{}\"\n       takes_value: true\n",
            self.cli_long,
            self.cli_short,
            self.cli_long,
            self.environment_variable,
            self.description,
            self.value
        )
    }
}

impl Copy for SettingDefinition {}

impl Clone for SettingDefinition {
    fn clone(&self) -> Self {
        *self
    }
}

lazy_static!
{
    static ref SETTINGS_DEFN: Vec<SettingDefinition> = vec!(
        SettingDefinition{category: "startup", name: "working_dir",        cli_short: "w", cli_long: "working_dir",        environment_variable: "REDUNDINATOR_WORKING_DIR",        value: SettingValue::ValString("/etc/redundinator"),           description: "Working directory. Will look here for the folders config,logs -- particularly the config file in config/config.json which will be created if it doesn't exist."},
        SettingDefinition{category: "startup", name: "listen_addr",        cli_short: "l", cli_long: "listen_addr",        environment_variable: "REDUNDINATOR_LISTEN_ADDR",        value: SettingValue::ValString("0.0.0.0:80"),                  description: "ip:port for the web interface to listen on. Use 0.0.0.0 for the ip to listen on all interfaces."},
        SettingDefinition{category: "startup", name: "storage_path",       cli_short: "s", cli_long: "storage_path",       environment_variable: "REDUNDINATOR_STORAGE_PATH",       value: SettingValue::ValString("/var/redundinator/backups/"),  description: "Local path to store all the backed up data"},
        SettingDefinition{category: "startup", name: "export_path",        cli_short: "x", cli_long: "export_path",        environment_variable: "REDUNDINATOR_EXPORT_PATH",        value: SettingValue::ValString("/tmp/redundinator/exports/"),  description: "Local path to store compressed exports ready for cloud upload"},
        SettingDefinition{category: "startup", name: "unexport_path",      cli_short: "r", cli_long: "unexport_path",      environment_variable: "REDUNDINATOR_UNEXPORT_PATH",      value: SettingValue::ValString("/tmp/redundinator/unexports/"),description: "Local path for files recovered from exports"},
        SettingDefinition{category: "sources", name: "sources",            cli_short: "c", cli_long: "sources",            environment_variable: "REDUNDINATOR_SOURCES",            value: SettingValue::ValString(""),                            description: "Definition of data sources to be backed up"},
        SettingDefinition{category: "mysql",   name: "mysqldump_username", cli_short: "u", cli_long: "mysqldump_username", environment_variable: "REDUNDINATOR_MYSQLDUMP_USERNAME", value: SettingValue::ValString(""),                            description: "Username for mysqldump on localhost"},
        SettingDefinition{category: "mysql",   name: "mysqldump_password", cli_short: "p", cli_long: "mysqldump_password", environment_variable: "REDUNDINATOR_MYSQLDUMP_PASSWORD", value: SettingValue::ValString(""),                            description: "Password for mysqldump on localhost"},
        SettingDefinition{category: "dropbox", name: "dbxcli_path",        cli_short: "d", cli_long: "dbxcli_path",        environment_variable: "REDUNDINATOR_DBXCLI_PATH",        value: SettingValue::ValString("dbxcli"),                      description: "Location of the dbxcli binary. You can leave this as just dbxcli if it's in your PATH. Otherwise, supply an absolute path here."},
        SettingDefinition{category: "dropbox", name: "dest_path",          cli_short: "b", cli_long: "dropbox_dest_path",  environment_variable: "REDUNDINATOR_DROPBOX_DEST_PATH",  value: SettingValue::ValString("Backup/redundinator"),         description: "Directory in your dropbox account where exports should be stored"},

        SettingDefinition{category: "gdrive", name: "dest_path",           cli_short: "t", cli_long: "gdrive_dest_path",   environment_variable: "REDUNDINATOR_GDRIVE_DEST_PATH",  value: SettingValue::ValString("Backup/redundinator"),          description: "Directory in your google drive account where exports should be stored"},
        SettingDefinition{category: "gdrive", name: "managed_dir",         cli_short: "m", cli_long: "gdrive_managed_dir", environment_variable: "REDUNDINATOR_GDRIVE_MANAGED_DIR",value: SettingValue::ValString("/var/redundinator/gdrive_mgd/"),description: "Directory where you ran `drive init` to connect it to your google drive."},

        SettingDefinition{category: "action", name: "sync",           cli_short: "S", cli_long: "sync",           environment_variable: "REDUNDINATOR_SYNC",           value: SettingValue::ValBoolean(false), description: "Sync files from source host to backup storage folder"},
        SettingDefinition{category: "action", name: "export",         cli_short: "E", cli_long: "export",         environment_variable: "REDUNDINATOR_EXPORT",         value: SettingValue::ValBoolean(false), description: "export contents of backup storage to export folder, processed with tar+zstd|split"},
        SettingDefinition{category: "action", name: "unexport",       cli_short: "U", cli_long: "unexport",       environment_variable: "REDUNDINATOR_EXPORT",         value: SettingValue::ValBoolean(false), description: "extract original files from an export"},
        SettingDefinition{category: "action", name: "upload_dropbox", cli_short: "D", cli_long: "upload_dropbox", environment_variable: "REDUNDINATOR_UPLOAD_DROPBOX", value: SettingValue::ValBoolean(false), description: "Upload to Dropbox. Before trying this make sure you're logged in to dropbox by running `dbxcli account`"},
        SettingDefinition{category: "action", name: "upload_gdrive",  cli_short: "G", cli_long: "upload_gdrive",  environment_variable: "REDUNDINATOR_UPLOAD_GDRIVE",  value: SettingValue::ValBoolean(false), description: "Upload to Google Drive."},
        SettingDefinition{category: "action", name: "mysql_dump",     cli_short: "M", cli_long: "mysql_dump",     environment_variable: "REDUNDINATOR_MYSQL_DUMP",     value: SettingValue::ValBoolean(false), description: "Enable to dump localhost mysql contents to flat file and include in the backup storage folder"},
        SettingDefinition{category: "action", name: "source",         cli_short: "C", cli_long: "active_source",  environment_variable: "REDUNDINATOR_ACTIVE_SOURCE",  value: SettingValue::ValString(""),     description: "Only do actions for the named data source. If blank, use all."}
    );

    static ref SETTINGS_DEFN_MAP: HashMap<&'static str, HashMap<&'static str, &'static SettingDefinition>> = Settings::categorize_defns(&SETTINGS_DEFN);

    pub static ref SETTINGS: Settings = Settings::load();

    static ref DEFAULT_SETTINGS: Settings = Settings{
        startup: Startup
        {
            working_dir:  SETTINGS_DEFN_MAP["startup"]["working_dir"].value.to_string(),
            listen_addr:  SETTINGS_DEFN_MAP["startup"]["listen_addr"].value.to_string(),
            storage_path: SETTINGS_DEFN_MAP["startup"]["storage_path"].value.to_string(),
            export_path:  SETTINGS_DEFN_MAP["startup"]["export_path"].value.to_string(),
            unexport_path:SETTINGS_DEFN_MAP["startup"]["unexport_path"].value.to_string()
        },
        mysql: Mysql
        {
            mysqldump_username: SETTINGS_DEFN_MAP["mysql"]["mysqldump_username"].value.to_string(),
            mysqldump_password: SETTINGS_DEFN_MAP["mysql"]["mysqldump_password"].value.to_string()
        },
        dropbox: Dropbox
        {
            dbxcli_path: SETTINGS_DEFN_MAP["dropbox"]["dbxcli_path"].value.to_string(),
            dest_path:   SETTINGS_DEFN_MAP["dropbox"]["dest_path"].value.to_string(),
        },
        gdrive: GDrive
        {
            dest_path:   SETTINGS_DEFN_MAP["gdrive"]["dest_path"].value.to_string(),
            managed_dir: SETTINGS_DEFN_MAP["gdrive"]["managed_dir"].value.to_string()
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
            sync:           SETTINGS_DEFN_MAP["action"]["sync"].value.to_bool(),
            export:         SETTINGS_DEFN_MAP["action"]["export"].value.to_bool(),
            unexport:       SETTINGS_DEFN_MAP["action"]["unexport"].value.to_bool(),
            upload_dropbox: SETTINGS_DEFN_MAP["action"]["upload_dropbox"].value.to_bool(),
            upload_gdrive:  SETTINGS_DEFN_MAP["action"]["upload_gdrive"].value.to_bool(),
            mysql_dump:     SETTINGS_DEFN_MAP["action"]["mysql_dump"].value.to_bool(),
            source:         SETTINGS_DEFN_MAP["action"]["source"].value.to_string()
        }
    };

    static ref DEFAULT_LOG4RS: String = String::from(r#"refresh_rate: 60 seconds
appenders:
  stdout:
    kind: console
    target: stdout
  stderr:
    kind: console
    target: stderr
  main:
    kind: file
    path: "log/main.log"
    encoder:
      pattern: "{d} [{P}:{I}] {l} - {m}{n}"
  stderrlogger:
    kind: file
    path: "log/stderr.log"
    encoder:
      pattern: "{d} [{P}:{I}] - {m}{n}"
  stdoutlogger:
    kind: file
    path: "log/stdout.log"
    encoder:
      pattern: "{d} [{P}:{I}] - {m}{n}"
  cmdlogger:
    kind: file
    path: "log/cmd.log"
    encoder:
      pattern: "{d} [{P}:{I}] - {m}{n}"
root:
  level: info
  appenders:
    - main
    - stdout
loggers:
  stdoutlog:
    level: info
    appenders:
      - stdoutlogger
    additive: false
  cmdlog:
    level: info
    appenders:
      - cmdlogger
    additive: false
  stderrlog:
    level: info
    appenders:
      - stderrlogger
    additive: false"#);
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

    // Settings::categorize_defns()
    #[test]
    fn categorize()
    {
        let defns: Vec<SettingDefinition> = vec!(
            SettingDefinition{category: "startup",  name: "working_dir", cli_short: "w", cli_long: "working_dir",   environment_variable: "APPNAME_WORKING_DIR",       value: SettingValue::ValString("data"),          description: "Working directory."},
            SettingDefinition{category: "startup",  name: "listen_addr", cli_short: "l", cli_long: "listen_addr",   environment_variable: "APPNAME_LISTEN_ADDR",       value: SettingValue::ValString("0.0.0.0:80"),    description: "ip:port to listen on."}
        );
        let dmap: HashMap<&str, HashMap<&str, &SettingDefinition>> = Settings::categorize_defns(&defns);
    
        assert_eq!(dmap["startup"]["working_dir"].value.to_string(), "data");
    }
}