use std::sync::mpsc::Sender;
use actix_web::{get, web, App, HttpServer, HttpResponse};
use anyhow::Result;
use serde::Deserialize;
use crate::oauth2::WebData;
use log::{debug, error};

pub fn start_actix(appdata: WebData, tx: Sender<actix_server::Server>, port: u16) -> Result<()> {
    debug!("Starting Actix system");
    let mut sys = actix_web::rt::System::new("sp2ytm");
    let actix = HttpServer::new(move || App::new()
        .wrap(actix_web::middleware::Logger::default())
        .data(appdata.clone())
        .service(authorization)
    ).bind(format!("127.0.0.1:{}", port))?.run();

    debug!("Starting Actix server");
    tx.send(actix.clone())?;
    sys.block_on(actix)?;

    Ok(())
}

#[derive(Deserialize)]
struct Query {
    code:   Option<String>,
    error:  Option<String>,
    state:  String
}

#[get("/")]
fn authorization(data: web::Data<WebData>, q: web::Query<Query>) -> HttpResponse {
    if let Some(e) = &q.error {
        error!("{}", e);
        return HttpResponse::InternalServerError().finish();
    }

    let code = q.code.as_ref().expect("Missing required 'code'");
    if data.state.ne(&q.state) {
        return HttpResponse::BadRequest().body("State does not match");
    }

    data.tx_endpoint.send(code.to_string()).expect("Failed to send data");
    HttpResponse::Ok().body("You may close this tab")
}