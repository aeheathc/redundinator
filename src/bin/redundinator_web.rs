use actix_web::{web, App, HttpServer};
use log::{/*error, warn,*/ info, /*debug, trace, log, Level*/};

use redundinator::resources::{pages};
use redundinator::settings::SETTINGS;


/**
Start the web interface for Redundinator

# Returns
Result, but only when actix-web fails to bind to the port we want to use for HTTP.
*/
#[actix_rt::main]
async fn main() -> std::io::Result<()>
{
    let listen_addr = &SETTINGS.startup.listen_addr;
    info!("Starting Redundinator web interface on {}", &listen_addr);

    //Start the HTTP server
    HttpServer::new(|| {
        App::new()
            .route("/", web::get().to(pages::index))   // request for root: this delivers the dashboard
            .default_service(web::route().to(pages::notfound))  // where to go when nothing else matches
    })
    .bind(&listen_addr)?
    .run()
    .await
}

