use crate::LoggingMiddleware;
use std::{
    future::{ready, Ready},
    rc::Rc,
};

use actix_web::{
    dev::{Service, ServiceRequest, ServiceResponse, Transform}, Error,
};

pub struct FiretailLogging;

impl Default for FiretailLogging {
    fn default() -> FiretailLogging {
        FiretailLogging
    }
}

impl<S: 'static, B> Transform<S, ServiceRequest> for FiretailLogging
where
    S: Service<ServiceRequest, Response = ServiceResponse<B>, Error = Error>,
    S::Future: 'static,
    B: 'static + std::fmt::Debug + actix_web::body::MessageBody,
{
    type Response = ServiceResponse<B>;
    type Error = Error;
    type InitError = ();
    type Transform = LoggingMiddleware<S>;
    type Future = Ready<Result<Self::Transform, Self::InitError>>;

    fn new_transform(&self, service: S) -> Self::Future {
        ready(Ok(LoggingMiddleware {
            service: Rc::new(service),
        }))
    }
}
