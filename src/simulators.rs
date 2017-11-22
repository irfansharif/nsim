use std::collections::VecDeque;
use generators::Generator;
use rand::{thread_rng, Rng};
use bit_vec::BitVec;
use cbuffer::CircularBuffer;

// Packet holds the value of the time unit that it was generated at, its length, and the id of its destination node.
#[derive(Copy, Clone, PartialEq)]
pub struct Packet {
    pub time_generated: u32,
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
    packet_length: u32,
    generator: G,
    statistics: ClientStatistics,
}

impl<G: Generator> Client<G> {
    // Client::new seeds the ticker using the provided generator.
    pub fn new(generator: G, resolution: f64, packet_length: u32) -> Self {
        Client {
            resolution: resolution,
            ticker: generator.next_event(resolution),
            packet_length: packet_length,
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
    pub fn tick(&mut self, curr_time: u32) -> Option<Packet> {
        // TODO(irfansharif): Resolution mismatch; no possibility of generating multiple packets.
        if self.ticker == 0 {
            self.statistics.packets_generated += 1;
            self.ticker = self.generator.next_event(self.resolution);
            return Some(Packet {
                time_generated: curr_time,
                length: self.packet_length,
            });
        }

        self.ticker -= 1;
        if self.ticker == 0 {
            self.statistics.packets_generated += 1;
            self.ticker = self.generator.next_event(self.resolution);
            Some(Packet {
                time_generated: curr_time,
                length: self.packet_length,
            })
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

#[derive(PartialEq, Clone, Copy)]
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
    Waiting {
        counter: u32,
        wait_time: u32,
        current_packet: Packet,
    }
}

// Server stores packets in a queue and processes them.
pub struct Server<G: Generator> {
    id: usize,
    client: Client<G>,
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
}

impl<G: Generator> Server<G> {
    // Server::new returns a Server with the specified buffer limit, if any.
    pub fn new(
        resolution: f64,
        pspeed: f64,
        buffer_limit: Option<usize>,
        id: usize,
        client: Client<G>,
    ) -> Self {
        Server {
            id: id,
            client: client,
            queue: VecDeque::new(),
            buffer_limit: buffer_limit,
            resolution: resolution,
            statistics: ServerStatistics::new(),
            state: ServerState::Idle,
            pspeed: pspeed,
            currently_processing: None,
            bits_processed: 0.0,
            retries: 0,
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
    pub fn tick(
        &mut self,
        local_state: &mut BitVec,
        medium: &Medium,
        curr_tick: u32,
    ) -> Option<Packet> {
        if let Some(p) = self.client.tick(curr_tick) {
            self.enqueue(p);
        }
        match self.state {
            ServerState::Idle => {
                match self.queue.pop_front() {
                    Some(p) => ServerState::Sensing {
                        counter: 0,
                        busy: false,
                        current_packet: p,
                    },
                    None => ServerState::Idle,
                };
            }
            ServerState::Sensing {
                counter,
                busy,
                current_packet,
            } => {
                let counter = counter + 1;
                if counter == 96 && busy {
                    if self.retries > 10 {
                        self.state = match self.queue.pop_front() {
                            Some(p) => ServerState::Sensing {
                                counter: 0,
                                busy: false,
                                current_packet: p,
                            },
                            None => ServerState::Idle,
                        };
                    } else {
                        self.retries += 1;
                        let rand: u32 = thread_rng().gen_range(0, 2u32.pow(self.retries) - 1);
                        let wait_time: u32 = rand * 512;
                        self.state = ServerState::Waiting {
                            counter: 0,
                            wait_time: wait_time,
                            current_packet,
                        };
                    }
                } else if counter == 96 && !busy {
                    self.state = ServerState::Transmitting {
                        bits_processed: 0.0,
                        current_packet,
                    };
                    return None;
                } else {
                    self.state = ServerState::Sensing {
                        counter,
                        busy: medium.is_available(self.id),
                        current_packet,
                    };
                }
            }
            ServerState::Transmitting {
                mut bits_processed,
                current_packet,
            } => {
                if !medium.is_available(self.id) {
                    if self.retries > 10 {
                        self.state = match self.queue.pop_front() {
                            Some(p) => ServerState::Sensing {
                                counter: 0,
                                busy: false,
                                current_packet: p,
                            },
                            None => ServerState::Idle,
                        };
                    } else {
                        self.retries += 1;
                        let rand: u32 = thread_rng().gen_range(0, 2u32.pow(self.retries) - 1);
                        let wait_time: u32 = rand * 512;
                        self.state = ServerState::Waiting {
                            counter: 0,
                            wait_time: wait_time,
                            current_packet,
                        };
                    }
                }
                bits_processed += self.pspeed / self.resolution;
                local_state.set(self.id, true);
                if (bits_processed as u32) >= current_packet.length {
                    self.state = match self.queue.pop_front() {
                        Some(p) => ServerState::Sensing {
                            counter: 0,
                            busy: false,
                            current_packet: p,
                        },
                        None => ServerState::Idle,
                    };
                    self.statistics.packets_processed += 1;
                    return Some(current_packet);
                }
                self.state = ServerState::Transmitting {
                    bits_processed,
                    current_packet,
                };
            }
            ServerState::Waiting {
                counter,
                wait_time,
                current_packet,
            } => {
                if counter < wait_time {
                    let counter = counter + 1;
                    self.state = ServerState::Waiting {
                        counter,
                        wait_time,
                        current_packet,
                    };
                } else {
                    self.state = ServerState::Sensing {
                        counter: 0,
                        busy: false,
                        current_packet,
                    };
                }
            }
            _ => panic!("Invalid State"),
        }
        None
    }

    // Server.packets_processed returns the number of packets processed by the Server thus far.
    pub fn packets_processed(&self) -> u32 {
        self.statistics.packets_processed

    }
}

// Medium contains a circular buffer, with a bit vector of size n at each index
//
// The bit vectors represent the n possible writes that n nodes can perform at one time
pub struct Medium {
    track: CircularBuffer<BitVec>,
    num_nodes: usize,
}

impl Medium {
    pub fn new(n: usize, csize: usize) -> Medium {
        Medium {
            track: CircularBuffer::new(csize, BitVec::from_elem(n, false)),
            num_nodes: n,
        }
    }

    pub fn tick(&mut self) {
        self.track.advance();
    }

    // is_available returns true if other nodes have not written and false otherwise
    fn is_available(&self, node_id: usize) -> bool {
        assert!(node_id < self.num_nodes);
        let curr = self.track.read();
        let mut node_mask = BitVec::from_elem(self.num_nodes, false);
        node_mask.set(node_id, true);
        curr.none() || curr == node_mask
    }

    // write writes a new bitvec to the curret index of the track
    pub fn write(&mut self, state: BitVec) {
        assert!(state.len() == self.track.read().len());
        self.track.write(state);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use super::super::generators::Deterministic;

    #[test]
    fn client_packet_generation() {
        let mut c = Client::new(Deterministic::new(0.5), 1.0);
        assert!(!c.tick());
        assert!(c.tick());
    }

    #[test]
    fn server_packet_delivery() {
        let mut s = Server::new(1.0, 0.5, None);
        s.enqueue(Packet {
            time_generated: 0,
            length: 1,
        });
        s.enqueue(Packet {
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
    fn test_medium() {
        let num_nodes: usize = 8;
        let mut med = Medium::new(num_nodes, 2);
        med.write(BitVec::from_bytes(&[0b10010000]));
        assert!(!med.is_available(3));
        med.write(BitVec::from_bytes(&[0b00010000]));
        assert!(med.is_available(3));
        med.tick();
        assert!(med.is_available(1));
        med.tick();
        assert!(med.is_available(3));
    }
}
