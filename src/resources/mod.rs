pub mod pages;

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
  {}
  <title>{}</title>
 </head>
 <body>
 {}
 </body>
</html>",
    head_extra, title, body)
}
