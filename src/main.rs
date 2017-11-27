extern crate nlib;
extern crate getopts;
extern crate stats;
extern crate bit_vec;

use bit_vec::BitVec;
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
const DEFAULT_SERVER_COUNT: usize = 10;
const DEFAULT_PERSISTENCE: bool = false;
const DEFAULT_REPORT_GEN: bool = false;

struct Params {
    rate: u32,
    psize: u32,
    lspeed: u32,
    duration: u32,
    ncount: usize,
    persistence: bool,
    resolution: f64,
    gen_report: bool,
}

impl fmt::Display for Params {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        writeln!(f, "Simulation configuration:").unwrap();
        writeln!(f, "\t Rate:                  {} packets/s", self.rate).unwrap();
        writeln!(f, "\t Packet size:           {} bits", self.psize).unwrap();
        writeln!(f, "\t LAN speed:             {} bits/s", self.lspeed).unwrap();
        writeln!(f, "\t Simulation duration:   {}s", self.duration).unwrap();
        writeln!(f, "\t Server count:          {} Clients", self.ncount).unwrap();
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
            DEFAULT_SERVER_COUNT
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
    opts.optflag(
        "",
        "gen_report",
        &format!(
            "Generates the lab report (def: {:?})",
            DEFAULT_REPORT_GEN,
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
        Some(x) => x.parse::<usize>().unwrap(),
        None => DEFAULT_SERVER_COUNT,
    };
    let persistence = if matches.opt_present("persistence") {
        true
    } else {
        DEFAULT_PERSISTENCE
    };
    let gen_report = if matches.opt_present("gen_report") {
        true
    } else {
        false
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
        gen_report,
    }
}

fn print_usage(program: &str, opts: &Options) {
    let brief = format!("Usage: {} [Options]", program);
    print!("{}", opts.usage(&brief));
}

fn gen_report() {
    let resolution = 1e6;
    let pspeed = 1e6;
    let psize = 8000;
    let ticks = (resolution * 10.0) as u32;

    // Question 1: Non persistent
    let n_vals = vec![4, 6, 8, 10, 12, 14, 16];
    let a_vals = vec![4, 6, 8];
    println!("A, N, Throughput, Delay");
    for a in a_vals {
        for n in n_vals.clone() {
            let mut total_processed: f64 = 0.0;
            let mut total_delay: f64 = 0.0;
            for _ in 0..10 {
                let mut servers: Vec<_> = (0..n)
                    .map(|id| {
                        Server::new(
                            id,
                            Markov::new(f64::from(a)),
                            psize,
                            resolution,
                            f64::from(pspeed),
                            true,
                        )
                    })
                    .collect();

                let mut pstats = OnlineStats::new();
                let mut medium = Medium::new(n, 26);
                for i in 0..ticks {
                    let mut local_state = BitVec::from_elem(n, false);
                    for server in servers.iter_mut() {
                        if let Some(p) = server.tick(&mut local_state, &medium, i) {
                            pstats.add(f64::from(i - p.time_generated) / resolution);
                        }
                    }
                    medium.write(local_state);
                    medium.tick();
                }
                let curr_processed: u32 = servers
                    .iter()
                    .map(|server| server.packets_processed())
                    .sum();
                total_processed += curr_processed as f64;
                total_delay += pstats.mean();
            }
            println!("{}, {}, {}, {}", a, n, (total_processed * psize as f64)/10.0, total_delay/10.0);
        }
    }
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

    if params.gen_report {
        gen_report();
        return;
    }

    let ticks = params.duration * params.resolution as u32;
    let mut servers: Vec<_> = (0..params.ncount)
        .map(|id| {
            Server::new(
                id,
                Markov::new(f64::from(params.rate)),
                params.psize,
                params.resolution,
                f64::from(params.lspeed),
                params.persistence,
            )
        })
        .collect();

    let mut pstats = OnlineStats::new();

    // Hardcode a 25.6 (rounding up to 26) microsecond delay
    let mut medium = Medium::new(params.ncount, 26);

    for i in 0..ticks {
        // TODO(irfansharif): Look at and try to use smart pointers, share link ownership with
        // Clients and the Server such that the main loop body simply ticks all participants instead of
        // additionally shuffling data around.
        let mut local_state = BitVec::from_elem(params.ncount, false);
        // TODO: Be able to handle multiple packet output
        // With a packet length of 1000, its impossible for more than 1 packet to be outputted at a given tick
        for server in servers.iter_mut() {
            if let Some(p) = server.tick(&mut local_state, &medium, i) {
                pstats.add(f64::from(i - p.time_generated) / params.resolution);
            }
        }
        medium.write(local_state);
        medium.tick();
    }

    println!("Simulation results:");
    println!(
        "\t Average sojourn time:              {:.4} +/- {:.4} seconds",
        pstats.mean(),
        pstats.stddev()
    );
    let packets_generated: u32 = servers
        .iter()
        .map(|server| server.packets_generated())
        .sum();
    println!(
        "\t Packets generated:                 {} packets",
        packets_generated
    );
    let packets_processed: u32 = servers
        .iter()
        .map(|server| server.packets_processed())
        .sum();
    println!(
        "\t Packets processed:                 {} packets",
        packets_processed
    );
    let packets_dropped: u32 = servers.iter().map(|server| server.packets_dropped()).sum();
    println!(
        "\t Packets dropped:                   {} packets",
        packets_dropped
    );
}
