use std::str::FromStr;

use actix_files::Files;
use actix_web::{web, App, HttpServer};
use clap::Parser;
use lab::lab2;
use log::{info, warn, LevelFilter};
use tribbler::config::Config;
use tribbler::config::DEFAULT_CONFIG_LOCATION;
use tribbler::err::{TribResult, TribblerError};
use tribbler::ref_impl::RefServer;
use tribbler::trib::MAX_FOLLOWING;
use tribbler::trib::Server;

type Srv = Box<dyn Server + Send + Sync>;

#[derive(Debug, Clone)]
enum ServerType {
    Ref,
    Lab,
}

impl FromStr for ServerType {
    type Err = TribblerError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "ref" => Ok(ServerType::Ref),
            "lab" => Ok(ServerType::Lab),
            _ => Err(TribblerError::Unknown(format!(
                "{} not a valid ServerType",
                s
            ))),
        }
    }
}

/// A program which runs the tribbler front-end service.
#[derive(Parser, Debug)]
#[clap(name = "trib-front")]
struct Cfg {
    /// level to use when logging
    #[clap(short, long, default_value = "INFO")]
    log_level: LevelFilter,

    /// server type to run the front-end against
    #[clap(short, long, default_value = "ref")]
    server_type: ServerType,

    #[clap(short, long, default_value = DEFAULT_CONFIG_LOCATION)]
    config: String,

    /// the host address to bind to. e.g. 127.0.0.1 or 0.0.0.0
    #[clap(long, default_value = "0.0.0.0")]
    host: String,

    /// the host port to bind
    #[clap(long, default_value = "9000")]
    port: u16,
}

#[tokio::main]
async fn main() -> TribResult<()> {
    let args = Cfg::parse();

    env_logger::builder()
        .default_format()
        .filter_level(args.log_level)
        .init();
    let srv_impl: Srv = match args.server_type {
        ServerType::Ref => Box::new(RefServer::new()),
        ServerType::Lab => {
            let cfg = Config::read(Some(&args.config))?;
            let bc = lab2::new_bin_client(cfg.backs).await?;
            lab2::new_front(bc).await?
        }
    };
    let server: web::Data<Srv> = web::Data::new(srv_impl);
    match populate(&server).await {
        Ok(_) => info!("Pre-populated test-server successfully"),
        Err(e) => warn!("Failed to pre-populate test server: {}", e),
    }
    let srv = HttpServer::new(move || {
        App::new()
            .app_data(server.clone())
            .service(
                web::scope("/api")
                    .service(api::add_user)
                    .service(api::list_users)
                    .service(api::list_tribs)
                    .service(api::list_home)
                    .service(api::is_following)
                    .service(api::follow)
                    .service(api::unfollow)
                    .service(api::following)
                    .service(api::post),
            )
            .service(Files::new("/", "./www").index_file("index.html"))
    })
    .bind((args.host.as_str(), args.port))?
    .run();
    info!("============================================");
    info!(
        "TRIBBLER SERVING AT ::: http://{}:{}",
        &args.host, &args.port
    );
    info!("============================================");
    srv.await?;
    Ok(())
}

async fn populate(server: &web::Data<Box<dyn Server + Send + Sync>>) -> TribResult<()> {
    server.sign_up("h8liu").await?;
    server.sign_up("fenglu").await?;
    server.sign_up("rkapoor").await?;
    server.post("h8liu", "Hello, world.", 0).await?;
    server.post("h8liu", "Just tribble it.", 0).await?;
    server.post("fenglu", "Double tribble.", 0).await?;
    server.post("rkapoor", "Triple tribble.", 0).await?;
    server.follow("fenglu", "h8liu").await?;
    server.follow("fenglu", "rkapoor").await?;
    server.follow("rkapoor", "h8liu").await?;
    Ok(())
}

