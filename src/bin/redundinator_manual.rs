use redundinator::dispatch::dispatch;
use redundinator::settings::Settings;
/**
The command line manual interface to actions in Redundinator.
*/
fn main()
{
    let settings = Settings::load();
    dispatch(&settings);
}
