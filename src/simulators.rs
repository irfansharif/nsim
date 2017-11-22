extern crate rand;

use std::collections::VecDeque;
use generators::{Generator, Markov};
use self::rand::{thread_rng, Rng};
use cbuffer;

// Packet holds the value of the time unit that it was generated at, and its length.
#[derive(Clone)]
pub struct Packet {
    pub time_generated: u32,
    pub destination_id: u32,
    pub length: u32,
}

// ClientStatistics is the set of statistics we care about post-simulation as far as the Client is
// concerned.
pub struct ClientStatistics {
    pub packets_generated: u32,
}

impl ClientStatistics {
    fn new() -> ClientStatistics {
        ClientStatistics { packets_generated: 0 }
    }
}

// Client generates packets according as per the parametrized generators::Generator. We maintain a
// ticker count to the next time a packet is to be generated, moving forward at ticks of the
// specified resolution. We also collect Client statistics through this progression.
pub struct Client<G: Generator> {
    resolution: f64,
    ticker: u32,
    peers: Vec<u32>,
    generator: G,
    statistics: ClientStatistics,
}

impl<G: Generator> Client<G> {
    // Client::new seeds the ticker using the provided generator.
    pub fn new(generator: G, resolution: f64, peers: Vec<u32>) -> Client<G> {
        assert!(peers.len() != 0);
        Client {
            resolution: resolution,
            ticker: generator.next_event(resolution),
            peers: peers,
            generator: generator,
            statistics: ClientStatistics::new(),
        }
    }

    // The caller is responsible for calling Client.tick() at fixed time intervals, moving the
    // Client simulator one time unit per call. We return a boolean indicating whether or not a
    // packet is generated in the most recently completed time unit.
    //
    // We're careful to check if self.ticker == 0 before decrementing because the parametrized
    // generator may very well return 0 (see top-level comment in src/generators.rs).
    pub fn tick(&mut self) -> Option<u32> {
        // TODO(irfansharif): Resolution mismatch; no possibility of generating multiple packets.
        if self.ticker == 0 {
            self.statistics.packets_generated += 1;
            self.ticker = self.generator.next_event(self.resolution);
            return Some(thread_rng().choose(&self.peers).unwrap().to_owned());
        }

        self.ticker -= 1;
        if self.ticker == 0 {
            self.statistics.packets_generated += 1;
            self.ticker = self.generator.next_event(self.resolution);
            Some(thread_rng().choose(&self.peers).unwrap().to_owned())
        } else {
            None
        }
    }

    // Client.packets_generated returns the number of packets generated by the Client thus far.
    pub fn packets_generated(&self) -> u32 {
        self.statistics.packets_generated
    }
}

// ServerStatistics is the set of statistics we care about post-simulation as far as the Server is
// concerned.
pub struct ServerStatistics {
    pub packets_processed: u32,
}

impl ServerStatistics {
    fn new() -> ServerStatistics {
        ServerStatistics { packets_processed: 0 }
    }
}

enum ServerState {
    Idle,
    Sensing {counter: u32, busy: bool},
    Transmitting,
    Waiting {counter: u32, wait_time: u32},
}

// Server stores packets in a queue and processes them.
pub struct Server {
    queue: VecDeque<Packet>,
    buffer_limit: Option<usize>,
    resolution: f64,
    pub statistics: ServerStatistics,
    state: ServerState,
    // Processing variables
    pspeed: f64,
    currently_processing: Option<Packet>,
    bits_processed: f64,
    retries: u32,
    total_delay: f64,
}

impl Server {
    // Server::new returns a Server with the specified buffer limit, if any.
    pub fn new(resolution: f64, pspeed: f64, buffer_limit: Option<usize>) -> Server {
        Server {
            queue: VecDeque::new(),
            buffer_limit: buffer_limit,
            resolution: resolution,
            statistics: ServerStatistics::new(),
            state: ServerState::Idle,
            pspeed: pspeed,
            currently_processing: None,
            bits_processed: 0.0,
            retries: 0,
            total_delay: 0.0,
        }
    }

    // Server.enqueue enqueues a packet for delivery. If the packet is to be dropped (due to the
    // internal queue being full it is recorded in the Server's internal statistics.
    pub fn enqueue(&mut self, packet: Packet) {
        match self.buffer_limit {
            Some(limit) => {
                if self.queue.len() < limit {
                    self.queue.push_back(packet);
                }
            }
            // Infinite queue, limit == None.
            None => {
                self.queue.push_back(packet);
            }
        }
    }

