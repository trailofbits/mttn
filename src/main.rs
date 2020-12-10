use anyhow::Result;
use clap::{App, Arg, ArgGroup};

use std::io::stdout;
use std::process;

mod tiny86;
mod trace;

use tiny86::Tiny86Write;
use trace::Step;

fn app<'a, 'b>() -> App<'a, 'b> {
    App::new(env!("CARGO_PKG_NAME"))
        .version(env!("CARGO_PKG_VERSION"))
        .about(env!("CARGO_PKG_DESCRIPTION"))
        .arg(
            Arg::with_name("output-format")
                .help("The output format to use")
                .short("F")
                .long("format")
                .takes_value(true)
                .possible_values(&["json", "tiny86"])
                .default_value("json"),
        )
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
            Arg::with_name("disable-aslr")
                .help("Disable ASLR on the tracee")
                .short("A")
                .long("disable-aslr"),
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
                .index(1),
        )
        .arg(
            Arg::with_name("tracee-args")
                .help("The command-line arguments to execute the tracee with")
                .raw(true),
        )
        .group(
            ArgGroup::with_name("target")
                .required(true)
                .args(&["tracee-pid", "tracee-name"]),
        )
}

fn run() -> Result<()> {
    let matches = app().get_matches();
    let tracer = trace::Tracer::from(&matches);

    let mut traces = tracer.trace()?;

    match matches.value_of("output-format").unwrap() {
        "json" => serde_json::to_writer(stdout(), &traces.collect::<Result<Vec<Step>>>()?)?,
        "tiny86" => traces.try_for_each(|s| s?.tiny86_write(&mut stdout()))?,
        _ => unreachable!(),
    };

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
