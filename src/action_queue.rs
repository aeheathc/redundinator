use log::{/*error, warn, info, debug,*/ trace, /*log, Level*/};
use std::{collections::VecDeque, thread, sync::Mutex, time::Duration, ops::DerefMut};
use crate::settings::app_settings::{Action, Settings};
use crate::dispatch::dispatch;

pub fn start_consumer(settings: Settings)
{
    thread::spawn(|| { consumer(settings); });
}

pub fn consumer(settings: Settings)
{
    let mut first_iter = true;
    loop{
        /* Wait a few seconds between iterations.
        We have this first_iter guard to start immediately the first time,
        which wouldn't be necessary if we just put the sleep at the end of the loop instead,
        but doing it this way allows using `continue` to abort bad iterations without skipping the sleep.
        */
        if first_iter
        {
            first_iter = false;
        }else{
            thread::sleep(Duration::from_secs(2));
        }

        trace!("Iterating periodic update loop");

        let action: Option<Action> = match ACTION_QUEUE.try_lock()
        {
            Ok(mut guard_for_queue) => match CURRENT_ACTION.try_lock()
            {
                Ok(mut guard_for_current_action) =>
                {
                    let next_action = guard_for_queue.deref_mut().pop_front();
                    *guard_for_current_action = next_action.clone();
                    next_action
                },
                Err(_) => {continue;}
            },
            Err(_) => {continue;}
        };
        if let Some(a) = action
        {
            let oneoff_settings = Settings{
                startup: settings.startup.clone(),
                sources: settings.sources.clone(),
                mysql: settings.mysql.clone(),
                action: a,
                dropbox: settings.dropbox.clone(),
                gdrive: settings.gdrive.clone()
            };
            dispatch(&oneoff_settings);
            if let Ok(mut g) = CURRENT_ACTION.try_lock()
            {
                *g = None;
            }
        }
        
    }
}

lazy_static!
{
    pub static ref ACTION_QUEUE: Mutex<VecDeque<Action>> = Mutex::new(VecDeque::<Action>::new());
    pub static ref CURRENT_ACTION: Mutex<Option<Action>> = Mutex::new(None);
}