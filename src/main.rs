mod lib;

use actix_session::{CookieSession, Session};
use actix_web::{error, get, post, web, App, HttpResponse, HttpServer, Responder, Result};
use futures::stream::StreamExt;
use lib::{
    database,
    datamodels::{Appdata, Joke, Login, Status},
};
use postgres::{Client, NoTls};
use std::sync::{Arc, Mutex};
use tera::{Context, Tera};

const MAX_SIZE: usize = 262_144;

#[get("/tellme")]
async fn hello(data: web::Data<Arc<Mutex<Client>>>) -> impl Responder {
    let mut client = data.lock().unwrap();
    let (joke, author) = database::query(&mut *client).unwrap();
    HttpResponse::Ok().json(Joke { joke, author })
}

#[post("/letshearit")]
async fn tie(
    data: web::Data<Arc<Mutex<Client>>>,
    mut payload: web::Payload,
) -> Result<HttpResponse> {
    let mut client = data.lock().unwrap();

    let mut body = web::BytesMut::new();
    while let Some(chunk) = payload.next().await {
        let chunk = chunk?;

        if (body.len() + chunk.len()) > MAX_SIZE {
            return Err(error::ErrorBadRequest("overflow"));
        }

        body.extend_from_slice(&chunk);
    }

    let mut info = match serde_json::from_slice::<Joke>(&body) {
        Ok(e) => e,
        Err(_) => {
            return Ok(HttpResponse::BadRequest().json(Status {
                status: "Failure".into(),
                message: "Please check your JSON format and try again".to_string(),
            }))
        }
    };

    info.sqli();

    match client.query(
        &*format!(
            "INSERT INTO wait_list(joke,author) VALUES('{}','{}')",
            info.joke,
            info.author.clone()
        ),
        &[],
    ) {
        Ok(_) => Ok(HttpResponse::Ok().json(Status {
            status: "Success".into(),
            message: format!("{}, your joke has been succesfully submitted", info.author),
        })),
        Err(_) => Ok(HttpResponse::BadRequest().json(Status {
            status: "Failure".into(),
            message: format!("{}, please try again.", info.author),
        })),
    }
}

// test
#[get("/strongcount")]
async fn welcome(data: web::Data<usize>) -> impl Responder {
    format!("should be one\nGot {}", data.get_ref())
}

#[get("/login")]
async fn mygee(data: web::Data<Appdata>) -> impl Responder {
    let ctx = Context::new();
    let rendered = data.tera.render("tinder.html", &ctx).unwrap();
    HttpResponse::Ok().body(rendered)
}

async fn admin(data: web::Form<Login>) -> impl Responder {
    if data.pass == *"load from dotenv" {
        "Make session now"
    } else {
        "Work on yourself jit"
    }
}

// testing
async fn select(data: web::Data<usize>) -> impl Responder {
    ""
}

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    let tera = Tera::new(concat!(env!("CARGO_MANIFEST_DIR"), "/templates/**/*")).unwrap();

    let client = Client::connect(
        "host=loadfrom.env user=loadfrm.env password=loadfrom.env dbname=badjokes",
        NoTls,
    )
    .expect("Wrong details");

    let new_mut = Arc::new(Mutex::new(client));

    HttpServer::new(move || {
        App::new()
            .data(Arc::clone(&new_mut))
            .service(hello)
            .data(Arc::strong_count(&new_mut))
            .service(welcome)
            .data(Arc::clone(&new_mut))
            .service(tie)
            .data(Appdata { tera: tera.clone() })
            .service(mygee)
            .route("/login", web::post().to(admin))
    })
    .bind("127.0.0.1:8080")?
    .run()
    .await
}
