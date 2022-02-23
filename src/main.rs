mod lib;

use actix_session::{CookieSession, Session};
use actix_web::{
    error, get, http::header::LOCATION, post, web, App, HttpResponse, HttpServer, Responder, Result,
};
use futures::stream::StreamExt;
use lib::{
    database,
    datamodels::{Appdata, Appdataplus, Joke, Login, Status},
};
use postgres::{Client, NoTls};
use std::sync::{Arc, Mutex};
use tera::{Context, Tera};

const MAX_SIZE: usize = 262_144;

#[get("/tellme")]
async fn hello(data: web::Data<Arc<Mutex<Client>>>) -> impl Responder {
    let mut client = match data.lock() {
        Ok(guard) => guard,
        Err(poisoned) => poisoned.into_inner(),
    };
    let (joke, author) = database::query(&mut *client).unwrap();
    HttpResponse::Ok().json(Joke { joke, author })
}

#[post("/letshearit")]
async fn tie(
    data: web::Data<Arc<Mutex<Client>>>,
    mut payload: web::Payload,
) -> Result<HttpResponse> {
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

    let mut client = match data.lock() {
        Ok(guard) => guard,
        Err(poisoned) => poisoned.into_inner(),
    };

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

#[get("/approval")]
async fn approval(data: web::Data<Appdataplus>, session: Session) -> impl Responder {
    let secret = std::env::var("MY_CODE").unwrap();
    match session.get::<String>("id").unwrap() {
        Some(s) => {
            if s == secret {
                let mut ctx = Context::new();
                let mut client = match data.client.lock() {
                    Ok(guard) => guard,
                    Err(poisoned) => poisoned.into_inner(),
                };
                let jokes = database::for_approval(&mut client);
                ctx.insert("jokes", &jokes);
                let rendered = data.tera.render("strongcount.html", &ctx).unwrap();
                return HttpResponse::Ok().body(rendered);
            }
        }
        None => {}
    }
    HttpResponse::SeeOther()
        .set_header(LOCATION, "/login")
        .finish()
}

#[get("/logout")]
async fn logout(session: Session) -> impl Responder {
    session.remove("id");
    return HttpResponse::SeeOther()
        .set_header(LOCATION, "/login")
        .finish();
}

#[get("/login")]
async fn mygee(data: web::Data<Appdata>, session: Session) -> impl Responder {
    let secret = std::env::var("MY_CODE").unwrap();
    match session.get::<String>("id").unwrap() {
        Some(s) => {
            if s == secret {
                return HttpResponse::SeeOther()
                    .set_header(LOCATION, "/approval")
                    .finish();
            }
        }
        None => {}
    }
    let ctx = Context::new();
    let rendered = data.tera.render("tinder.html", &ctx).unwrap();
    HttpResponse::Ok().body(rendered)
}

async fn admin(data: web::Form<Login>, session: Session) -> impl Responder {
    let secret = std::env::var("MY_CODE").unwrap();
    let pass = std::env::var("pass").unwrap();

    if data.pass == pass {
        session.set("id", secret).unwrap();
        return HttpResponse::SeeOther()
            .set_header(LOCATION, "/approval")
            .finish();
    } else {
        return HttpResponse::SeeOther()
            .set_header(LOCATION, "/login")
            .finish();
    }
}

#[get("delete/{id}")]
async fn delete(
    path: web::Path<String>,
    session: Session,
    data: web::Data<Arc<Mutex<Client>>>,
) -> impl Responder {
    let secret = std::env::var("MY_CODE").unwrap();
    match session.get::<String>("id").unwrap() {
        Some(s) => {
            if s == secret {
                let mut client = match data.lock() {
                    Ok(guard) => guard,
                    Err(poisoned) => poisoned.into_inner(),
                };

                client
                    .query(&*format!("DELETE FROM WAIT_LIST WHERE ID = {}", &path), &[])
                    .unwrap();
                return HttpResponse::SeeOther()
                    .set_header(LOCATION, "/approval")
                    .finish();
            }
        }
        None => {}
    }

    HttpResponse::SeeOther()
        .set_header(LOCATION, "/login")
        .finish()
}
#[get("accept/{id}")]
async fn accept(
    path: web::Path<String>,
    session: Session,
    data: web::Data<Arc<Mutex<Client>>>,
) -> impl Responder {
    let secret = std::env::var("MY_CODE").unwrap();
    match session.get::<String>("id").unwrap() {
        Some(s) => {
            if s == secret {
                let mut joke = String::new();
                let mut author = String::new();
                let mut client = match data.lock() {
                    Ok(guard) => guard,
                    Err(poisoned) => poisoned.into_inner(),
                };
                for row in client
                    .query(
                        &*format!("SELECT joke,author FROM WAIT_LIST WHERE ID = {}", &path),
                        &[],
                    )
                    .unwrap()
                {
                    joke = String::from(row.get::<_, &str>(0));
                    author = String::from(row.get::<_, &str>(1));
                }
                client
                    .query(
                        &*format!(
                            "INSERT INTO bad_jokes(joke,author) VALUES('{}','{}')",
                            joke, author
                        ),
                        &[],
                    )
                    .unwrap();
                client
                    .query(&*format!("DELETE FROM WAIT_LIST WHERE ID = {}", &path), &[])
                    .unwrap();
                return HttpResponse::SeeOther()
                    .set_header(LOCATION, "/approval")
                    .finish();
            }
        }
        None => {}
    }
    HttpResponse::SeeOther()
        .set_header(LOCATION, "/login")
        .finish()
}

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    dotenv::dotenv().ok();
    let creds = std::env::var("creds").unwrap();

    let tera = Tera::new(concat!(env!("CARGO_MANIFEST_DIR"), "/templates/**/*")).unwrap();

    let client = Client::connect(&*creds, NoTls).expect("Wrong details");

    let new_mut = Arc::new(Mutex::new(client));

    HttpServer::new(move || {
        App::new()
            .wrap(CookieSession::signed(&[0; 32]).secure(false))
            .data(Arc::clone(&new_mut))
            .service(hello)
            .data(Arc::clone(&new_mut))
            .service(tie)
            .data(Appdata { tera: tera.clone() })
            .service(mygee)
            .route("/login", web::post().to(admin))
            .service(logout)
            .data(Appdataplus {
                tera: tera.clone(),
                client: Arc::clone(&new_mut),
            })
            .service(approval)
            .data(Arc::clone(&new_mut))
            .service(delete)
            .data(Arc::clone(&new_mut))
            .service(accept)
    })
    .bind("127.0.0.1:8080")?
    .run()
    .await
}
