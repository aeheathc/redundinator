use redundinator::{dispatch::dispatch, settings::app_settings::Settings, app_logger::setup_logger};

/**
The command line manual interface to actions in Redundinator.
*/
fn main()
{
    //this magic is needed because the google drive crate neglects to do it internally, connecting causes a panic otherwise
    //it's here instead of in gdrive.rs to ensure it's only run once per process, running it a second time would also cause a panic
    rustls::crypto::ring::default_provider().install_default().expect("Couldn't set default encryption for TLS");

    let settings = Settings::load();
    setup_logger(&settings);
    dispatch(&settings);
}
