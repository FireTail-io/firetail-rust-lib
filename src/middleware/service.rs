use crate::middleware::backend::{env_vars};
use actix_web::{
    dev::{self, Service, ServiceRequest, ServiceResponse},
    web, Error,
};
use futures_util::future::LocalBoxFuture;
use actix_http::h1;
use std::{
    rc::Rc,
};
use reqwest_middleware::{ClientBuilder, ClientWithMiddleware};
use reqwest_retry::{RetryTransientMiddleware, policies::ExponentialBackoff};
use serde::{Serialize, Deserialize};
use std::time::{SystemTime, UNIX_EPOCH};

pub struct LoggingMiddleware<S> {
    // This is special: We need this to avoid lifetime issues.
    pub service: Rc<S>,
}

#[derive(Serialize, Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
struct Data {
    version: String,
    date_created: u128,
    execution_time: u128,
    request: Request,
    response: Response,
//    oauth: Oauth
}

#[derive(Serialize, Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
struct Request {
    http_protocol: String,
    headers: String,
    method: String,
    body: String,
    ip: String,
    resource_path: String,
    uri: String
}
#[derive(Serialize, Deserialize, Debug)]
struct Response {
    status_code: u32,
    headers: String,
    body: String,
}
#[derive(Serialize, Deserialize, Debug)]
struct Oauth {}

impl<S, B> Service<ServiceRequest> for LoggingMiddleware<S>
where
    S: Service<ServiceRequest, Response = ServiceResponse<B>, Error = Error> + 'static,
    S::Future: 'static,
    B: 'static + std::fmt::Debug,
{
    type Response = ServiceResponse<B>;
    type Error = Error;
    type Future = LocalBoxFuture<'static, Result<Self::Response, Self::Error>>;

    dev::forward_ready!(service);

    fn call(&self, mut req: ServiceRequest) -> Self::Future {
        let (api_key, url) = env_vars();
        let svc = self.service.clone();

        Box::pin(async move {
            let start = SystemTime::now();
            let date_created = start
                .duration_since(UNIX_EPOCH)
                .expect("Start time of request");

            // extract bytes from request body
            let body = req.extract::<web::Bytes>().await.unwrap();
            let req_header_iter = req.headers();
            for val in req_header_iter {
                println!("request header: {:?}", val);
            }
            println!("request method: {:?}", req.method());
            println!("request http version: {:?}", req.version());
            println!("request path {:?}", req.path());
            println!("request body (middleware): {:?}", body);

            // re-insert body back into request to be used by handlers
            req.set_payload(bytes_to_payload(body));

            let res = svc.call(req).await?;

            // Retry up to 3 times with increasing intervals between attempts.
            let retry_policy = ExponentialBackoff::builder().build_with_max_retries(3);
            let client = ClientBuilder::new(reqwest::Client::new())
                // Trace HTTP requests. See the tracing crate to make use of these traces.
                // Retry failed requests.
                .with(RetryTransientMiddleware::new_with_policy(retry_policy))
                .build();

            let end = SystemTime::now();
            let date_running = end
                .duration_since(UNIX_EPOCH)
                .expect("Time after running request");

            let request = Request {
                http_protocol: "HTTP/1.1".to_string(),
                headers: "{\"key\":[\"value\"]}".to_string(),
                method: "POST".to_string(),
                body: "{\"key\":\"value\"}".to_string(),
                ip: "\"127.0.0.1\"".to_string(),
                resource_path: "/posts/{postId}/comments".to_string(),
                uri: "http://localhost:8080".to_string()
            };

            let response = Response {
                status_code: 200,
                body: "{\"key\":\"value\"}".to_string(),
                headers: "{\"key\":[\"value\"]}".to_string()
            };

            let data = Data {
                version: "1.0.0-alpha".to_string(),
                date_created: date_created.as_millis(),
                execution_time: date_running.as_millis() - date_created.as_millis(),
                request: request,
                response: response,
            };

            let payload = serde_json::to_string(&data)?;

            // call firetail backend and send data
            run(client, url, api_key, payload).await;

            println!("response body: {:?}", res.response().body());

            let res_header_iter = res.response().headers();
            for val in res_header_iter {
                println!("response header iter: {:?}", val);
            }
            Ok(res)
        })
    }
}

async fn run(client: ClientWithMiddleware, url: String, api_key: String, payload: String) {
//    println!("url: {}", url);
        let res = client
            .post(url)
            .header("Content-Type", "application/nd-json")
            .header("x-ft-api-key", api_key)
            .body(payload)
            .send()
            .await
            .unwrap();
        println!("firetail status: {:?}", res.status())
}

fn bytes_to_payload(buf: web::Bytes) -> dev::Payload {
    let (_, mut pl) = h1::Payload::create(true);
    pl.unread_data(buf);
    dev::Payload::from(pl)
}