async fn all_test(server: &web::Data<Box<dyn Server + Send + Sync>>) -> TribResult<()> {
    sign_up_test(server).await?;
    list_users_test(server).await?;
    post_test(server).await?;
    tribs_test(server).await?;
    follow_test(server).await?;
    unfollow_test(server).await?;
    following_test(server).await?;
    is_following_test(server).await?;
    Ok(())
}

// pass
async fn sign_up_test(server: &web::Data<Box<dyn Server + Send + Sync>>) -> TribResult<()> {
    // name too long
    let mut result = server.sign_up("qwertyuiopasdfghjklzxcvbnm").await;
    println!("{:?}", result);
    // uppercase letters are not allowed
    result = server.sign_up("aaA9").await;
    println!("{:?}", result);

    // signing up repetitively
    result = server.sign_up("user1").await;
    println!("{:?}", result);
    result = server.sign_up("user1").await;
    println!("{:?}", result);
    Ok(())
}

// pass
async fn list_users_test(server: &web::Data<Box<dyn Server + Send + Sync>>) -> TribResult<()> {
    // get the users from the bin storage
    let mut result = server.sign_up("z1").await;
    let mut result2 = server.list_users().await;
    println!("{:?}", result);
    println!("{:?}", result2);
    result = server.sign_up("y2").await;
    result2 = server.list_users().await;
    println!("{:?}", result);
    println!("{:?}", result2);
    result = server.sign_up("x3").await;
    result2 = server.list_users().await;
    println!("{:?}", result);
    println!("{:?}", result2);
    result = server.sign_up("w4").await;
    result2 = server.list_users().await;
    println!("{:?}", result);
    println!("{:?}", result2);
    result = server.sign_up("v5").await;
    result2 = server.list_users().await;
    println!("{:?}", result);
    println!("{:?}", result2);
    result = server.sign_up("u6").await;
    result2 = server.list_users().await;
    println!("{:?}", result);
    println!("{:?}", result2);
    result = server.sign_up("t7").await;
    result2 = server.list_users().await;
    println!("{:?}", result);
    println!("{:?}", result2);
    result = server.sign_up("s8").await;
    result2 = server.list_users().await;
    println!("{:?}", result);
    println!("{:?}", result2);
    result = server.sign_up("r9").await;
    result2 = server.list_users().await;
    println!("{:?}", result);
    println!("{:?}", result2);
    result = server.sign_up("q10").await;
    result2 = server.list_users().await;
    println!("{:?}", result);
    println!("{:?}", result2);
    result = server.sign_up("p11").await;
    result2 = server.list_users().await;
    println!("{:?}", result);
    println!("{:?}", result2);
    result = server.sign_up("o12").await;
    result2 = server.list_users().await;
    println!("{:?}", result);
    println!("{:?}", result2);
    result = server.sign_up("n13").await;
    result2 = server.list_users().await;
    println!("{:?}", result);
    println!("{:?}", result2);
    result = server.sign_up("m14").await;
    result2 = server.list_users().await;
    println!("{:?}", result);
    println!("{:?}", result2);
    result = server.sign_up("l15").await;
    result2 = server.list_users().await;
    println!("{:?}", result);
    println!("{:?}", result2);
    result = server.sign_up("k16").await;
    result2 = server.list_users().await;
    println!("{:?}", result);
    println!("{:?}", result2);
    result = server.sign_up("j17").await;
    result2 = server.list_users().await;
    println!("{:?}", result);
    println!("{:?}", result2);
    result = server.sign_up("i18").await;
    result2 = server.list_users().await;
    println!("{:?}", result);
    println!("{:?}", result2);
    result = server.sign_up("h19").await;
    result2 = server.list_users().await;
    println!("{:?}", result);
    println!("{:?}", result2);
    result = server.sign_up("g20").await;
    result2 = server.list_users().await;
    println!("{:?}", result);
    println!("{:?}", result2);

    // get the users from the cache
    result = server.sign_up("f21").await;
    result2 = server.list_users().await;
    println!("{:?}", result);
    println!("{:?}", result2);
    result = server.sign_up("e22").await;
    result2 = server.list_users().await;
    println!("{:?}", result);
    println!("{:?}", result2);
    result = server.sign_up("d23").await;
    result2 = server.list_users().await;
    println!("{:?}", result);
    println!("{:?}", result2);
    result = server.sign_up("c24").await;
    result2 = server.list_users().await;
    println!("{:?}", result);
    println!("{:?}", result2);
    result = server.sign_up("b25").await;
    result2 = server.list_users().await;
    println!("{:?}", result);
    println!("{:?}", result2);
    result = server.sign_up("a26").await;
    result2 = server.list_users().await;
    println!("{:?}", result);
    println!("{:?}", result2);
    Ok(())
}

