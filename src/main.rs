use clap::ArgMatches;
use failure::{err_msg, Error, Fallible};
use rayon::prelude::*;
use regex::Regex;
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

fn compile_regex() -> Fallible<Regex> {
    let filters: Vec<_> = read_config("filters")?
        .split('\n')
        .filter(|s| !s.is_empty())
        .map(String::from)
        .collect();

    let filters_string = String::from(r"(?i)") + &filters.join("|");
    Ok(Regex::new(&filters_string)?)
}

fn create_client() -> Fallible<Client> {
    let token = read_config("token")
        .expect("Failed to read GitHub token from ~/.ghnf/token")
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
    // `ignore` is optional, return empty vec when not found
    Ok(read_config("ignore")
        .or_else(|e: failure::Error| -> _ {
            if let Some(i) = e.as_fail().downcast_ref::<std::io::Error>() {
                match i.kind() {
                    std::io::ErrorKind::NotFound => Ok("".into()),
                    _ => Err(e),
                }
            } else {
                Err(e)
            }
        })?
        .split('\n')
        .filter(|s| !s.is_empty())
        .map(|s| s.parse())
        .collect::<Result<Vec<_>, _>>()?)
}

fn filter_and_unsubscribe(ss: Vec<Subscription>, confirm: bool, c: &Client) -> Fallible<()> {
    let ignore = load_ignored()?;
    let candidates: Vec<_> = ss
        .into_iter()
        .filter(|s| !ignore.contains(&s.thread_id))
        .collect();
    if !candidates.is_empty() {
        if confirm {
            for s in candidates.iter() {
                println!("{}", s);
            }

            println!("\nTo unsubscribe the notification(s), press Enter\n(If you don't want to, just abort (e.g. Ctrl+C))");
            let mut s = String::new();
            let _ = std::io::stdin().read_line(&mut s)?;
        }

        candidates
            .par_iter()
            .map(|s: &Subscription| -> Fallible<()> {
                s.unsubscribe_thread(&c)?;
                s.mark_a_thread_as_read(&c)?;
                println!("Unsubscribed {}", s);
                Ok(())
            })
            .collect::<Result<(), _>>()?;
    } else {
        println!("No notification matched");
    }

    Ok(())
}

fn fetch_filtered(re: &Regex, c: &Client) -> Fallible<Vec<Subscription>> {
    let ss = Subscription::fetch_unread(&c)?;
    Ok(ss
        .into_iter()
        .filter(|s| re.is_match(&s.subject.title))
        .collect::<Vec<_>>())
}

fn sc_open(m: &ArgMatches) -> Fallible<()> {
    let c = create_client()?;
    let ss: Vec<Subscription> = {
        if let Some(i) = m.value_of("filter") {
            Ok(fetch_filtered(&Regex::new(i)?, &c)?)
        } else if let Ok(i) = m.value_of("thread_id").unwrap().parse() {
            Ok(vec![Subscription::from_thread_id(i, &c)?])
        } else {
            Err(err_msg("unreachable in sc_open"))
        }
    }?;
    ss.par_iter().map(|s: &Subscription| -> Fallible<()> {
        println!("Open {}", s);
        s.open_thread(&c)
    }).collect::<Fallible<()>>()
}

fn sc_list(m: &ArgMatches) -> Fallible<()> {
    let c = create_client()?;
    let ss: Vec<_> = {
        Ok(if let Some(i) = m.value_of("filter") {
            fetch_filtered(&Regex::new(i)?, &c)?
        } else {
            Subscription::fetch_unread(&c)?
        })
    }
    .unwrap_or_else(|e: Error| panic!("{} :\n{}", e, e.backtrace()));
    for s in ss {
        println!("{}", s);
    }
    Ok(())
}

fn sc_remove(m: &ArgMatches) -> Fallible<()> {
    let confirm = m.is_present("confirm");
    let c = create_client()?;
    let re = {
        if let Some(i) = m.value_of("filter") {
            Regex::new(&i)
        } else {
            Ok(compile_regex().expect("Failed to read ~/.ghnf/filters"))
        }
    }?;
    let ss = fetch_filtered(&re, &c)?;

    filter_and_unsubscribe(ss, confirm, &c)
}

fn main() {
    let m = clap::App::new("github-notification-filter")
        .version("0.2.0")
        .setting(clap::AppSettings::SubcommandRequiredElseHelp)
        .subcommand(
            clap::SubCommand::with_name("remove")
                .visible_alias("rm")
                .about("Unsubscribe notifications by regex")
                .args(&[
                    clap::Arg::with_name("confirm")
                        .help("Pause before unsubscription")
                        .long("confirm")
                        .short("c"),
                    clap::Arg::with_name("filter")
                        .help("regex to filter")
                        .long("filter")
                        .short("f")
                        .takes_value(true)
                        .conflicts_with("thread_id"),
                ]),
        )
        .subcommand(
            clap::SubCommand::with_name("open")
                .about("Open a thread, or all filtered thread with the web browser")
                .args(&[
                    clap::Arg::with_name("thread_id")
                        .index(1)
                        .required(true)
                        .conflicts_with("filter"),
                    clap::Arg::with_name("filter")
                        .help("regex to filter")
                        .long("filter")
                        .short("f")
                        .takes_value(true)
                        .conflicts_with("thread_id"),
                ]),
        )
        .subcommand(
            clap::SubCommand::with_name("list")
                .visible_alias("ls")
                .about("List unread subscriptions")
                .arg(
                    clap::Arg::with_name("filter")
                        .help("regex to filter")
                        .long("filter")
                        .short("f")
                        .takes_value(true),
                ),
        )
        .get_matches();

    match m.subcommand() {
        ("open", Some(sub_m)) => sc_open(sub_m),
        ("list", Some(sub_m)) => sc_list(sub_m),
        ("remove", Some(sub_m)) => sc_remove(sub_m),
        _ => Ok(()),
    }
    .unwrap_or_else(|e: Error| panic!("{} :\n{}", e, e.backtrace()));
}
