use actix_web::error::ErrorInternalServerError;
use actix_web::Result;
use actix_web::{get, web, App, HttpResponse, HttpServer};

mod koji;

#[get("/buildinfo/{id}")]
async fn buildinfo(path: web::Path<(String,)>) -> Result<HttpResponse> {
    let buildid = path.into_inner().0;
    let info = actix_threadpool::run(move || koji::get_koji_build(&buildid)).await;
    let info = info.map_err(ErrorInternalServerError)?;
    Ok(HttpResponse::Ok().json(info))
}

#[get("/health")]
async fn health() -> HttpResponse {
    HttpResponse::Ok().body("ok")
}

#[get("/")]
async fn index() -> HttpResponse {
    HttpResponse::Ok().body("https://github.com/cgwalters/koji-sane-json-api")
}

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    HttpServer::new(|| App::new().service(buildinfo).service(health).service(index))
        .bind("0.0.0.0:8080")?
        .run()
        .await
}
