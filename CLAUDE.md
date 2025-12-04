# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Build and Run Commands

```bash
cargo build          # Build debug version
cargo build --release  # Build release version
cargo run            # Run the application
cargo check          # Type-check without building
cargo clippy         # Run linter
cargo fmt            # Format code
```

## Project Overview

This is a Rust MQTT client that logs telemetry data from an EZPlugV2 (Tasmota) smart plug to a SQLite database.

## Architecture

The application runs an async event loop using tokio:
1. Connects to an MQTT broker and subscribes to telemetry topics (`tele/EZPlugV2_743EEC/STATE` and `tele/EZPlugV2_743EEC/SENSOR`)
2. Parses incoming JSON messages into typed structs (`TeleState`, `TeleSensor`)
3. Persists data to SQLite tables (`state` and `sensor`)

Key components:
- **Tracing**: Uses `tracing-subscriber` with a reloadable filter - log levels can be changed at runtime by editing `log_config.txt`
- **File Watcher**: A `notify` watcher monitors `log_config.txt` for changes to dynamically adjust logging
- **Database**: SQLx with SQLite, auto-creates tables on startup

## Configuration

- Broker IP and port are constants at the top of `main.rs` (`BROKER_IP`, `BROKER_PORT`, `TOPIC_BASE`)
- Telemetry period is configurable via `TELE_PERIOD` constant (seconds)
- Runtime log level: Create/edit `log_config.txt` with a tracing filter directive (e.g., `debug`, `info`, `ezv2_t1=debug`)

## Dependencies

- `rumqttc` - Async MQTT client
- `sqlx` - Async SQLite with runtime-tokio-rustls
- `tokio` - Async runtime
- `serde`/`serde_json` - JSON deserialization
- `tracing`/`tracing-subscriber` - Structured logging
- `notify` - File system watcher
