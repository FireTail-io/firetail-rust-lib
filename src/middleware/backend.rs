use std::env;

pub fn env_vars() -> (String, String) {
    let api_key = env::var("FIRETAIL_APIKEY").expect("FIRETAIL_APIKEY env var is not set");
    let url = env::var("FIRETAIL_URL").expect("FIRETAIL_URL env var is not set");

    return (api_key, url)
}
