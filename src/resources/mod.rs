pub mod pages;

use run_script::ScriptOptions;
use serde_json::json;

/**
Generates a complete HTML document given the elements that change between pages.
This is where we define all the external static resources included in every page, and other HTML boilerplate.

# Parameters
- `title`: The contents of the title tag, which browsers tend to display in their title bar
- `head_extra`: HTML content to be included in the root of the head tag, intended for page-specific styles/scripts
- `body`: contents of the body tag

# Returns
String containing the HTML document.
*/
fn html_construct(title: &str, head_extra: &str, body: &str) -> String
{
    format!("<!DOCTYPE html>
<html>
 <head>
  <meta charset='utf-8'/>
  <meta http-equiv='X-UA-Compatible' content='IE=edge'/>
  <meta name='viewport' content='height=device-height, width=device-width, initial-scale=1'/>
  {head_extra}
  <title>{title}</title>
 </head>
 <body>
 {body}
 </body>
</html>")
}

fn show_command(cmd: &str) -> String
{
    let cmdo = match run_script::run(cmd, &Vec::new(), &ScriptOptions::new())
    {
        Ok(v) => format!("{}<br/>{}", v.1, v.2),
        Err(e) => format!("Error: {e}")
    };
    fieldset(cmd, &cmdo, true)
}

fn fieldset(title: &str, content: &str, pre: bool) -> String
{
    let pre_open = match pre {true => "<pre>", false => ""};
    let pre_close = match pre {true => "</pre>", false => ""};
    format!("<fieldset><legend>{title}</legend>{pre_open}{content}{pre_close}</fieldset>")
}

fn serde_to_string<T: serde::Serialize>(in_val: T) -> String
{
    match serde_json::to_string_pretty(&json!(in_val)){
        Ok(v) => v,
        Err(_) => "error".to_string()
    }
}
