extern crate nlib;
extern crate getopts;
extern crate stats;

use getopts::Options;
use nlib::generators::*;
use nlib::simulators::*;
use stats::OnlineStats;
use std::env;
use std::fmt;

const DEFAULT_RATE: u32 = 10;
const DEFAULT_PSIZE: u32 = 1;
const DEFAULT_LSPEED: u32 = 1_000_000;
const DEFAULT_DURATION: u32 = 5;
const DEFAULT_CLIENT_COUNT: u32 = 10;
const DEFAULT_PERSISTENCE: bool = false;

struct Params {
    rate: u32,
    psize: u32,
    lspeed: u32,
    duration: u32,
    ncount: u32,
    persistence: bool,
    resolution: f64,
}

impl fmt::Display for Params {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        writeln!(f, "Simulation configuration:").unwrap();
        writeln!(f, "\t Rate:                  {} packets/s", self.rate).unwrap();
        writeln!(f, "\t Packet size:           {} bits", self.psize).unwrap();
        writeln!(f, "\t LAN speed:             {} bits/s", self.lspeed).unwrap();
        writeln!(f, "\t Simulation duration:   {}s", self.duration).unwrap();
        writeln!(f, "\t Client count:            {} Clients", self.ncount).unwrap();
        writeln!(f, "\t CSMA/CD Persistence:   {}", self.persistence).unwrap();
        writeln!(f, "\t Resolution:            1µs").unwrap(); // TODO(irfansharif).
        write!(
            f,
            "\t Ticks per packet:      {}",
            f64::from(self.psize) / f64::from(self.lspeed) * self.resolution
        )
    }
}

fn construct_options() -> Options {
    let mut opts = Options::new();
    opts.optflag("h", "help", "Display this message");
    opts.optopt(
        "",
        "rate",
        &format!(
            "Average number of generated packets/s (def: {})",
            DEFAULT_RATE
        ),
        "NUM",
    );
    opts.optopt(
        "",
        "psize",
        &format!("Packet size; bits (def: {})", DEFAULT_PSIZE),
        "NUM",
    );
    opts.optopt(
        "",
        "lspeed",
        &format!(
            "LAN speed in terms of bits read from/written to network links; bits/s (def: {})",
            DEFAULT_LSPEED
        ),
        "NUM",
    );
    opts.optopt(
        "",
        "duration",
        &format!(
            "Duration of simulation; seconds (def: {})",
            DEFAULT_DURATION
        ),
        "NUM",
    );
    opts.optopt(
        "",
        "ncount",
        &format!(
            "Number of Clients connected to the LAN (def: {})",
            DEFAULT_CLIENT_COUNT
        ),
        "NUM",
    );
    opts.optflag(
        "",
        "persistence",
        &format!(
            "Simulate 1-persistent CSMA/CD protocol (def: {:?})",
            DEFAULT_PERSISTENCE
        ),
    );
    opts
}

fn parse_params(matches: &getopts::Matches) -> Params {
    let rate = match matches.opt_str("rate") {
        Some(x) => x.parse::<u32>().unwrap(),
        None => DEFAULT_RATE,
    };
    let psize = match matches.opt_str("psize") {
        Some(x) => x.parse::<u32>().unwrap(),
        None => DEFAULT_PSIZE,
    };
    let lspeed = match matches.opt_str("lspeed") {
        Some(x) => x.parse::<u32>().unwrap(),
        None => DEFAULT_LSPEED,
    };
    let duration = match matches.opt_str("duration") {
        Some(x) => x.parse::<u32>().unwrap(),
        None => DEFAULT_DURATION,
    };
    let ncount = match matches.opt_str("ncount") {
        Some(x) => x.parse::<u32>().unwrap(),
        None => DEFAULT_CLIENT_COUNT,
    };
    let persistence = if matches.opt_present("persistence") {
        true
    } else {
        DEFAULT_PERSISTENCE
    };
    let resolution = 1e6; // TODO(irfansharif).

    Params {
        rate,
        psize,
        lspeed,
        duration,
        ncount,
        persistence,
        resolution,
    }
}

fn print_usage(program: &str, opts: &Options) {
    let brief = format!("Usage: {} [Options]", program);
    print!("{}", opts.usage(&brief));
}

fn main() {
    let args: Vec<String> = env::args().collect();
    let program = args[0].clone();

    let opts = construct_options();
    let matches = match opts.parse(&args[1..]) {
        Ok(m) => m,
        Err(f) => {
            println!("{}: illegal usage -- {}", program, f.to_string());
            print_usage(&program, &opts);
            std::process::exit(1)
        }
    };

    if matches.opt_present("h") {
        print_usage(&program, &opts);
        return;
    }

    let params = parse_params(&matches);
    println!("{}", params);

    let ticks = params.duration * params.resolution as u32;
    let mut clients: Vec<_> = (0..params.ncount)
        .map(|i| {
            Client::new(
                Markov::new(f64::from(params.rate)),
                params.resolution,
            )
        })
        .collect();

    let mut server = Server::new(params.resolution, f64::from(params.lspeed), None);
    let mut pstats = OnlineStats::new();

    for i in 0..ticks {
        // TODO(irfansharif): Look at and try to use smart pointers, share link ownership with
        // Clients and the Server such that the main loop body simply ticks all participants instead of
        // additionally shuffling data around.
        
        if let Some(p) = server.tick() {
            // We record the time it took for the processed packet to get processed.
            pstats.add(f64::from(i - p.time_generated) / params.resolution);
        }
    }

    println!("Simulation results:");
    println!(
        "\t Average sojourn time:              {:.4} +/- {:.4} seconds",
        pstats.mean(),
        pstats.stddev()
    );
    let packets_generated: u32 = clients
        .iter()
        .map(|client| client.packets_generated())
        .sum();
    println!(
        "\t Packets generated:                 {} packets",
        packets_generated
    );
    println!(
        "\t Packets processed:                 {} packets",
        server.packets_processed()
    );
}
