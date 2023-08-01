use crate::middleware::backend::{env_vars};
use actix_web::{
    dev::{self, Service, ServiceRequest, ServiceResponse},
    web, Error,
    web::{Bytes}
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
use actix_web::http::Version;
use std::collections::HashMap;
use std::fmt::Debug;
use pluralizer::pluralize;
use std::thread;
use std::mem::size_of_val;

const MAX_BULK_SIZE_IN_BYTES: usize = 1 * 1024* 1024; // 1MB

pub struct LoggingMiddleware<S> {
    // This is special: We need this to avoid lifetime issues.
    pub service: Rc<S>,
}

trait Body {
    fn as_str(&self) -> &str;
}

impl Body for Bytes {
    fn as_str(&self) -> &str {
        std::str::from_utf8(self).unwrap()
    }
}

#[derive(Serialize, Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
struct Data {
    version: String,
    date_created: u128,
    execution_time: u128,
    request: Request,
    response: Response
}

#[derive(Serialize, Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
struct Request {
    http_protocol: String,
    headers: HashMap<String, Vec<String>>,
    method: String,
    body: String,
    ip: String,
    resource: String,
    uri: String
}

#[derive(Serialize, Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
struct Response {
    status_code: u16,
    headers: HashMap<String, Vec<String>>,
    body: String
}

static mut PAYLOAD: Vec<String> = Vec::new();

impl<S, B> Service<ServiceRequest> for LoggingMiddleware<S>
where
    S: Service<ServiceRequest, Response = ServiceResponse<B>, Error = Error> + 'static,
    S::Future: 'static,
    B: 'static + std::fmt::Debug + actix_web::body::MessageBody,
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

            // process the headers
            let req_header_iter = req.headers();
            let mut req_headers: HashMap<String, Vec<String>> = HashMap::new();
            for (key, val) in req_header_iter {
                let value = vec![val.to_str().unwrap().to_string()];
                req_headers.insert(key.as_str().to_string(), value);
            }

            let protocol = match req.version() {
                Version::HTTP_09 => "HTTP/0.9",
                Version::HTTP_10 => "HTTP/1.0",
                Version::HTTP_11 => "HTTP/1.1",
                Version::HTTP_2 => "HTTP/2.0",
                Version::HTTP_3 => "HTTP/3.0",
                _ => "Unknown HTTP Protocol",
            };

            let request = Request {
                http_protocol: protocol.to_string(),
                headers: req_headers,
                method: req.method().as_str().to_string(),
                body: std::str::from_utf8(&body).unwrap().to_string(),
                ip: req.connection_info().realip_remote_addr().unwrap().to_string(),
                resource: convert_path_to_resource(req.path().to_string()),
                uri: req.connection_info().scheme().to_string() + "://" + req.connection_info().host() + req.path()
            };  

            req.set_payload(bytes_to_payload(body));

            // run request middleware
            let res = svc.call(req).await?;

            // time after request
            let end = SystemTime::now();
            let date_running = end
                .duration_since(UNIX_EPOCH)
                .expect("Time since running request");

            //println!("response body: {:?}", res.response().body());

            let res_header_iter = res.response().headers();
            let mut res_headers: HashMap<String, Vec<String>> = HashMap::new();
            for (key, val) in res_header_iter {
                let value = vec![val.to_str().unwrap().to_string()];
                res_headers.insert(key.as_str().to_string(), value);
            }

            let response = Response {
                status_code: u16::from(res.status()),
                body: format!("{:?}", res.response().body()),  // minor hack to get it to work
                headers: res_headers
            };

            let data = Data {
                version: "1.0.0-alpha".to_string(),
                date_created: date_created.as_millis(),
                execution_time: date_running.as_millis() - date_created.as_millis(),
                request: request,
                response: response,
            };

            let json_data = serde_json::to_string(&data)?;

            unsafe {
                PAYLOAD.push(json_data);
                if PAYLOAD.len() >= 10 || size_of_val(&*PAYLOAD) >= MAX_BULK_SIZE_IN_BYTES {

                    let k = PAYLOAD.join("\n");

                    // Retry up to 3 times with increasing intervals between attempts.
                    let retry_policy = ExponentialBackoff::builder().build_with_max_retries(3);
                    let client = ClientBuilder::new(reqwest::Client::new())
                        // Trace HTTP requests. See the tracing crate to make use of these traces.
                        // Retry failed requests.
                        .with(RetryTransientMiddleware::new_with_policy(retry_policy))
                        .build();

                    // call firetail backend and send data by spawning a thread
                    thread::spawn(|| {
                        let _ = run(client, url, api_key, k);
                        PAYLOAD.clear(); // remove the payload from memory
                    });
                }
            }

            Ok(res)
        })
    }
}

#[tokio::main]
async fn run(client: ClientWithMiddleware, url: String, api_key: String, payload: String) -> Result<(), Box<dyn std::error::Error>> {
         let res =  client
             .post(url)
             .header("content-type", "application/nd-json")
             .header("x-ft-api-key", api_key)
             .body(payload)
             .send()
             .await?;

        if res.status() == 200 {
            println!("Successfully sent to firetail");
        } else {
            println!("Error sending to firetail with error: {}", res.status());
        };

        Ok(())
}

fn bytes_to_payload(buf: web::Bytes) -> dev::Payload {
    let (_, mut pl) = h1::Payload::create(true);
    pl.unread_data(buf);
    dev::Payload::from(pl)
}

fn convert_path_to_resource(path: String) -> String {
    let items = path.split("/");
    let mut resources: Vec<String> = vec![];
    for item in items {
        resources.push(item.to_string());
    }

    let mut resource: Vec<String> = vec![];
    for (index, item) in resources.iter().enumerate() {
        if item.parse::<u64>().is_ok() {
            // go back one index to get the string
            let back = index-1;
            // pluralize and do stringId, example: "productId"
            resource.push(format!("{}Id", pluralize(&resources[back].to_string(), 1, false)));
        } else {
            resource.push(resources[index].to_string());
        }
    }

    let final_resource = resource.join("/");
    return final_resource;
}
