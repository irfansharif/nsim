## nsim: CSMA/CD protocol simulator, LAN

```sh
~ cargo build --release; cargo run --release -- --help
Usage: target/release/nsim [Options]

Options:
    -h, --help          Display this message
        --rate NUM      Average number of generated packets/s (def: 10)
        --psize NUM     Packet size; bits (def: 1)
        --lspeed NUM    LAN speed in terms of bits read from/written to
                        network links; bits/s (def: 1000000)
        --duration NUM  Duration of simulation; seconds (def: 5)
        --ncount NUM    Number of nodes connected to the LAN (def: 10)
        --persistence   Simulate 1-persistent CSMA/CD protocol (def: false)
```
