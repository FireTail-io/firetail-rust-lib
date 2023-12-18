## Rust Library
* Note: The library is built for actix at the moment

### Before running this library
* Make sure there are firetail environment variables. The two environment variables needed are:
1. `FIRETAIL_APIKEY` - This is the API Key used to communicate with firetail servers
2. `FIRETAIL_URL` - This is the firetail backend URL, which can be set the US Platform and by default it points to EU (Europe) Platform. If you are using the US Platform, it is recommended to set it to `https://api.logging.us-east-2.prod.us.firetail.app`

### Example code to run it with actix
```rust
use actix_web::{get, post, web, App, HttpResponse, HttpServer, Responder};
use firetail_rust_lib::FiretailLogging;

#[get("/")]
async fn hello() -> impl Responder {
    HttpResponse::Ok().body("Hello world!")
}

#[post("/echo")]
async fn echo(req_body: String) -> impl Responder {
    HttpResponse::Ok().body(req_body)
}

async fn manual_hello() -> impl Responder {
    HttpResponse::Ok().body("Hey there!")
}

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    HttpServer::new(|| {
        App::new()
            .wrap(FiretailLogging::default())
            .service(hello)
            .service(echo)
            .route("/hey", web::get().to(manual_hello))
    })
    .bind(("127.0.0.1", 8080))?
    .run()
    .await
}
```

You will need to include the firetail library in `Cargo.toml`
```toml
firetail-rust-lib = "0.0.1"
```

then add the library in your actix's main.rs
```rust
use firetail_actix_lib::FiretailLogging;
```

then activate firetail in your http code:
```rust
    HttpServer::new(|| {
        App::new()
            .wrap(FiretailLogging::default())
            .service(hello)
            .service(echo)
            .route("/hey", web::get().to(manual_hello))
    })
```
