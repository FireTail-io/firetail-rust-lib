use std::env;

pub fn env_vars() -> (String, String) {
    let api_key = env::var("FIRETAIL_APIKEY").expect("FIRETAIL_APIKEY env var is not set");

    let url = match env::var("FIRETAIL_URL") {
       Ok(v) => v,
       _ => String::from("https://api.logging.eu-west-1.prod.firetail.app/logs/bulk")
    };

    return (api_key, url)
}
