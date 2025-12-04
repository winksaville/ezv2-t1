# ezv2-t1

An MQTT client for logging telemetry data from an EZPlugV2 (Tasmota) smart plug to a SQLite database.

Created with:
- ChatGPT 5.1: Initial implementation
  https://chatgpt.com/share/6930bbf3-a5a8-800c-a89d-3719a0a5f58a
- Claude Code: Added dynamic log level configuration, refactored into functions, and documentation

Note: The `.claude` directory in this repo is symlinked from `~/.claude/projects/-home-wink-data-prgs-mqtt-ezv2-t1/`
to preserve Claude conversation history within the repository.

## What It Does

This application connects to an MQTT broker and subscribes to telemetry topics from a Tasmota-flashed EZPlugV2 smart plug. It captures two types of messages:

- **STATE**: Device status including uptime, heap memory, WiFi signal strength, and power relay state
- **SENSOR**: Energy readings including voltage, current, power (W), power factor, and cumulative energy usage (kWh)

All data is stored in a local SQLite database (`ezplug.db`) with separate tables for state and sensor data.

## Building

```bash
cargo build            # Debug build
cargo build --release  # Release build
```

## Running

```bash
cargo run
```

Or run the built binary directly:

```bash
./target/release/ezv2-t1
```

The application will:
1. Create `ezplug.db` if it doesn't exist
2. Connect to the MQTT broker at 192.168.1.195:1883
3. Subscribe to telemetry topics and begin logging data

## Configuration

The broker IP, port, and device topic are configured as constants in `src/main.rs`:

```rust
const BROKER_IP: &str = "192.168.1.195";
const BROKER_PORT: u16 = 1883;
const TOPIC_BASE: &str = "EZPlugV2_743EEC";
```

## Dynamic Log Levels

Log levels can be changed at runtime without restarting the application by editing `log_config.txt` in the working directory.

### Setting a Log Level

Create or edit `log_config.txt` with a tracing filter directive:

```bash
# Simple levels (applies to all modules)
echo "debug" > log_config.txt
echo "info" > log_config.txt
echo "warn" > log_config.txt

# Module-specific levels
echo "ezv2_t1=debug" > log_config.txt
echo "ezv2_t1=debug,rumqttc=warn" > log_config.txt
```

The application watches this file and automatically reloads the configuration when it changes.

### Filter Syntax

The filter uses [tracing-subscriber's EnvFilter syntax](https://docs.rs/tracing-subscriber/latest/tracing_subscriber/filter/struct.EnvFilter.html):

- `debug` - Set all modules to debug level
- `info` - Set all modules to info level
- `ezv2_t1=debug` - Set only this crate to debug
- `ezv2_t1=debug,sqlx=warn` - Multiple targets with different levels
- `ezv2_t1[mqtt_message]=debug` - Enable debug for specific spans

### Startup Log Level

Set the `RUST_LOG` environment variable to configure the initial log level:

```bash
RUST_LOG=debug cargo run
RUST_LOG=ezv2_t1=debug,rumqttc=info cargo run
```

If `RUST_LOG` is not set, the default level is `info`.

## License

Licensed under either of

- Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE) or http://apache.org/licenses/LICENSE-2.0)
- MIT license ([LICENSE-MIT](LICENSE-MIT) or http://opensource.org/licenses/MIT)

### Contribution

Unless you explicitly state otherwise, any contribution intentionally submitted
for inclusion in the work by you, as defined in the Apache-2.0 license, shall
be dual licensed as above, without any additional terms or conditions.
