use anyhow::Result;
use clap::{App, Arg, ArgGroup};

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
            Arg::with_name("ignore-unsupported-memops")
                .help("Ignore unsupported memory ops instead of failing")
                .short("I")
                .long("ignore-unsupported-memops"),
        )
        .arg(
            Arg::with_name("debug-on-fault")
                .help("Suspend the tracee and detach if a memory access faults")
                .short("d")
                .long("debug-on-fault"),
        )
        .arg(
            Arg::with_name("tracee-pid")
                .help("Attach to the given PID for tracing")
                .short("a")
                .long("attach")
                .takes_value(true),
        )
        .arg(
            Arg::with_name("tracee-name")
                .help("The program to trace")
                .index(1)
        )
        .arg(
            Arg::with_name("tracee-args")
                .help("The command-line arguments to execute the tracee with")
                .raw(true)
        )
        .group(
            ArgGroup::with_name("target")
                .required(true)
                .args(&["tracee-pid", "tracee-name"]),
        )
}

fn run() -> Result<()> {
    let tracer = trace::Tracer::from(app().get_matches());

    let traces = tracer.trace()?.collect::<Result<Vec<trace::Step>>>()?;
    serde_json::to_writer(std::io::stdout(), &traces)?;

    Ok(())
}

fn main() {
    env_logger::init();

    process::exit(match run() {
        Ok(()) => 0,
        Err(e) => {
            eprintln!("Fatal: {:#}", e);
            1
        }
    });
}
