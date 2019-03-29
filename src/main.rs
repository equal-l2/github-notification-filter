fn main() {
    let client =
    {
        let token = std::env::var("GITHUB_PERSONAL_ACCESS_TOKEN").unwrap();
        let mut headers = reqwest::header::HeaderMap::new();
        headers.insert(
            reqwest::header::AUTHORIZATION,
            reqwest::header::HeaderValue::from_str(&format!("token {}",token)).unwrap()
        );
        reqwest::Client::builder().default_headers(headers).build().unwrap()
    };

    let mut resp = client.get("https://api.github.com/notifications").send().unwrap();

    if resp.status() == 200 {
        println!("{}", resp.text().unwrap());
    } else {
        println!("Error {} ", resp.status());
    }
}
