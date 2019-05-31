mod subscription;
mod util;

use crate::subscription::Subscription;
use clap::ArgMatches;
use failure::{err_msg, Error, Fallible};
use rayon::prelude::*;
use regex::Regex;

fn sc_open(m: &ArgMatches) -> Fallible<()> {
    let c = util::create_client()?;
    let ss: Vec<Subscription> = {
        if let Some(i) = m.value_of("filter") {
            util::fetch_filtered(&Regex::new(i)?, &c)
        } else if let Ok(i) = m.value_of("thread_id").unwrap().parse() {
            Ok(vec![Subscription::from_thread_id(i, &c)?])
        } else {
            Err(err_msg("unreachable in sc_open"))
        }
    }?;
    ss.into_par_iter()
        .map(|s| -> _ {
            println!("Open {}", s);
            s.open_thread(&c)
        })
        .collect()
}

fn sc_list(m: &ArgMatches) -> Fallible<()> {
    let c = util::create_client()?;
    let ss: Vec<_> = {
        Ok(if let Some(i) = m.value_of("filter") {
            util::fetch_filtered(&Regex::new(i)?, &c)?
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
    let c = util::create_client()?;
    let re = {
        if let Some(i) = m.value_of("filter") {
            Regex::new(&i).map_err(Into::into)
        } else {
            util::compile_regex()
        }
    }?;

    let ss = util::fetch_filtered(&re, &c)?;
    println!("{} notifications left", ss.len());

    util::filter_and_unsubscribe(ss, confirm, &c)
}

fn sc_request(m: &ArgMatches) -> Fallible<()> {
    let url = m.value_of("URL").unwrap();
    let c = util::create_client()?;
    let mut resp = c.get(url).send()?;
    if resp.status() != 200 {
        println!("Failed to GET, status code: {}", resp.status());
        if let Ok(i) = resp.text() {
            println!("{}", i);
        }
    } else {
        println!("{}", resp.text().unwrap());
    }
    Ok(())
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
        .subcommand(
            clap::SubCommand::with_name("request")
                .visible_alias("req")
                .about("Make a GET request to URL using ~/.ghnf/token (for devs)")
                .arg(clap::Arg::with_name("URL").index(1).required(true)),
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
