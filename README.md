# Midimaxe

MIDI master clock for the synthie jam session

[![asciicast](https://asciinema.org/a/EvLaL88JvkaVTNHCR4e7L5J2I.svg)](https://asciinema.org/a/EvLaL88JvkaVTNHCR4e7L5J2I)

## Introduction

Midimaxe is a MIDI master clock for multiple MIDI devices. Clients can join and leave during a session. Clients joining late will be started precisely at a multiple of a definable Quantum (usually 1, 4, 8, or 16 bars).

It also comes with a beat and quantum display that can be put on a screen in the session room. No one has an excuse not to play on the correct beat now.

## Motivation

I regularly join a synthesizer jam session where I used my DAW (Bitwig Studio) to synchronize all the hardware synths and laptops. This turned out problematic sometimes. Loading a particularly CPU intensive patch could desync the whole session. People join and leave during the session, cables get janked out, people press the wrong button on their device, usually requiring a full restart to get everything back in sync. I wanted a sync solution that could:

* Provide a synchronized MIDI clock to many connected USB MIDI devices
* Allow devices to join and leave during the session
* Joining devices do so at a specified quantum (e.g. 4, 8, 16 bars)

This will allow resuming single devices that lost synchronization without stopping the whole session.

## Building

Install Rust by your preferred means. Then build as usual:

```
cargo build --release
cargo run --bin midimaxe --release
cargo run --bin sync_checker --release
```

