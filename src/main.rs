#![warn(rust_2018_idioms)]
#![warn(rust_2018_compatibility)]
#![warn(future_incompatible)]
use clap::{crate_version, App, AppSettings, Arg, ArgMatches, SubCommand};
use failure::{err_msg, format_err, Error, Fallible};
use rayon::prelude::*;
use regex::Regex;

mod subscription;
mod util;

use crate::subscription::gh_objects::SubjectType;
use crate::subscription::Subscription;

fn sc_open(m: &ArgMatches<'_>) -> Fallible<()> {
    let k = m.value_of("kind").and_then(|v| match v {
        "commit" => Some(SubjectType::Commit),
        "issue" => Some(SubjectType::Issue),
        "pr" => Some(SubjectType::PullRequest),
        _ => {
            eprintln!("Invalid argument for <kind>, expected \"commit\", \"issue\", or \"pr\"");
            std::process::exit(1)
        }
    });
    let n = m.value_of("count").and_then(|v| {
        Some(v.parse().unwrap_or_else(|_| {
            eprintln!("Invalid argument for <count>, expected integer");
            std::process::exit(1)
        }))
    });
    let c = util::create_client()?;
    let ss: Vec<Subscription> = {
        if let Some(i) = m.value_of("filter") {
            util::fetch_filtered(&Regex::new(i)?, n, k, &c)
        } else if let Some(i) = k {
            Ok(Subscription::fetch_unread(&c)?
                .into_par_iter()
                .filter(|v| v.subject.r#type == i)
                .collect())
        } else if let Some(i) = m.values_of("thread_ids") {
            let mut ids = vec![];
            for v in i {
                let id_str = v.parse();
                if let Ok(id) = id_str {
                    if let Ok(s) = Subscription::from_thread_id(id, &c) {
                        ids.push(s);
                    } else {
                        return Err(err_msg(format_err!("could not retrieve: {}", id)));
                    }
                } else {
                    return Err(err_msg(format_err!("malformed input: {}", v)));
                }
            }
            Ok(ids)
        } else {
            unreachable!();
        }
    }?;
    ss.into_par_iter()
        .map(|s| -> _ {
            println!("Open {}", s);
            s.open_thread(&c)
        })
        .collect()
}

fn sc_list(m: &ArgMatches<'_>) -> Fallible<()> {
    let c = util::create_client()?;
    let ss: Vec<_> = {
        Ok(if let Some(i) = m.value_of("filter") {
            util::fetch_filtered(&Regex::new(i)?, None, None, &c)?
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

fn sc_remove(m: &ArgMatches<'_>) -> Fallible<()> {
    let k = m.value_of("kind").and_then(|v| match v {
        "commit" => Some(SubjectType::Commit),
        "issue" => Some(SubjectType::Issue),
        "pr" => Some(SubjectType::PullRequest),
        _ => {
            eprintln!("Invalid argument for <kind>, expected \"commit\", \"issue\", or \"pr\"");
            std::process::exit(1)
        }
    });
    let n = m.value_of("count").and_then(|v| {
        Some(v.parse().unwrap_or_else(|_| {
            eprintln!("Invalid argument for <count>, expected integer");
            std::process::exit(1)
        }))
    });
    let confirm = m.is_present("confirm");
    let c = util::create_client()?;
    let re = {
        if let Some(i) = m.value_of("filter") {
            Regex::new(&i).map_err(Into::into)
        } else {
            util::compile_regex()
        }
    }?;

    let ss = util::fetch_filtered(&re, n, k, &c)?;
    println!("{} notifications left", ss.len());

    util::filter_and_unsubscribe(ss, confirm, &c)
}

fn sc_request(m: &ArgMatches<'_>) -> Fallible<()> {
    let url = m.value_of("URL").unwrap();
    let c = util::create_client()?;
    let mut resp = c.get(url).send()?;
    if resp.status() != 200 {
        println!("Failed to GET, status code: {}", resp.status());
    }
    println!("Headers:\n{:?}", resp.headers());
    if let Ok(i) = resp.text() {
        println!("Body: {}", i);
    }
    Ok(())
}

fn main() {
    let m = App::new("github-notification-filter")
        .version(format!("{} (built at {})", crate_version!(), env!("BUILD_DATE")).as_str())
        .setting(AppSettings::SubcommandRequiredElseHelp)
        .subcommand(
            SubCommand::with_name("remove")
                .about("Unsubscribe notifications by regex")
                .args(&[
                    Arg::with_name("confirm")
                        .help("Pause before unsubscription")
                        .long("confirm")
                        .short("c"),
                    Arg::with_name("count")
                        .help("only process specified count (the order is undetermined)")
                        .short("n")
                        .takes_value(true),
                    Arg::with_name("filter")
                        .help("regex to filter")
                        .long("filter")
                        .short("f")
                        .takes_value(true),
                    Arg::with_name("kind")
                        .help("specify a kind of notification (\"commit\", \"issue\", or \"pr\"")
                        .short("k")
                        .takes_value(true),
                ])
                .visible_alias("rm"),
        )
        .subcommand(
            SubCommand::with_name("open")
                .about("Open a thread, or all filtered thread with the web browser")
                .args(&[
                    Arg::with_name("count")
                        .help("only process specified count (the order is undetermined)")
                        .short("n")
                        .takes_value(true),
                    Arg::with_name("thread_ids")
                        .conflicts_with("filter")
                        .conflicts_with("kind")
                        .min_values(1)
                        .required(true),
                    Arg::with_name("filter")
                        .conflicts_with("thread_ids")
                        .help("regex to filter")
                        .long("filter")
                        .short("f")
                        .takes_value(true),
                    Arg::with_name("kind")
                        .conflicts_with("thread_ids")
                        .help("specify a kind of notification (\"commit\", \"issue\", or \"pr\"")
                        .short("k")
                        .takes_value(true),
                ]),
        )
        .subcommand(
            SubCommand::with_name("list")
                .about("List unread subscriptions")
                .arg(
                    Arg::with_name("filter")
                        .help("regex to filter")
                        .long("filter")
                        .short("f")
                        .takes_value(true),
                )
                .visible_alias("ls"),
        )
        .subcommand(
            SubCommand::with_name("request")
                .about("Make a GET request to URL using ~/.ghnf/token (for devs)")
                .arg(Arg::with_name("URL").index(1).required(true))
                .visible_alias("req"),
        )
        .get_matches();

    match m.subcommand() {
        ("open", Some(sub_m)) => sc_open(sub_m),
        ("list", Some(sub_m)) => sc_list(sub_m),
        ("remove", Some(sub_m)) => sc_remove(sub_m),
        ("request", Some(sub_m)) => sc_request(sub_m),
        _ => Ok(()),
    }
    .unwrap_or_else(|e: Error| panic!("{} :\n{}", e, e.backtrace()));
}
