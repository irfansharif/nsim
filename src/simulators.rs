use std::collections::VecDeque;
use generators::Generator;
use rand::{thread_rng, Rng};
use bit_vec::BitVec;
use cbuffer::CircularBuffer;

// Packet holds the value of the time unit that it was generated at and its length.
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct Packet {
    pub time_generated: u32,
    pub length: u32,
}

// Client generates packets according as per the parametrized generators::Generator. We maintain a
// ticker count to the next time a packet is to be generated, moving forward at ticks of the
// specified resolution.
pub struct Client<G: Generator> {
    resolution: f64,
    ticker: u32,
    packet_length: u32,
    generator: G,
}

impl<G: Generator> Client<G> {
    // Client::new seeds the ticker using the provided generator.
    pub fn new(generator: G, resolution: f64, packet_length: u32) -> Self {
        Client {
            resolution: resolution,
            ticker: generator.next_event(resolution),
            packet_length: packet_length,
            generator: generator,
        }
    }

    // The caller is responsible for calling Client.tick() at fixed time intervals, moving the
    // Client simulator one time unit per call. We return a Option<Packet> indicating whether or
    // not a packet is generated in the most recently completed time unit.
    //
    // We're careful to check if self.ticker == 0 before decrementing because the parametrized
    // generator may very well return 0 (see top-level comment in src/generators.rs).
    pub fn tick(&mut self, current_time: u32) -> Option<Packet> {
        // TODO(irfansharif): Resolution mismatch; no possibility of generating multiple packets.
        if self.ticker == 0 {
            self.ticker = self.generator.next_event(self.resolution);
            return Some(Packet {
                time_generated: current_time,
                length: self.packet_length,
            });
        }

        self.ticker -= 1;
        if self.ticker == 0 {
            self.ticker = self.generator.next_event(self.resolution);
            Some(Packet {
                time_generated: current_time,
                length: self.packet_length,
            })
        } else {
            None
        }
    }
}

// ServerStatistics is the set of statistics we care about post-simulation as far as the Server is
// concerned.
pub struct ServerStatistics {
    pub packets_processed: u32,
    pub packets_generated: u32,
    pub packets_dropped: u32,
}

impl ServerStatistics {
    fn new() -> ServerStatistics {
        ServerStatistics {
            packets_processed: 0,
            packets_generated: 0,
            packets_dropped: 0,
        }
    }
}

#[derive(PartialEq, Clone, Copy, Debug)]
enum ServerState {
    Idle,
    Sensing {
        counter: u32,
        busy: bool,
        current_packet: Packet,
    },
    Transmitting {
        bits_processed: f64,
        current_packet: Packet,
    },
    Jamming {
        counter: u32,
        current_packet: Packet,
    },
    Waiting {
        counter: u32,
        wait_time: u32,
        current_packet: Packet,
    },
}

// Server stores packets in a queue and processes them.
pub struct Server<G: Generator> {
    id: usize,
    client: Client<G>,
    queue: VecDeque<Packet>,
    resolution: f64,
    statistics: ServerStatistics,
    state: ServerState,
    persistence: bool,
    // Processing variables
    pspeed: f64,
    retries: u32,
}

impl<G: Generator> Server<G> {
    // Server::new returns a Server.
    pub fn new(
        id: usize,
        generator: G,
        packet_length: u32,
        resolution: f64,
        pspeed: f64,
        persistence: bool,
    ) -> Self {
        Server {
            id: id,
            client: Client::new(generator, resolution, packet_length),
            queue: VecDeque::new(),
            resolution: resolution,
            statistics: ServerStatistics::new(),
            state: ServerState::Idle,
            pspeed: pspeed,
            retries: 0,
            persistence: persistence,
        }
    }

    // Server.enqueue enqueues a packet for delivery. If the packet is to be dropped (due to the
    // internal queue being full it is recorded in the Server's internal statistics.
    pub fn enqueue(&mut self, packet: Packet) {
        // Infinite queue, limit == None.
        self.queue.push_back(packet);
    }

