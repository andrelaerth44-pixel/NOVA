# Contributing

NOVA is a real compiler/runtime project. Do not add simulated backends or fake APIs.

Run:
cargo build
cargo test
cargo run -- version

Every new language rule should have an executable test. Every advertised backend must actually compile for its target.
