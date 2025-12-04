// Created with ChatGPT 5.1
//   https://chatgpt.com/share/6930bbf3-a5a8-800c-a89d-3719a0a5f58a
use std::time::Duration;
use std::{error::Error, fs, path::Path};

use config::{MqttConfig, load_config};
use database::{TeleSensor, TeleState, init_db, save_sensor, save_state};
use notify::{Config, Event as NotifyEvent, RecommendedWatcher, RecursiveMode, Watcher};
use rumqttc::{AsyncClient, Event, EventLoop, Incoming, MqttOptions, QoS};
use sqlx::SqlitePool;
use tracing::{debug, error, info, info_span, warn};
use tracing_subscriber::{EnvFilter, prelude::*, reload};

const CONFIG_FILE: &str = "ezv2-config.toml";

type ReloadHandle = reload::Handle<EnvFilter, tracing_subscriber::Registry>;

#[tokio::main]
pub async fn main() -> Result<(), Box<dyn Error>> {
    // Initialize logging with dynamic reload support
    let reload_handle = init_tracing();

    // Load configuration
    let config = load_config(CONFIG_FILE)?;
    info!("Loaded configuration from {CONFIG_FILE}");

    spawn_config_watcher(reload_handle, config.logging.config_file.clone());

    info!("ezv2: SQLite Logger starting");

    // Set up database and MQTT connections
    let pool = init_db(&config.database.filename).await?;
    let (_client, mut eventloop) = setup_mqtt(&config.mqtt).await?;

    let state_topic = format!("tele/{}/STATE", config.mqtt.topic_base);
    let sensor_topic = format!("tele/{}/SENSOR", config.mqtt.topic_base);

    // Main event loop: process incoming MQTT messages
    loop {
        match eventloop.poll().await {
            Ok(Event::Incoming(Incoming::Publish(p))) => {
                let topic = p.topic.clone();
                let payload_str = String::from_utf8_lossy(&p.payload);

                let _span = info_span!("mqtt_message", topic = %topic).entered();

                if topic == state_topic {
                    handle_state_message(&payload_str, &pool).await;
                } else if topic == sensor_topic {
                    handle_sensor_message(&payload_str, &pool).await;
                } else {
                    warn!("Other topic {} => {}", topic, payload_str);
                }
            }
            Ok(_) => {} // Ignore pings, acks, etc.
            Err(e) => {
                error!("MQTT error: {e}");
                return Err(e.into());
            }
        }
    }
}

/// Initialize tracing subscriber with a reloadable filter layer.
/// Returns a handle that can be used to dynamically change the log level.
fn init_tracing() -> ReloadHandle {
    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"));

    let (filter_layer, reload_handle) = reload::Layer::new(filter);

    tracing_subscriber::registry()
        .with(filter_layer)
        .with(tracing_subscriber::fmt::layer())
        .init();

    reload_handle
}

