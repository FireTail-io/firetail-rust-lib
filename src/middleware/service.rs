use crate::middleware::backend::{env_vars};
use actix_web::{
    dev::{self, Service, ServiceRequest, ServiceResponse, Transform},
    web, Error,
};
use futures_util::future::LocalBoxFuture;
use actix_http::h1;
use std::{
    future::{ready, Ready},
    rc::Rc,
};
use reqwest_middleware::{ClientBuilder, ClientWithMiddleware};
use reqwest_retry::{RetryTransientMiddleware, policies::ExponentialBackoff};
use reqwest_tracing::TracingMiddleware;

pub struct LoggingMiddleware<S> {
    // This is special: We need this to avoid lifetime issues.
    pub service: Rc<S>,
}

impl<S, B> Service<ServiceRequest> for LoggingMiddleware<S>
where
    S: Service<ServiceRequest, Response = ServiceResponse<B>, Error = Error> + 'static,
    S::Future: 'static,
    B: 'static,
{
    type Response = ServiceResponse<B>;
    type Error = Error;
    type Future = LocalBoxFuture<'static, Result<Self::Response, Self::Error>>;

    dev::forward_ready!(service);

    fn call(&self, mut req: ServiceRequest) -> Self::Future {
        let (api_key, url) = env_vars();
        let svc = self.service.clone();

        Box::pin(async move {
            // extract bytes from request body
            let body = req.extract::<web::Bytes>().await.unwrap();
            println!("request body (middleware): {body:?}");

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

            run(client, url).await;
            println!("response: {:?}", res.headers());
            Ok(res)
        })
    }
}

async fn run(client: ClientWithMiddleware, url: String) {
    println!("url: {}", url);
    client
        .post(url)
        .header("foo", "bar")
        .send()
        .await
        .unwrap();
}

fn bytes_to_payload(buf: web::Bytes) -> dev::Payload {
    let (_, mut pl) = h1::Payload::create(true);
    pl.unread_data(buf);
    dev::Payload::from(pl)
}
