use rand::distributions::{Exp, IndependentSample};
use rand::thread_rng;

// Generators generate events, the generation of which is dictated by which specific Generator is
// used. The underlying RNG distribution, if configured (consider λ in an exponentially distributed
// generator for e.g.), should map to an events/s parameter.
pub trait Generator {
    // next_event returns an u32 integer corresponding to how many discrete time units of the
    // specified resolution (1e6 for a µs scale for e.g.) would need to pass until
    // the next such event.
    //
    // NB: If the resolution is too course (1 for e.g. corresponding to a 1s resolution), the
    // return value might be 0, this just means we've potentially lost useful information due to
    // rounding up errors. If the next event was to occur after 5ms, a specified resolution of a 1s
    // scale (asking for the next second the event would occur) would return 0 -- hardly useful
    // information.
    fn next_event(&self, resolution: f64) -> u32;
}

// generators::Markov generates events where the interarrival time between subsequent events is
// dictated by an exponential distribution.
pub struct Markov {
    exp: Exp,
}

impl Markov {
    pub fn new(lambda: f64) -> Self {
        Markov { exp: Exp::new(lambda) }
    }
}

impl Generator for Markov {
    fn next_event(&self, resolution: f64) -> u32 {
        (self.exp.ind_sample(&mut thread_rng()) * resolution) as u32
    }
}

pub struct Deterministic {
    rate: f64,
}

impl Deterministic {
    pub fn new(rate: f64) -> Self {
        Deterministic { rate: rate }
    }
}

impl Generator for Deterministic {
    fn next_event(&self, resolution: f64) -> u32 {
        (resolution / self.rate) as u32
    }
}


#[cfg(test)]
mod tests {
    use super::{Generator, Markov, Deterministic};

    // Use `cargo test -- --nocapture` to verify the generation of exponentially distributed random
    // u32 integers, at 100 packets/s and a µs scale resolution, a typical generation would be
    // [8728, 12561, 4670, 5370, 9221].
    #[test]
    fn generate_markovian_events() {
        let mg = Markov::new(100.0);
        let mut events = vec![];
        for _ in 0..5 {
            events.push(mg.next_event(1e6));
        }
        println!("event deltas: {:?}", events)
    }

    #[test]
    fn generate_deterministic_events() {
        let dg = Deterministic::new(1000.0);
        let mut events = vec![];
        for _ in 0..5 {
            events.push(dg.next_event(1e6));
        }
        assert_eq!(events, vec![1000; 5]);
    }
}