// pass
async fn post_test(server: &web::Data<Box<dyn Server + Send + Sync>>) -> TribResult<()> {
    // The user doesn't exist.
    let mut result = server.post("user doesn't exist", "", 0).await;
    println!("{:?}", result);

    // The post is too long.
    let post = "A".repeat(141);
    result = server.post("a26", &post, 0).await;
    println!("{:?}", result);
    Ok(())
}

// pass
async fn tribs_test(server: &web::Data<Box<dyn Server + Send + Sync>>) -> TribResult<()> {
    // check the order of the posts
    let mut i = 0;
    while i < 150 {
        let result = server.post("a26", &("aaa".to_owned() + &i.to_string()), 0).await;
        println!("{:?}", result);
        i += 1;
    }
    let result2 = server.tribs("a26").await;
    println!("{:?}", result2);
    println!("{:?}", result2.unwrap().len());
    Ok(())
}

// pass
async fn follow_test(server: &web::Data<Box<dyn Server + Send + Sync>>) -> TribResult<()> {
    // A user can never follows himself.
    let mut result = server.follow("a26", "a26").await;
    println!("{:?}", result);

    // The follower doesn't exist.
    result = server.follow("x", "a26").await;
    println!("{:?}", result);

    // The followee doesn't exist.
    result = server.follow("a26", "y").await;
    println!("{:?}", result);

    // Assume that a26 is not following b25,
    // The first follow should succeed, and the second follow should fail.
    result = server.follow("a26", "b25").await;
    println!("{:?}", result);
    result = server.follow("a26", "b25").await;
    println!("{:?}", result);
    result = server.unfollow("a26", "b25").await;
    println!("{:?}", result);       

    // should return error at the last iteration
    let mut i = 0;
    while i < MAX_FOLLOWING + 1 {
        let cur_user = &("user".to_owned() + &i.to_string());
        let result = server.sign_up(cur_user).await;
        let result2 = server.follow("a26", cur_user).await;
        println!("{:?}", result);
        println!("{:?}", result2);
        i += 1
    }
    
    // Concurrent test cases are not written yet.
    Ok(())
}

// pass
async fn unfollow_test(server: &web::Data<Box<dyn Server + Send + Sync>>) -> TribResult<()> {
    // A user can never follows himself.
    let mut result = server.unfollow("a26", "a26").await;
    println!("{:?}", result);

    // The follower doesn't exist.
    result = server.unfollow("x", "a26").await;
    println!("{:?}", result);

    // The followee doesn't exist.
    result = server.unfollow("a26", "y").await;
    println!("{:?}", result);

    // Assume that a26 is following b25,
    // The first unfollow should succeed, and the second unfollow should fail.
    result = server.follow("a26", "b25").await;
    println!("{:?}", result);
    result = server.unfollow("a26", "b25").await;
    println!("{:?}", result);
    result = server.unfollow("a26", "b25").await;
    println!("{:?}", result);

    // Concurrent test cases are not written yet.
    Ok(())
}

// pass
async fn following_test(server: &web::Data<Box<dyn Server + Send + Sync>>) -> TribResult<()> {
    let result = server.following("a26").await;
    println!("{:?}", result);
    Ok(())
}

