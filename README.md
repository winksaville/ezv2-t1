# ezv2-t1

An MQTT client for logging telemetry data from an EZPlugV2 (Tasmota) smart plug to a SQLite database.

Created with:
- ChatGPT 5.1: Initial implementation
  https://chatgpt.com/share/6930bbf3-a5a8-800c-a89d-3719a0a5f58a
- Claude Code: Added dynamic log level configuration, refactored into functions, and documentation

## Suggested Cloning procudure

**I've placed this first I suggest a slightly different cloning technique.**

Ideally the `.claude` directory in this repo is symlinked from `~/.claude/projects/<FullPath>-ezv2-t1/`
to preserve Claude conversation history within the repository. Where FullPath is
a claude compatible path with '/' replaced with '-'. So if the
<FullPath> to ezv2-t1 is `/home/wink/data/prgs/mqtt/ezv2-t1` then
`<FullPath>-ezv2-t1` is `-home-wink-data-prgs-mqtt-ezv2-t1`

The suggested way to clone this repo is:
```bash
wink@3900x 25-12-04T19:16:30.500Z:~/data/prgs/mqtt
$ git clone git@github.com:winksaville/ezv2-t1
Cloning into 'ezv2-t1'...
remote: Enumerating objects: 88, done.
remote: Counting objects: 100% (88/88), done.
remote: Compressing objects: 100% (30/30), done.
remote: Total 88 (delta 43), reused 88 (delta 43), pack-reused 0 (from 0)
Receiving objects: 100% (88/88), 224.16 KiB | 2.24 MiB/s, done.
Resolving deltas: 100% (43/43), done.
wink@3900x 25-12-04T19:16:39.843Z:~/data/prgs/mqtt
$ cd ezv2-t1/
wink@3900x 25-12-04T19:16:46.421Z:~/data/prgs/mqtt/ezv2-t1 (main)
$ ln -sf $(pwd)/.claude ~/.claude/projects/-home-wink-data-prgs-mqtt-ezv2-t1
```

And you can verify the symlink using diff:
```bash
wink@3900x 25-12-04T19:17:06.452Z:~/data/prgs/mqtt/ezv2-t1 (main)
$ diff ~/.claude/projects/-home-wink-data-prgs-mqtt-ezv2-t1 .claude
wink@3900x 25-12-04T19:17:26.983Z:~/data/prgs/mqtt/ezv2-t1 (main)
$ echo $?
0
wink@3900x 25-12-04T19:17:32.087Z:~/data/prgs/mqtt/ezv2-t1 (main
```

Note: if you endup with a `.claude/.claude` symlink it means
that `~/.claude/projects/<FullPath>-ezv2-t1` already existed,
just delete the redundant symlink `rm .claude/.claude`

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

### Collector

```bash
cargo run -p collector
```

The collector will:
1. Create `ezplug.db` if it doesn't exist
2. Connect to the MQTT broker at 192.168.1.195:1883
3. Subscribe to telemetry topics and begin logging data

### Viewer

Plot power data from the database to a PNG file:

```bash
cargo run -p viewer -- <db_file> <output.png> [start] [count]
```

Arguments:
- `db_file` - Path to the SQLite database
- `output.png` - Output PNG file path
- `start` - Starting row ID (default: 0)
- `count` - Number of points to plot (default: 100)

Example:
```bash
cargo run -p viewer -- ezplug.db power.png 0 500
```

## Configuration

The application is configured via `ezv2-config.toml`:

```toml
[mqtt]
broker_ip = "192.168.1.195"
broker_port = 1883
topic_base = "EZPlugV2_743EEC"
client_id = "ezplugv2_sqlite_logger_dev"
tele_period = 10  # Telemetry interval in seconds (10-3600)

[database]
filename = "ezplug.db"

[logging]
config_file = "log_config.txt"  # File for dynamic log level changes
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