    // Server.tick checks to see if a packet is currently being processed, and if so,
    // increments Server.bits_processed, and if the resulting sum is equal to the bits
    // in the packet, then it returns the packet and resets the state of Server.
    pub fn tick(
        &mut self,
        local_state: &mut BitVec,
        medium: &Medium,
        current_time: u32,
    ) -> Option<Packet> {
        if let Some(packet) = self.client.tick(current_time) {
            self.statistics.packets_generated += 1;
            self.enqueue(packet);
        }
        loop {
            match self.state {
                ServerState::Idle => {
                    match self.queue.pop_front() {
                        Some(packet) => {
                            self.state = ServerState::Sensing {
                                counter: 0,
                                busy: false,
                                current_packet: packet,
                            }
                        }
                        None => {
                            self.state = ServerState::Idle;
                            break;
                        }
                    };
                }
                ServerState::Sensing {
                    counter,
                    busy,
                    current_packet,
                } => {
                    // TODO(irfansharif): Factor in resolution.
                    if counter < 96 {
                        self.state = ServerState::Sensing {
                            counter: counter + 1,
                            busy: medium.is_busy(self.id) || busy,
                            current_packet: current_packet,
                        };
                        break;
                    } else if busy {
                        assert!(counter == 96);

                        self.retries += 1;
                        if self.retries > 10 {
                            self.state = ServerState::Idle;
                            self.statistics.packets_dropped += 1;
                        } else {
                            // TODO(irfansharif): Factor in resolution.
                            let mut wait_time: u32 =
                                thread_rng().gen_range(0, 2u32.pow(self.retries) - 1) * 512;
                            if self.persistence {
                                // Persistent mode, wait_time == 0.
                                wait_time = 0;
                            }
                            self.state = ServerState::Waiting {
                                counter: 0,
                                wait_time: wait_time,
                                current_packet,
                            };
                        }
                    } else {
                        assert!(counter == 96);

                        self.state = ServerState::Transmitting {
                            bits_processed: 0.0,
                            current_packet,
                        };
                    }
                }
                ServerState::Transmitting {
                    bits_processed,
                    current_packet,
                } => {
                    if !medium.is_busy(self.id) {
                        let bits_processed = bits_processed + (self.pspeed / self.resolution);
                        local_state.set(self.id, true);
                        if (bits_processed as u32) >= current_packet.length {
                            self.statistics.packets_processed += 1;
                            self.state = ServerState::Idle;
                            return Some(current_packet);
                        }
                        self.state = ServerState::Transmitting {
                            bits_processed,
                            current_packet,
                        };
                        break;
                    } else {
                        self.state = ServerState::Jamming {
                            counter: 48,
                            current_packet,
                        }
                    }
                },
                ServerState::Jamming {
                    mut counter,
                    current_packet,
                } => {
                    counter -= 1;
                    if counter == 0 {
                        self.retries += 1;
                        if self.retries > 10 {
                            self.state = ServerState::Idle;
                            self.statistics.packets_dropped += 1;
                        } else {
                            let wait_time: u32 =
                                thread_rng().gen_range(0, 2u32.pow(self.retries) - 1) * 512;
                            self.state = ServerState::Waiting {
                                counter: 0,
                                wait_time: wait_time,
                                current_packet,
                            };
                        }
                    } else {
                        self.state = ServerState::Jamming {
                            counter,
                            current_packet,
                        }
                    }
                },
                ServerState::Waiting {
                    counter,
                    wait_time,
                    current_packet,
                } => {
                    if counter < wait_time {
                        self.state = ServerState::Waiting {
                            counter: counter + 1,
                            wait_time: wait_time,
                            current_packet: current_packet,
                        };
                        break;
                    } else {
                        self.state = ServerState::Sensing {
                            counter: 0,
                            busy: false,
                            current_packet,
                        };
                    }
                }
            }
        }
        None
    }

    // Server.packets_processed returns the number of packets processed by the Server thus far.
    pub fn packets_processed(&self) -> u32 {
        self.statistics.packets_processed
    }

    // Server.packets_generated returns the number of packets generated by the Server thus far.
    pub fn packets_generated(&self) -> u32 {
        self.statistics.packets_generated
    }

    // Server.packets_dropped returns the number of packets cropped by the Server thus far.
    pub fn packets_dropped(&self) -> u32 {
        self.statistics.packets_dropped
    }
}

// Medium contains a circular buffer, with a bit vector of size n at each index
//
// The bit vectors represent the n possible writes that n nodes can perform at one time
pub struct Medium {
    tracks: CircularBuffer<BitVec>,
    num_nodes: usize,
}

impl Medium {
    pub fn new(num_nodes: usize, bsize: usize) -> Medium {
        Medium {
            tracks: CircularBuffer::new(bsize, BitVec::from_elem(num_nodes, false)),
            num_nodes: num_nodes,
        }
    }

    pub fn tick(&mut self) {
        self.tracks.tick();
    }

    fn is_busy(&self, id: usize) -> bool {
        assert!(id < self.num_nodes);
        let mut mask = BitVec::from_elem(self.num_nodes, true);
        mask.set(id, false);
        mask.intersect(&self.tracks.read());
        mask.any()
    }

