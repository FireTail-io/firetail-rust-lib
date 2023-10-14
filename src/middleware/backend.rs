use std::env;

pub fn env_vars() -> (String, String) {
    let api_key = env::var("FIRETAIL_APIKEY").expect("FIRETAIL_APIKEY env var is not set");

    let url = env::var("FIRETAIL_URL").expect("FIRETAIL_URL is not set");
    if url.is_empty() {
      let url = "https://api.logging.eu-west-1.prod.firetail.app".to_string();
    }

    return (api_key, url)
}
