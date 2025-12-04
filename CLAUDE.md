# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Build and Run Commands

```bash
cargo build                      # Build all crates (debug)
cargo build --release            # Build all crates (release)
cargo run -p collector           # Run the MQTT collector
cargo run -p viewer -- <db> <out.png> [start] [count]  # Run the data viewer
cargo check                      # Type-check without building
cargo clippy                     # Run linter
cargo fmt                        # Format code
```

## Project Overview

This is a Rust workspace for logging and visualizing telemetry data from an EZPlugV2 (Tasmota) smart plug.

## Workspace Structure

```
crates/
├── collector/   # MQTT client that logs telemetry to SQLite
├── viewer/      # CLI tool for plotting power data to PNG
├── database/    # Shared database models and operations (lib)
└── config/      # Shared TOML configuration loading (lib)
```

### Crates

- **collector**: Main application - connects to MQTT broker, subscribes to telemetry topics, persists data to SQLite
- **viewer**: Generates PNG charts from sensor data. Usage: `viewer <db_file> <output.png> [start] [count]`
- **database**: Shared library with SQLx models (`TeleState`, `TeleSensor`) and database operations
- **config**: Shared library for loading TOML configuration (`AppConfig`, `MqttConfig`, etc.)

## Architecture

The collector runs an async event loop using tokio:
1. Loads configuration from `ezv2-config.toml`
2. Connects to an MQTT broker and subscribes to telemetry topics (`tele/{topic_base}/STATE` and `tele/{topic_base}/SENSOR`)
3. Parses incoming JSON messages into typed structs
4. Persists data to SQLite tables (`state` and `sensor`)

Key components:
- **Tracing**: Uses `tracing-subscriber` with a reloadable filter - log levels can be changed at runtime
- **File Watcher**: A `notify` watcher monitors the log config file for dynamic log level changes
- **Database**: SQLx with SQLite, auto-creates tables on startup

## Configuration

Configuration is loaded from `ezv2-config.toml`:

```toml
[mqtt]
broker_ip = "192.168.1.195"
broker_port = 1883
topic_base = "EZPlugV2_743EEC"
client_id = "ezplugv2_sqlite_logger_dev"
tele_period = 10  # seconds (10..3600)

[database]
filename = "ezplug.db"

[logging]
config_file = "log_config.txt"  # Dynamic log level file
```

Runtime log level: Edit the file specified in `logging.config_file` with a tracing filter directive (e.g., `debug`, `info`, `collector=debug`)

## Key Dependencies

- `rumqttc` - Async MQTT client (collector)
- `sqlx` - Async SQLite with runtime-tokio-rustls (collector, viewer, database)
- `tokio` - Async runtime
- `serde`/`toml` - Configuration parsing (config)
- `tracing`/`tracing-subscriber` - Structured logging (collector)
- `notify` - File system watcher for dynamic config (collector)
- `plotters` - Chart generation (viewer)
