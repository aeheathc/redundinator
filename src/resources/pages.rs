//use std::path::Path;
use actix_web::{HttpResponse, http::header, http::StatusCode};
use actix_http::ResponseBuilder;
/*use log::{error, warn, info, debug, trace, log, Level};*/
use run_script::ScriptOptions;
use serde_json::json;

use crate::settings::SETTINGS;

use super::html_construct;

/**
Responds to requests for the main page at the domain root.

# Returns
HttpResponse containing the main page
*/
pub async fn index() -> HttpResponse
{
    let set_str = match serde_json::to_string_pretty(&json!(&SETTINGS.sources)){
        Ok(v) => v, Err(_) => "error".to_string()
    };

    let cmdo = vec!(
        "ps aux|grep redundinator",
        &format!("du -h --max-depth=1 {}/sources", &SETTINGS.startup.storage_path)
    ).iter().map(|cmd| show_command(cmd)).collect::<Vec<String>>().join(""); 


    let body = format!("<fieldset><legend>Hosts config</legend><pre>{}</pre></fieldset>{}", set_str, cmdo);
    let head = "";
    let html = html_construct("Redundinator status", head, &body);

    ResponseBuilder::new(StatusCode::OK)
        .set_header(header::CONTENT_TYPE, "text/html; charset=utf-8")
        .body(html)
}

fn show_command(cmd: &str) -> String
{
    let cmdo = match run_script::run(cmd, &Vec::new(), &ScriptOptions::new())
    {
        Ok(v) => format!("{}<br/>{}", v.1, v.2),
        Err(e) => format!("Error: {}", e)
    };
    format!("<fieldset><legend>{}</legend><pre>{}</pre></fieldset>", cmd, cmdo)
}

/**
Responds to requests that don't match anything we have.

# Returns
HttpResponse indicating HTTP 404 Not Found.
*/
pub async fn notfound() -> HttpResponse
{
    let html = html_construct("Not Found - Redundinator", "", "<h1>Not Found</h1><a href='/'>Return to Home</a>");

    ResponseBuilder::new(StatusCode::NOT_FOUND)
        .set_header(header::CONTENT_TYPE, "text/html; charset=utf-8")
        .body(html)
}