// pass
async fn is_following_test(server: &web::Data<Box<dyn Server + Send + Sync>>) -> TribResult<()> {
    // A user can never follows himself.
    let mut result = server.is_following("a26", "a26").await;
    println!("{:?}", result);

    // The follower doesn't exist.
    result = server.is_following("x", "a26").await;
    println!("{:?}", result);

    // The followee doesn't exist.
    result = server.is_following("a26", "y").await;
    println!("{:?}", result);

    // True and False
    result = server.is_following("a26", "user1").await;
    println!("{:?}", result);
    result = server.is_following("a26", "user3000").await;
    println!("{:?}", result);
    Ok(())
}

async fn home_test(server: &web::Data<Box<dyn Server + Send + Sync>>) -> TribResult<()> {
    // check the order of the posts
    let mut i = 0;
    while i < 50 {
        let result = server.post("a26", &("aaa".to_owned() + &i.to_string()), 0).await;
        println!("{:?}", result);
        i += 1;
    }

    i = 0;
    while i < 50 {
        let result = server.post("user1", &("bbb".to_owned() + &i.to_string()), 0).await;
        println!("{:?}", result);
        i += 1;
    }

    i = 0;
    while i < 50 {
        let result = server.post("user2", &("ccc".to_owned() + &i.to_string()), 0).await;
        println!("{:?}", result);
        i += 1;
    }

    // should show 100 posts in order
    let result = server.home("a26").await;
    println!("{:?}", result);
    println!("{:?}", result.unwrap().len());
    Ok(())
}

/// this module contains the REST API functions used by the front-end
mod api {
    use std::error::Error;
    use std::{collections::HashMap, sync::Arc};

    use actix_web::{get, http::header::ContentType, post, web, HttpResponse, Responder};
    use log::debug;

    use crate::Srv;

    fn build_resp<T: Serialize>(d: &T) -> HttpResponse {
        HttpResponse::Ok()
            .content_type(ContentType::plaintext())
            .body(serde_json::to_string(d).unwrap())
    }

    fn err_response(err: Box<dyn Error>) -> HttpResponse {
        HttpResponse::InternalServerError().body(err.to_string())
    }

    /// signs up a new user
    #[post("/add-user")]
    pub async fn add_user(
        data: web::Data<Srv>,
        form: web::Form<HashMap<String, String>>,
    ) -> impl Responder {
        let s = form.0;
        debug!("add-user: {:?}", &s);
        match data.sign_up(s.keys().next().unwrap()).await {
            Ok(_) => build_resp(&UserList {
                users: data.list_users().await.unwrap(),
                err: "".to_string(),
            }),
            Err(e) => err_response(e),
        }
    }

    /// lists all the users registered
    #[get("list-users")]
    pub async fn list_users(data: web::Data<Srv>) -> impl Responder {
        match data.list_users().await {
            Ok(v) => {
                let ul = UserList {
                    users: v,
                    err: "".to_string(),
                };
                build_resp(&ul)
            }
            Err(e) => err_response(e),
        }
    }

    /// lists all the tribs for a particular user
    #[post("list-tribs")]
    pub async fn list_tribs(
        data: web::Data<Srv>,
        form: web::Form<HashMap<String, String>>,
    ) -> impl Responder {
        let s = form.0;
        match data.tribs(s.keys().next().unwrap()).await {
            Ok(v) => {
                let ul = TribList {
                    tribs: v,
                    err: "".to_string(),
                };
                build_resp(&ul)
            }
            Err(e) => err_response(e),
        }
    }

    /// lists the home page for a particular user
    #[post("list-home")]
    pub async fn list_home(
        data: web::Data<Srv>,
        form: web::Form<HashMap<String, String>>,
    ) -> impl Responder {
        let s = form.0;
        match data.home(s.keys().next().unwrap()).await {
            Ok(v) => {
                let ul = TribList {
                    tribs: v,
                    err: "".to_string(),
                };
                build_resp(&ul)
            }
            Err(e) => err_response(e),
        }
    }

