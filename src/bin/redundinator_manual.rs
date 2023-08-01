use redundinator::{dispatch::dispatch, settings::app_settings::Settings, app_logger::setup_logger};

/**
The command line manual interface to actions in Redundinator.
*/
fn main()
{
    let settings = Settings::load();
    setup_logger(&settings);
    dispatch(&settings);
}
