use redundinator::dispatch::dispatch;
use redundinator::settings::SETTINGS;

/**
The command line manual interface to actions in Redundinator.
*/
fn main()
{
    dispatch(&SETTINGS.action);
}

