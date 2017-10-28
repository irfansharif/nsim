extern crate qlib;
extern crate getopts;
extern crate stats;

use getopts::Options;
use qlib::generators::*;
use qlib::simulators::*;
use stats::OnlineStats;
use std::env;

const DEFAULT_RATE: u32 = 10_000;
const DEFAULT_PSIZE: u32 = 1;
const DEFAULT_PSPEED: u32 = 10_000;
const DEFAULT_DURATION: u32 = 5;
const DEFAULT_QLIMIT: Option<usize> = None;

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
        "pspeed",
        &format!("Packet processing speed; bits/s (def: {})", DEFAULT_PSPEED),
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
        "qlimit",
        &format!(
            "Limit on of the buffer queue length; int (def: {:?})",
            DEFAULT_QLIMIT
        ),
        "NUM",
    );
    opts
}

fn parse_params(matches: &getopts::Matches) -> (u32, u32, u32, u32, Option<usize>) {
    let rate = match matches.opt_str("rate") {
        Some(x) => x.parse::<u32>().unwrap(),
        None => DEFAULT_RATE,
    };
    let psize = match matches.opt_str("psize") {
        Some(x) => x.parse::<u32>().unwrap(),
        None => DEFAULT_PSIZE,
    };
    let pspeed = match matches.opt_str("pspeed") {
        Some(x) => x.parse::<u32>().unwrap(),
        None => DEFAULT_PSPEED,
    };
    let duration = match matches.opt_str("duration") {
        Some(x) => x.parse::<u32>().unwrap(),
        None => DEFAULT_DURATION,
    };
    let qlimit = match matches.opt_str("qlimit") {
        Some(x) => Some(x.parse::<u32>().unwrap() as usize),
        None => DEFAULT_QLIMIT,
    };

    (rate, psize, pspeed, duration, qlimit)
}

fn print_usage(program: &str, opts: &Options) {
    let brief = format!("Usage: {} [options]", program);
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
            std::process::exit(1)
        }
    };

    if matches.opt_present("h") {
        print_usage(&program, &opts);
        return;
    }

    let resolution = 1e6;
    let (rate, psize, pspeed, duration, qlimit) = parse_params(&matches);

    println!("Simulation configuration:");
    println!("\t Rate:                  {} packets/s", rate);
    println!("\t Packet size:           {} bits", psize);
    println!("\t Server speed:          {} bits/s", pspeed);
    println!("\t Simulation time:       {}s", duration);
    println!("\t Resolution:            1µs");
    println!("\t Queue size limit:      {:?}", qlimit);
    println!(
        "\t Ticks per packet:      {}",
        f64::from(psize) / f64::from(pspeed) * resolution
    );
    println!();

    let ticks = duration * resolution as u32;

    let mut client = Client::new(Markov::new(f64::from(rate)), resolution);
    let mut server = Server::new(resolution, f64::from(pspeed), qlimit);
    let mut pstats = OnlineStats::new();
    let mut qstats = OnlineStats::new();

    for i in 0..ticks {
        qstats.add(server.qlen());

        if client.tick() {
            server.enqueue(Packet {
                time_generated: i,
                length: psize,
            });
        }
        if let Some(p) = server.tick() {
            // We record the time it took for the processed packet to get processed.
            pstats.add(f64::from(i - p.time_generated) / resolution);
        }
    }

    println!("Simulation results:");
    println!(
        "\t Average sojourn time:              {:.4} +/- {:.4} seconds",
        pstats.mean(),
        pstats.stddev()
    );
    println!(
        "\t Average # of queued packets:       {:.2} +/- {:.2} packets",
        qstats.mean(),
        qstats.stddev()
    );
    println!(
        "\t Packets generated:                 {} packets",
        client.packets_generated()
    );
    println!(
        "\t Packets processed:                 {} packets",
        server.packets_processed()
    );
    println!(
        "\t Packets droppped:                  {} packets",
        server.packets_dropped()
    );
    println!(
        "\t Packet loss probability:           {:.2}%",
        f64::from(server.packets_dropped()) / f64::from(client.packets_generated()) * 100.0
    );
    println!(
        "\t Server idle proportion:            {:.2}%",
        server.idle_proportion()
    );
    println!("\t Packets leftover in queue:         {}", server.qlen());
}