/// Watch log config file for changes and reload the tracing filter when modified.
/// Spawns a file watcher thread and a tokio task to handle reload events.
fn spawn_config_watcher(reload_handle: ReloadHandle, log_config_file: String) {
    let (tx, mut rx) = tokio::sync::mpsc::channel::<()>(10);

    let log_config_filename = Path::new(&log_config_file)
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or(&log_config_file)
        .to_string();

    let watch_filename = log_config_filename.clone();

    // Spawn file watcher in a separate thread (notify requires blocking)
    std::thread::spawn(move || {
        let tx_clone = tx.clone();
        let mut watcher = RecommendedWatcher::new(
            move |res: Result<NotifyEvent, notify::Error>| {
                if let Ok(event) = res {
                    // Check if event is related to log config file
                    let is_log_config = event.paths.iter().any(|p| {
                        p.file_name()
                            .and_then(|n| n.to_str())
                            .map(|n| n == watch_filename)
                            .unwrap_or(false)
                    });

                    if is_log_config {
                        // Trigger reload on close-write, create, or remove events
                        // Note: We use Access(Close(Write)) instead of Modify to avoid duplicate events
                        // from truncate + write generating two separate Modify(Data) inotify events
                        let is_close_write = matches!(
                            event.kind,
                            notify::EventKind::Access(notify::event::AccessKind::Close(
                                notify::event::AccessMode::Write
                            ))
                        );
                        if is_close_write || event.kind.is_create() || event.kind.is_remove() {
                            let _ = tx_clone.blocking_send(());
                        }
                    }
                }
            },
            Config::default(),
        )
        .expect("Failed to create file watcher");

        // Watch the current directory instead of the file directly
        // This allows us to detect file creation/deletion
        watcher
            .watch(std::path::Path::new("."), RecursiveMode::NonRecursive)
            .expect("Failed to watch current directory");

        // Keep watcher alive
        loop {
            std::thread::sleep(Duration::from_secs(1));
        }
    });

    // Handle reload events
    tokio::spawn(async move {
        while rx.recv().await.is_some() {
            // Read log level from file
            match fs::read_to_string(&log_config_file) {
                Ok(content) => {
                    // Collect all non-empty, non-comment lines and concatenate them
                    let level: String = content
                        .lines()
                        .map(|l| l.trim())
                        .filter(|l| !l.is_empty() && !l.starts_with('#'))
                        .collect::<Vec<_>>()
                        .join("");

                    if !level.is_empty() {
                        let new_filter = EnvFilter::new(&level);

                        match reload_handle.reload(new_filter) {
                            Ok(_) => {
                                info!(
                                    "Log configuration reloaded from {}: {}",
                                    log_config_file, level
                                )
                            }
                            Err(e) => error!("Failed to reload log configuration: {e}"),
                        }
                    } else {
                        warn!(
                            "{} contains no valid configuration (only comments/empty lines)",
                            log_config_file
                        );
                    }
                }
                Err(e) => {
                    // If file doesn't exist, just ignore (keep current log level)
                    if e.kind() != std::io::ErrorKind::NotFound {
                        error!("Failed to read {}: {e}", log_config_file);
                    }
                }
            }
        }
    });
}

/// Set up MQTT client, subscribe to topics, and configure telemetry period.
async fn setup_mqtt(config: &MqttConfig) -> Result<(AsyncClient, EventLoop), Box<dyn Error>> {
    info!("Setting up MQTT client");
    let mut mqttoptions =
        MqttOptions::new(&config.client_id, &config.broker_ip, config.broker_port);
    mqttoptions.set_keep_alive(Duration::from_secs(10));

    let (client, eventloop) = AsyncClient::new(mqttoptions, 10);

    let topic_base = &config.topic_base;
    client
        .subscribe(format!("tele/{topic_base}/STATE"), QoS::AtMostOnce)
        .await?;
    client
        .subscribe(format!("tele/{topic_base}/SENSOR"), QoS::AtMostOnce)
        .await?;
    info!("Subscribed to tele/{topic_base}/STATE and tele/{topic_base}/SENSOR");

    // Set TelePeriod
    let tele_period = config.tele_period;
    info!("Setting TelePeriod to {tele_period} seconds");
    client
        .publish(
            format!("cmnd/{topic_base}/TelePeriod"),
            QoS::AtLeastOnce,
            false,
            tele_period.to_string(),
        )
        .await?;
    info!("TelePeriod command sent ({tele_period} seconds)");

    Ok((client, eventloop))
}

/// Parse and save a STATE message to the database.
async fn handle_state_message(payload_str: &str, pool: &SqlitePool) {
    match serde_json::from_str::<TeleState>(payload_str) {
        Ok(state) => {
            if let Err(e) = save_state(pool, &state).await {
                error!("Failed to save STATE: {e}");
            } else {
                debug!("STATE saved: {}", state.time);
            }
        }
        Err(e) => {
            error!("Failed to parse STATE JSON: {e}");
            error!("Payload: {payload_str}");
        }
    }
}

/// Parse and save a SENSOR message to the database.
async fn handle_sensor_message(payload_str: &str, pool: &SqlitePool) {
    match serde_json::from_str::<TeleSensor>(payload_str) {
        Ok(sensor) => {
            if let Err(e) = save_sensor(pool, &sensor).await {
                error!("Failed to save SENSOR: {e}");
            } else {
                debug!(
                    "SENSOR saved: {}  P={}W V={}V",
                    sensor.time, sensor.energy.power, sensor.energy.voltage
                );
            }
        }
        Err(e) => {
            error!("Failed to parse SENSOR JSON: {e}");
            error!("Payload: {payload_str}");
        }
    }
}