    pub fn write(&mut self, state: BitVec) {
        assert!(state.len() == self.tracks.read().len());
        self.tracks.write(state);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use super::super::generators::Deterministic;

    #[test]
    fn client_packet_generation() {
        let mut c = Client::new(Deterministic::new(0.5), 1.0, 1);
        assert!(c.tick(0).is_none());
        assert!(
            c.tick(1).unwrap() ==
                Packet {
                    time_generated: 1,
                    length: 1,
                }
        );
    }

    #[test]
    fn test_medium() {
        let num_nodes: usize = 8;
        let mut med = Medium::new(num_nodes, 2);

        med.write(BitVec::from_bytes(&[0b10000000]));
        assert!(!med.is_busy(0));
        assert!(med.is_busy(1));

        med.write(BitVec::from_bytes(&[0b01000000]));
        assert!(med.is_busy(0));
        assert!(!med.is_busy(1));

        med.write(BitVec::from_bytes(&[0b11000000]));
        assert!(med.is_busy(0));
        assert!(med.is_busy(1));

        med.write(BitVec::from_bytes(&[0b00000000]));
        assert!(!med.is_busy(0));
        assert!(!med.is_busy(1));

        med.tick();
        assert!(!med.is_busy(0));
        assert!(!med.is_busy(1));
        med.write(BitVec::from_bytes(&[0b01000000]));
        assert!(med.is_busy(0));
        assert!(!med.is_busy(1));

        med.tick();
        assert!(!med.is_busy(0));
        assert!(!med.is_busy(1));
        med.write(BitVec::from_bytes(&[0b11000000]));
        assert!(med.is_busy(0));
        assert!(med.is_busy(1));

        med.tick();
        assert!(med.is_busy(0));
        assert!(!med.is_busy(1));

        med.tick();
        assert!(med.is_busy(0));
        assert!(med.is_busy(1));
    }

    #[test]
    fn server_idle_to_sensing() {
        let medium = Medium::new(1, 1);
        let mut server = Server::new(
            0, // id
            Deterministic::new(0.5), // generator
            1, // psize
            1.0, // resolution
            1.0, // lspeed
            false, // persistence
        );
        let mut state = BitVec::from_elem(1, false);
        server.tick(&mut state, &medium, 1);
        assert!(server.state == ServerState::Idle);
        server.tick(&mut state, &medium, 1);
        assert!(
            server.state ==
                ServerState::Sensing {
                    counter: 1,
                    busy: false,
                    current_packet: Packet {
                        time_generated: 1,
                        length: 1,
                    },
                }
        );
        server.tick(&mut state, &medium, 2);
        assert!(
            server.state ==
                ServerState::Sensing {
                    counter: 2,
                    busy: false,
                    current_packet: Packet {
                        time_generated: 1,
                        length: 1,
                    },
                }
        );
    }

    #[test]
    fn server_busy_medium() {
        let mut medium = Medium::new(2, 1);
        medium.write(BitVec::from_elem(2, true));
        medium.tick();
        let mut server = Server::new(
            0, // id
            Deterministic::new(0.5), // generator
            1, // psize
            1.0, // resolution
            1.0, // lspeed
            false, // persistence
        );
        let mut state = BitVec::from_elem(2, true);
        server.tick(&mut state, &medium, 1);
        assert!(server.state == ServerState::Idle);
        server.tick(&mut state, &medium, 1);
        assert!(
            server.state ==
                ServerState::Sensing {
                    counter: 1,
                    busy: true,
                    current_packet: Packet {
                        time_generated: 1,
                        length: 1,
                    },
                }
        );
    }

    #[test]
    fn server_sensing_to_transmitting() {
        let mut medium = Medium::new(2, 1);
        let mut server = Server::new(
            0, // id
            Deterministic::new(0.5), // generator
            2, // psize
            1.0, // resolution
            1.0, // lspeed
            false, // persistence
        );
        let mut state = BitVec::from_elem(2, false);
        server.tick(&mut state, &medium, 1);
        assert!(server.state == ServerState::Idle);
        server.tick(&mut state, &medium, 2);
        assert!(
            server.state ==
                ServerState::Sensing {
                    counter: 1,
                    busy: false,
                    current_packet: Packet {
                        time_generated: 2,
                        length: 2,
                    },
                }
        );
        medium.write(BitVec::from_elem(2, false));
        server.state = ServerState::Sensing {
            counter: 96,
            busy: false,
            current_packet: Packet {
                time_generated: 2,
                length: 2,
            },
        };
        server.tick(&mut state, &medium, 3);
        assert!(
            server.state ==
                ServerState::Transmitting {
                    bits_processed: 1.0,
                    current_packet: Packet {
                        time_generated: 2,
                        length: 2,
                    },
                }
        );
    }
}
