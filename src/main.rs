#![warn(rust_2018_idioms)]
#![warn(rust_2018_compatibility)]
#![warn(future_incompatible)]
#![warn(clippy::nursery)]
#![warn(clippy::pedantic)]
#![allow(clippy::fallible_impl_from)]
#![allow(clippy::filter_map)]

use clap::{crate_version, App, AppSettings, Arg, ArgMatches, SubCommand};
use failure::{err_msg, format_err, Error, Fallible};
use futures::future;
use reqwest::Client;

mod subscription;
mod util;

use crate::subscription::gh_objects::SubjectType;
use crate::subscription::Subscription;
use util::Filters;

async fn sc_open(m: &ArgMatches<'_>, c: &Client) -> Fallible<()> {
    let ss: Vec<Subscription> = {
        if let Some(i) = m.values_of("thread_ids") {
            let mut ids = vec![];
            for v in i {
                let id_str = v.parse();
                if let Ok(id) = id_str {
                    if let Ok(s) = Subscription::from_thread_id(id, &c).await {
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
            util::fetch_filtered(Filters::new(m, false)?, &c).await
        }
    }?;
    println!("Finished filtering, now open {} page(s)...", ss.len());

    let mut futs = vec![];
    for s in ss {
        let c_ref = &c;
        futs.push(async move {
            println!("Open {}", s);
            s.open(c_ref).await
        });
    }

    future::try_join_all(futs).await?;

    Ok(())
}

async fn sc_list(m: &ArgMatches<'_>, c: &Client) -> Fallible<()> {
    let ss = util::fetch_filtered(Filters::new(m, false)?, &c).await.unwrap_or_else(|e: Error| panic!("{} :\n{}", e, e.backtrace()));

    for s in ss {
        println!("{}", s);
    }

    Ok(())
}

async fn sc_remove(m: &ArgMatches<'_>, c: &Client) -> Fallible<()> {
    let dry = m.is_present("dry-run");

    let ss = util::fetch_filtered(Filters::new(m, true)?, &c).await?;
    println!("{} notifications left", ss.len());

    util::filter_and_unsubscribe(ss, dry, &c).await
}

async fn sc_request(m: &ArgMatches<'_>, c: &Client) -> Fallible<()> {
    let url = m.value_of("URL").unwrap();
    let resp = c.get(url).send().await?;
    if resp.status() != 200 {
        println!("Failed to GET, status code: {}", resp.status());
    }
    eprintln!("Headers:\n{:?}", resp.headers());
    if let Ok(i) = resp.text().await {
        println!("{}", i);
    }
    Ok(())
}

#[tokio::main]
async fn main() {
    let m = App::new("github-notification-filter")
        .version(format!("{} (built at {})", crate_version!(), env!("BUILD_DATE")).as_str())
        .setting(AppSettings::ColoredHelp)
        .setting(AppSettings::SubcommandRequiredElseHelp)
        .subcommand(
            SubCommand::with_name("remove")
                .about("Unsubscribe notifications by regex")
                .args(&[
                    Arg::with_name("dry-run")
                        .help("Do not unsubscribe, but list threads to be unsubscribed")
                        .long("dry-run")
                        .short("d"),
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
                        .takes_value(true)
                        .possible_values(&["commit", "issue", "pr"]),
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
                        .takes_value(true)
                        .possible_values(&["commit", "issue", "pr"]),
                ]),
        )
        .subcommand(
            SubCommand::with_name("list")
                .about("List unread subscriptions")
                .args(&[
                    Arg::with_name("filter")
                        .help("regex to filter")
                        .long("filter")
                        .short("f")
                        .takes_value(true),
                    Arg::with_name("kind")
                        .help("specify a kind of notification (\"commit\", \"issue\", or \"pr\"")
                        .short("k")
                        .takes_value(true)
                        .possible_values(&["commit", "issue", "pr"]),
                ])
                .visible_alias("ls"),
        )
        .subcommand(
            SubCommand::with_name("request")
                .about("Make a GET request to URL using ~/.ghnf/token (for devs)")
                .arg(Arg::with_name("URL").index(1).required(true))
                .visible_alias("req"),
        )
        .get_matches();

    let c = util::create_client().unwrap();
    match m.subcommand() {
        ("open", Some(sub_m)) => sc_open(sub_m, &c).await,
        ("list", Some(sub_m)) => sc_list(sub_m, &c).await,
        ("remove", Some(sub_m)) => sc_remove(sub_m, &c).await,
        ("request", Some(sub_m)) => sc_request(sub_m, &c).await,
        _ => unreachable!(),
    }
    .unwrap()
}
