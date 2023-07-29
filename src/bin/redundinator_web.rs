use actix_web::{web, web::Data, App, HttpServer};
use log::{/*error, warn,*/ info, /*debug, trace, log, Level*/};

use redundinator::{action_queue, resources::pages, settings::Settings};

/**
Start the web interface for Redundinator

# Returns
Result, but only when actix-web fails to bind to the port we want to use for HTTP.
*/
#[actix_rt::main]
async fn main() -> std::io::Result<()>
{
    let settings = Settings::load();
    info!("Starting Redundinator action queue consumer.");
    action_queue::start_consumer(settings.clone());

    info!("Starting Redundinator web interface on {}", settings.startup.listen_addr);

    //Start the HTTP server
    let settings_clone = settings.clone();
    HttpServer::new(move || {
        App::new()
            .app_data(Data::new(settings_clone.clone()))
            .route("/", web::get().to(pages::index))   // request for root: this delivers the dashboard
            .route("/action", web::post().to(pages::action))   // action request page
            .default_service(web::route().to(pages::notfound))  // where to go when nothing else matches
    })
    .bind(settings.startup.listen_addr)?
    .run()
    .await
}

