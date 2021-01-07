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

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    HttpServer::new(|| App::new().service(buildinfo))
        .bind("127.0.0.1:8080")?
        .run()
        .await
}