    // Server.tick checks to see if a packet is currently being processed, and if so,
    // increments Server.bits_processed, and if the resulting sum is equal to the bits
    // in the packet, then it returns the packet and resets the state of Server.
    pub fn tick(&mut self) -> Option<Packet> {
        match self.currently_processing.clone() {
            // TODO(irfansharif): CSMA/CD FSM.
            Some(p) => {
                self.state = match self.state {
                    ServerState::Sensing{counter, busy} => {
                        let counter = counter + 1;
                        if counter == 96 && busy {
                            if self.retries > 10 {
                                //TODO: some sort of error
                                ServerState::Idle
                            } else {
                                self.retries += 1;
                                let rand: u32 = thread_rng().gen_range(0, 2u32.pow(self.retries)-1);
                                let wait_time: u32 = rand * 512/10;
                                ServerState::Waiting{counter: 0, wait_time:wait_time}
                            }
                        } else if counter == 96 && !busy {
                            ServerState::Transmitting
                        } else {
                            let busy = false || busy;
                            ServerState::Sensing{counter, busy}
                        }
                    }
                    ServerState::Transmitting => {
                        self.bits_processed += self.pspeed / self.resolution;
                        if (self.bits_processed as u32) < p.length {
                            //TODO: code for checking collision

                            //if collision
                            if self.retries > 10 {
                                //TODO: some sort of error
                                self.bits_processed = 0.0;
                                self.state = ServerState::Idle;
                                return None;
                            } else {
                                self.retries += 1;
                                let rand: u32 = thread_rng().gen_range(0, 2u32.pow(self.retries)-1);
                                let wait_time: u32 = rand * 512/10;
                                self.state = ServerState::Waiting{counter: 0, wait_time: wait_time};
                                return None;
                            }
                        }
                        self.currently_processing = None;
                        self.bits_processed = 0.0;
                        self.statistics.packets_processed += 1;
                        ServerState::Transmitting
                    }
                    ServerState::Waiting{counter, wait_time} => {
                        if counter < wait_time {
                            let counter = counter + 1;
                            ServerState::Waiting{counter, wait_time}
                        } else {
                            ServerState::Sensing{counter: 0, busy: false}
                        }
                    }

                    _ => panic!("Invaild State")
                };
                self.total_delay += 1.0;
                Some(p)
            }
            None => {
                match self.queue.pop_front() {
                    Some(p) => {
                        self.retries = 0;
                        self.total_delay = 0.0;
                        self.state = match self.state {
                            ServerState::Idle => {
                                ServerState::Sensing{counter:0, busy:false}
                            }
                            _ => panic!("Invaild State")
                        };
                        Some(p)
                    }
                    None => None,
                }
            }
        }
    }

    // Server.packets_processed returns the number of packets processed by the Server thus far.
    pub fn packets_processed(&self) -> u32 {
        self.statistics.packets_processed

    }
}

pub struct Medium {
    tracks: Vec<cbuffer::CircularBuffer<Packet>>,
}

impl Medium {
    fn new(ntracks: usize, csize: usize) -> Medium {
        Medium {
            tracks: (0..ntracks)
                .map(|_| cbuffer::CircularBuffer::new(csize))
                .collect(),
        }
    }

    fn tick(&mut self) {
        for t in self.tracks.iter_mut() {
            t.advance()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use super::super::generators::Deterministic;

    #[test]
    fn Client_packet_generation() {
        let mut c = Client::new(Deterministic::new(0.5), 1.0, vec![1]);
        assert!(c.tick().is_none());
        assert!(c.tick().is_some());
    }

    #[test]
    fn Server_packet_delivery() {
        let mut s = Server::new(1.0, 0.5, None);
        s.enqueue(Packet {
            destination_id: 0,
            time_generated: 0,
            length: 1,
        });
        s.enqueue(Packet {
            destination_id: 0,
            time_generated: 0,
            length: 1,
        });
        s.tick();
        assert_eq!(s.statistics.packets_processed, 0);

        s.tick();
        assert_eq!(s.statistics.packets_processed, 1);

        s.tick();
        assert_eq!(s.statistics.packets_processed, 1);

        s.tick();
        assert_eq!(s.statistics.packets_processed, 2);
    }

    #[test]
    fn Server_packet_dropped() {
        let mut s = Server::new(1.0, 1.0, Some(1));
        s.enqueue(Packet {
            destination_id: 0,
            time_generated: 0,
            length: 1,
        });
        s.enqueue(Packet {
            destination_id: 0,
            time_generated: 0,
            length: 1,
        });

        s.tick();
        assert_eq!(s.statistics.packets_processed, 1);
        assert_eq!(s.statistics.packets_dropped, 1);
    }

    #[test]
    fn Server_idle_count() {
        let mut s = Server::new(1.0, 1.0, Some(1));

        s.tick();
        assert_eq!(s.statistics.idle_count, 1);

        s.tick();
        assert_eq!(s.statistics.idle_count, 2);

        s.enqueue(Packet {
            destination_id: 0,
            time_generated: 0,
            length: 1,
        });
        s.tick();
        assert_eq!(s.statistics.idle_count, 2);
        assert_eq!(s.statistics.packets_processed, 1);
    }
}
