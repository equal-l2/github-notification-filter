use failure::{err_msg, Error, Fallible};
use reqwest::Client;

mod subscription;
use subscription::{Subscription, ThreadID};

fn read_config(filename: &str) -> Fallible<String> {
    let path = dirs::home_dir()
        .ok_or(err_msg("Failed to read ~/"))?
        .join(".ghnf")
        .join(filename);
    Ok(std::fs::read_to_string(path)?)
}

fn compile_regex() -> Fallible<regex::Regex> {
    let filters: Vec<_> = read_config("filters")?
        .split('\n')
        .filter(|s| !s.is_empty())
        .map(String::from)
        .collect();

    let filters_string = String::from(r"(?i)") + &filters.join("|");
    Ok(regex::Regex::new(&filters_string)?)
}

fn create_client() -> Fallible<Client> {
    let token = read_config("token")?
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

fn load_ignored() -> Fallible<Vec<ThreadID>> {
    Ok(read_config("ignore")?
        .split('\n')
        .filter(|s| !s.is_empty())
        .map(|s| s.parse())
        .collect::<Result<Vec<ThreadID>, _>>()?)
}

fn filter_and_unsubscribe(ss: Vec<Subscription>, no_confirm: bool, c: &Client) -> Fallible<()> {
    let re = compile_regex()?;
    let ignore = load_ignored()?;
    let candidates: Vec<_> = ss
        .into_iter()
        .filter(|s| re.is_match(&s.subject.title))
        .filter(|s| !ignore.contains(&s.thread_id))
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
        .subcommand(
            clap::SubCommand::with_name("open")
                .about("Open the thread with the web browser")
                .arg(clap::Arg::with_name("thread_id").index(1)),
        )
        .get_matches();

    let c = create_client().unwrap_or_else(|e: Error| panic!("{}", e.backtrace()));

    if let Some(sub_m) = m.subcommand_matches("open") {
        if let Some(i) = sub_m.value_of("thread_id") {
            if let Ok(n) = i.parse::<ThreadID>() {
                Subscription::from_thread_id(&c, n)
                    .unwrap()
                    .open_thread(&c)
                    .unwrap();
            }
        } else {
            println!("{}", m.usage());
        }
        return;
    }

    let list = m.is_present("list");
    let no_confirm = m.is_present("no-confirm");

    let ss = Subscription::fetch_unread(&c).unwrap_or_else(|e: Error| panic!("{}", e.backtrace()));
    if list {
        for s in ss {
            println!("{}", s);
        }
        return;
    }

    filter_and_unsubscribe(ss, no_confirm, &c)
        .unwrap_or_else(|e: Error| panic!("{}", e.backtrace()));
}
