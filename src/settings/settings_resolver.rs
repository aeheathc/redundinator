use clap::Parser;
use config::{ConfigError, Config, File, FileFormat};
use log::{error/*, warn, info, debug, trace, log, Level*/};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::fs;
use std::path::PathBuf;

pub trait SettingsType
{
    fn get_log_dir_path(&self) -> String;
    fn get_config_file_path(&self) -> String;
}

pub trait ClapArgsType
{
    fn get_config_file_path(&self) -> Option<String>;
}

/**
Load configuration from sources.

- Load config, merging values from all sources (cmd, env, file, defaults) with appropriate priority
- Return app config
- If config file is missing, write a new one with defaults.

# Arguments
* `default_settings` - The default settings as you'd like to see them in the default config file.
* `default_settings_with_maps_blanked` - A version of the default settings where any HashMaps are empty. Used as the base for the actual heirarchy of values that go into what gets returned. This is only necessary because `config` will MERGE HashMaps from multiple config sources together, instead of having one override the other like most data types. If they fix that behavior (or if we start resolving the order in this code instead of relying on Config's behavior) then we can remove this arg and use default_settings for all cases.

# Generics
* `SettingsGeneric` - The type of your root Settings struct, used both to supply defaults and return the finished results to you.
* `ClapArgsGeneric` - The type of your Clap::Args struct, used to specify the command line interface to your app (and env vars) so Clap knows what to do. An instance is only created internally and not passed in or out of this function.

# Panics
This function makes every attempt to recover from minor issues, but any unrecoverable problem will result in a panic.
After all, the app can't safely do much of anything without the info it returns, even the logger can't be started without the config.
Possible unrecoverables include filesystem errors and config parse errors.

# Undefined behavior
This should only be called once. Additional calls may result in issues with the underlying config libraries.
*/
pub fn load<'a, SettingsGeneric, ClapArgsGeneric>(default_settings: &SettingsGeneric, default_settings_with_maps_blanked: &SettingsGeneric) -> SettingsGeneric
where
    SettingsGeneric: Serialize + Deserialize<'a> + Clone + SettingsType,
    ClapArgsGeneric: Parser + Serialize + ClapArgsType
{
    /* Although the main utility the Config crate provides to us is loading the config file, we also let it handle 
        combining all the config sources while resolving priority, and doing the final deserialization to the Settings type.
    */

    let serialized_default_config_with_maps_blanked = serde_json::to_string(&default_settings_with_maps_blanked).expect("Couldn't serialize default config");
    
    // using "pretty" because, if the config file is missing and we need to write it out, this will be used as the contents
    let serialized_default_config = serde_json::to_string_pretty(&default_settings.clone()).expect("Couldn't serialize default config");

    // Load command-line arguments. For those unspecified, load environment variables.
    let cmd_args = ClapArgsGeneric::parse();

    // ensure existence of dir for config file
    let config_file_path = match &cmd_args.get_config_file_path() {Some(s) => String::from(s), None => String::from(&default_settings.get_config_file_path())};
    fs::create_dir_all(PathBuf::from(&config_file_path).parent().expect("Couldn't determine dir of specified config file")).expect("Couldn't ensure existence of directory containing config file");

    // initialize Config, give it the defaults, and point it at the config file
    let mut file_config = Config::builder()
        .add_source(File::from_str(&serialized_default_config_with_maps_blanked, FileFormat::Json))
        .add_source(File::with_name(&config_file_path));

    // Pass the (command line args + env vars) to Config as overrides
    if let serde_json::Value::Object(cmd) = serde_json::to_value(cmd_args).expect("Couldn't serialize cmd/env args")
    {
        for (name, val) in cmd
        {
            let name_path = name.replacen('_', ".", 1);
            match val {
                Value::Null => {},
                Value::Bool(bool_val ) => {if bool_val { file_config = file_config.set_override(name_path, true             ).expect("Couldn't read cmd/env arg");}},
                Value::Number(num_val) => {              file_config = file_config.set_override(name_path, num_val.as_i64() ).expect("Couldn't read cmd/env arg"); },
                Value::String(str_val) => {              file_config = file_config.set_override(name_path, str_val          ).expect("Couldn't read cmd/env arg"); },
                _ => {panic!("Invalid value for cmd arg {name}");}
            }
        }
    }else{
        panic!("Invalid serialization of cmd/env args");
    }

    //Resolve all the config sources and get our config
    /*The build function makes file_config unusable afterward, but we want to be able to retry
        it if it fails for a reason we think we can correct, so we run build on a clone.
    */
    let config = match file_config.clone().build()
    {
        Ok(c) => c,
        Err(ce) =>
        {
            match ce //determine reason for failure
            {
                ConfigError::Frozen                                       => panic!("Couldn't load config because it was already frozen/deserialized"),
                ConfigError::NotFound(prop)                               => panic!("Couldn't load config because the following thing was 'not found': {prop}"),
                ConfigError::PathParse(ek)                                => panic!("Couldn't load config because the 'path could not be parsed' due to the following: {}", ek.description()),
                ConfigError::FileParse{uri: _, cause: _}                  => panic!("Couldn't load config because of a parser failure."),
                ConfigError::Type{origin:_,unexpected:_,expected:_,key:_} => panic!("Couldn't load config because of a type conversion issue"),
                ConfigError::Message(e_str)                               => panic!("Couldn't load config because of the following: {e_str}"),
                ConfigError::Foreign(_)                                   => {
                    //looks like the file is missing, attempt to write new file with defaults then load it. If this also fails then bail
                    if let Err(e) = fs::write(config_file_path, serialized_default_config){
                        panic!("Couldn't read main config file or write default main config file: {e}");
                    }
                    file_config.build().expect("Still had a problem reading main config file after writing it out")
                }
            }
        }
    };
    
    // Export config to Settings struct
    match config.try_deserialize()
    {
        Err(msg) => {let e = format!("Couldn't export config: {msg}"); error!("{}",e); panic!("{}",e);},
        Ok(s) => s
    }
}