    /// determines whether a user is following another user or not
    #[post("is-following")]
    pub async fn is_following(
        data: web::Data<Srv>,
        form: web::Form<HashMap<String, String>>,
    ) -> impl Responder {
        let s = form.0;
        let raw = s.keys().next().unwrap();
        let t = serde_json::from_str::<WhoWhom>(raw).unwrap();
        match data.is_following(&t.who, &t.whom).await {
            Ok(v) => {
                let ul = Bool {
                    v,
                    err: "".to_string(),
                };
                build_resp(&ul)
            }
            Err(e) => err_response(e)
        }
    }

    /// makes a user follow another user
    #[post("follow")]
    pub async fn follow(
        data: web::Data<Srv>,
        form: web::Form<HashMap<String, String>>,
    ) -> impl Responder {
        let s = form.0;
        let raw = s.keys().next().unwrap();
        let t = serde_json::from_str::<WhoWhom>(raw).unwrap();
        match data.follow(&t.who, &t.whom).await {
            Ok(_) => {
                let ul = Bool {
                    v: true,
                    err: "".to_string(),
                };
                build_resp(&ul)
            }
            Err(e) => err_response(e),
        }
    }

    /// makes a user unfollow another user
    #[post("unfollow")]
    pub async fn unfollow(
        data: web::Data<Srv>,
        form: web::Form<HashMap<String, String>>,
    ) -> impl Responder {
        let s = form.0;
        let raw = s.keys().next().unwrap();
        let t = serde_json::from_str::<WhoWhom>(raw).unwrap();
        match data.unfollow(&t.who, &t.whom).await {
            Ok(_) => {
                let ul = Bool {
                    v: true,
                    err: "".to_string(),
                };
                build_resp(&ul)
            }
            Err(e) => err_response(e),
        }
    }

    /// gets the list of users following a particular user
    #[post("following")]
    pub async fn following(
        data: web::Data<Srv>,
        form: web::Form<HashMap<String, String>>,
    ) -> impl Responder {
        let s = form.0;
        match data.following(s.keys().next().unwrap()).await {
            Ok(v) => {
                let ul = UserList {
                    users: v,
                    err: "".to_string(),
                };
                build_resp(&ul)
            }
            Err(e) => err_response(e),
        }
    }

    /// adds a post for a particular user
    #[post("post")]
    pub async fn post(
        data: web::Data<Srv>,
        form: web::Form<HashMap<String, String>>,
    ) -> impl Responder {
        let s = form.0;
        let raw = s.keys().next().unwrap();
        match serde_json::from_str::<Post>(raw) {
            Ok(p) => {
                let x = match data.post(&p.who, &p.message, p.clock).await {
                    Ok(_) => Bool {
                        v: true,
                        err: "".to_string(),
                    },
                    Err(e) => Bool {
                        v: false,
                        err: e.to_string(),
                    },
                };
                build_resp(&x)
            }
            Err(e) => err_response(Box::new(e)),
        }
    }

    use serde::{Deserialize, Serialize};
    use tribbler::trib::Trib;

    #[derive(Serialize, Deserialize, Debug, Clone)]
    struct UserList {
        err: String,
        users: Vec<String>,
    }

    #[derive(Serialize, Deserialize, Debug, Clone)]
    struct TribList {
        err: String,
        tribs: Vec<Arc<Trib>>,
    }

    #[derive(Serialize, Deserialize, Debug, Clone)]
    struct Bool {
        err: String,
        v: bool,
    }

    #[derive(Serialize, Deserialize, Debug, Clone)]
    struct Clock {
        err: String,
        n: u64,
    }

    #[derive(Serialize, Deserialize, Debug, Clone)]
    struct WhoWhom {
        who: String,
        whom: String,
    }

    #[derive(Serialize, Deserialize, Debug, Clone)]
    struct Post {
        who: String,
        message: String,
        clock: u64,
    }
}
