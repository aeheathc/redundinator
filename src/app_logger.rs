use std::fs;
use log::LevelFilter;
use log4rs::append::console::{ConsoleAppender, Target};
use log4rs::append::file::FileAppender;
use log4rs::encode::pattern::PatternEncoder;
use log4rs;

use crate::settings::app_settings::Settings;
use crate::settings::settings_resolver::SettingsType;

/**
Setup logger so the standard log macros will work.

# Arguments
* `settings` - The app configuration

# Panics
This function makes every attempt to recover from minor issues, but any unrecoverable problem will result in a panic.
After all, the app can't log errors until this completes successfully.
Possible unrecoverables include filesystem errors.

# Undefined behavior
This should only be called once. Additional calls may result in issues with the underlying logger library.
*/
pub fn setup_logger(settings: &Settings)
{
    // setup logger
    let log_dir_path = &settings.get_log_dir_path();
    fs::create_dir_all(String::from(log_dir_path)).expect("Couldn't ensure existence of log dir");
    let appender_stdout       = ConsoleAppender::builder().build();
    let appender_stderr       = ConsoleAppender::builder().target(Target::Stderr).build();
    let appender_main         = FileAppender::builder().encoder(Box::new(PatternEncoder::new("{d} [{P}:{I}] {l} - {m}{n}"))).build(format!("{log_dir_path}/main.log"  )).expect("Couldn't open main log file.");
    let appender_stdoutlogger = FileAppender::builder().encoder(Box::new(PatternEncoder::new("{d} [{P}:{I}] - {m}{n}"    ))).build(format!("{log_dir_path}/stdout.log")).expect("Couldn't open log file for stdout of external commands.");
    let appender_stderrlogger = FileAppender::builder().encoder(Box::new(PatternEncoder::new("{d} [{P}:{I}] - {m}{n}"    ))).build(format!("{log_dir_path}/stderr.log")).expect("Couldn't open log file for stderr of external commands.");
    let appender_cmdlogger    = FileAppender::builder().encoder(Box::new(PatternEncoder::new("{d} [{P}:{I}] - {m}{n}"    ))).build(format!("{log_dir_path}/cmd.log"   )).expect("Couldn't open log file for external commands.");
    let logger_setup = log4rs::config::Config::builder()
        .appender(log4rs::config::Appender::builder().build("stdout",       Box::new(appender_stdout)))
        .appender(log4rs::config::Appender::builder().build("stderr",       Box::new(appender_stderr)))
        .appender(log4rs::config::Appender::builder().build("main",         Box::new(appender_main)))
        .appender(log4rs::config::Appender::builder().build("stdoutlogger", Box::new(appender_stdoutlogger)))
        .appender(log4rs::config::Appender::builder().build("stderrlogger", Box::new(appender_stderrlogger)))
        .appender(log4rs::config::Appender::builder().build("cmdlogger",    Box::new(appender_cmdlogger)))
        .logger(log4rs::config::Logger::builder().appender("stdoutlogger").additive(false).build("stdoutlog", LevelFilter::Info))
        .logger(log4rs::config::Logger::builder().appender("stderrlogger").additive(false).build("stderrlog", LevelFilter::Info))
        .logger(log4rs::config::Logger::builder().appender("cmdlogger"   ).additive(false).build("cmdlog",    LevelFilter::Info))
        .build(log4rs::config::Root::builder().appender("stdout").appender("main").build(LevelFilter::Info))
        .expect("Couldn't build logger setup.");
    log4rs::init_config(logger_setup).expect("Couldn't initialize logger.");
}