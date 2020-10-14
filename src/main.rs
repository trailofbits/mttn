use anyhow::Result;
use clap::{App, Arg};

use std::process;

mod trace;

fn app<'a, 'b>() -> App<'a, 'b> {
    App::new(env!("CARGO_PKG_NAME"))
        .version(env!("CARGO_PKG_VERSION"))
        .about(env!("CARGO_PKG_DESCRIPTION"))
        .arg(
            Arg::with_name("mode")
                .help("The CPU mode to decode instructions with")
                .short("m")
                .long("mode")
                .takes_value(true)
                .possible_values(&["32", "64"])
                .default_value("64"),
        )
        .arg(
            Arg::with_name("tracee")
                .help("The program to trace")
                .index(1)
                .required(true),
        )
        .arg(
            Arg::with_name("tracee-args")
                .help("The command-line arguments to pass to the tracee process")
                .raw(true),
        )
}

fn run() -> Result<()> {
    let mut tracer = trace::Tracer::from(app().get_matches());

    let traces = tracer.trace()?;
    serde_json::to_writer(std::io::stdout(), &traces)?;

    Ok(())
}

fn main() {
    env_logger::init();

    process::exit(match run() {
        Ok(()) => 0,
        Err(e) => {
            eprintln!("Fatal: {}", e);
            1
        }
    });
}
