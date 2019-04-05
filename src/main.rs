use failure::{err_msg, Fallible};
use reqwest::Client;
use std::io::{BufRead, BufReader};

mod subscription;

use subscription::Subscription;

fn compile_regex() -> Fallible<regex::Regex> {
    let path = dirs::home_dir().unwrap().join(".ghnf").join("filters");
    let f = std::fs::File::open(path)?;
    let filters = BufReader::new(f)
        .lines()
        .collect::<Result<Vec<_>, std::io::Error>>()?;
    let filters_string = String::from(r"(?i)") + &filters.join("|");
    Ok(regex::Regex::new(&filters_string)?)
}

fn create_client() -> Fallible<Client> {
    let path = dirs::home_dir().unwrap().join(".ghnf").join("token");
    let token = std::fs::read_to_string(path)?
        .split('\n')
        .nth(0)
        .ok_or(err_msg("Malformed GitHub Personal Access Token"))?
        .to_owned();
    let mut headers = reqwest::header::HeaderMap::new();
    headers.insert(
        reqwest::header::AUTHORIZATION,
        reqwest::header::HeaderValue::from_str(&format!("token {}", token))?,
    );
    Ok(reqwest::Client::builder()
        .default_headers(headers)
        .build()?)
}

fn filter_and_unsubscribe(ss: Vec<Subscription>, no_confirm: bool, c: &Client) -> Fallible<()> {
    let re = compile_regex().unwrap();
    let candidates: Vec<_> = ss
        .into_iter()
        .filter(|s| re.is_match(&s.subject.title))
        .collect();
    if !candidates.is_empty() {
        if !no_confirm {
            for s in candidates.iter() {
                println!("{}", s);
            }

            println!("\nTo unsubscribe the notification(s), press Enter\n(If you don't want to, just abort (e.g. Ctrl+C))");
            let mut s = String::new();
            let _ = std::io::stdin().read_line(&mut s)?;
        }

        for s in candidates.iter() {
            s.unsubscribe_thread(&c)?;
            s.mark_a_thread_as_read(&c)?;
            println!("Unsubscribed {}", s);
        }
    } else {
        println!("No notification matched");
    }

    Ok(())
}

fn main() {
    let m = clap::App::new("github-notification-filter")
        .version("0.2.0")
        .arg(
            clap::Arg::with_name("list")
                .help("List all subscriptions")
                .long("list"),
        )
        .arg(
            clap::Arg::with_name("no-confirm")
                .help("Do not pause before unsubscription")
                .long("no-confirm")
                .short("y"),
        )
        .get_matches();

    let list = m.is_present("list");
    let no_confirm = m.is_present("no-confirm");

    let c = create_client().unwrap();
    let ss = Subscription::fetch_unread(&c).unwrap();
    if list {
        for s in ss {
            println!("{}", s);
        }
        return;
    }

    filter_and_unsubscribe(ss, no_confirm, &c).unwrap();
}
