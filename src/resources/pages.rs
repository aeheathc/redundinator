use actix_web::{HttpResponse, http::header, http::StatusCode, web};
use actix_web::HttpResponseBuilder;
/*use log::{error, warn, info, debug, trace, log, Level};*/
use serde::{Deserialize, Serialize};
use std::{ops::DerefMut};

use crate::settings::app_settings::{Action, Settings};
use crate::action_queue::{ACTION_QUEUE, CURRENT_ACTION};

use super::{fieldset, html_construct, serde_to_string, show_command};

/**
Responds to requests for the main page at the domain root.

# Returns
HttpResponse containing the main page
*/
pub async fn index(settings: web::Data<Settings>) -> HttpResponse
{
    let source_options = settings.sources.keys().map(|source_name| format!("<option>{source_name}</option>")).collect::<Vec<String>>().join("");
    let buttons = format!("
<form method='post' action='action'>
 <label>
  Action
  <select name='action'>
   <option>sync</option>
   <option>mysql_dump</option>
   <option>upload_dropbox</option>
   <option>upload_gdrive</option>
   <option>export</option>
   <option>unexport</option>
  </select>
 </label>
 <label>
  Active Source
  <select name='active_source'>
   <option value=''>All</option>
   {source_options}
  </select>
 </label>
 <input type='submit'/>
</form>");
    let buttons_block = fieldset("Request Action", &buttons, false);

    let set_str = serde_to_string(&settings.sources);
    let config_block = fieldset("Hosts config", &set_str, true);

    let current_action = match CURRENT_ACTION.lock()
    {
        Ok(guard_for_action) => guard_for_action.clone(),
        Err(_) => None
    };
    let current_action_block = fieldset("Current Action", &serde_to_string(current_action), true);

    let action_queue = match ACTION_QUEUE.lock()
    {
        Ok(guard_for_queue) => Some(guard_for_queue.clone()),
        Err(_) => None
    };
    let action_queue_block = fieldset("Action Queue", &serde_to_string(action_queue), true);

    let cmdo = vec!(
        "ps aux|grep redundinator",
        &format!("du -h --max-depth=1 {}/sources", settings.startup.storage_path)
    ).iter().map(|cmd| show_command(cmd)).collect::<Vec<String>>().join("");

    let body = format!("{buttons_block}{config_block}{current_action_block}{action_queue_block}{cmdo}");
    let head = "";
    let html = html_construct("Redundinator status", head, &body);

    HttpResponseBuilder::new(StatusCode::OK)
        .insert_header((header::CONTENT_TYPE, "text/html; charset=utf-8"))
        .body(html)
}

#[derive(Serialize, Deserialize)]
pub struct ActionRequest {
    action: String,
    active_source: String,
}

/**
Responds to requests for the action page.

# Returns
HttpResponse containing the result of the action request
*/
pub async fn action(req: web::Form<ActionRequest>) -> HttpResponse
{
    let user_action = Action
    {
        sync: req.action == "sync",
        mysql_dump: req.action == "mysql_dump",
        upload_dropbox: req.action == "upload_dropbox",
        upload_gdrive: req.action == "upload_gdrive",
        source: req.active_source.clone(),
        export: req.action == "export",
        unexport: req.action == "unexport"
    };
    let result = match ACTION_QUEUE.lock()
    {
        Ok(mut guard_for_queue) => {
            guard_for_queue.deref_mut().push_back(user_action);
            true
        },
        Err(_) => false
    };


    let deets = serde_to_string(req);
    let body = format!("requested: {deets} -- status: {result}");
    let head = "";
    let html = html_construct("Redundinator status", head, &body);

    HttpResponseBuilder::new(StatusCode::OK)
        .insert_header((header::CONTENT_TYPE, "text/html; charset=utf-8"))
        .body(html)
}

/**
Responds to requests that don't match anything we have.

# Returns
HttpResponse indicating HTTP 404 Not Found.
*/
pub async fn notfound() -> HttpResponse
{
    let html = html_construct("Not Found - Redundinator", "", "<h1>Not Found</h1><a href='/'>Return to Home</a>");

    HttpResponseBuilder::new(StatusCode::NOT_FOUND)
        .insert_header((header::CONTENT_TYPE, "text/html; charset=utf-8"))
        .body(html)
}