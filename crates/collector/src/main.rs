// Created with ChatGPT 5.1
//   https://chatgpt.com/share/6930bbf3-a5a8-800c-a89d-3719a0a5f58a
use std::time::Duration;
use std::{error::Error, fs, path::Path};

use notify::{Config, Event as NotifyEvent, RecommendedWatcher, RecursiveMode, Watcher};
use rumqttc::{AsyncClient, Event, EventLoop, Incoming, MqttOptions, QoS};
use serde::Deserialize;
use sqlx::{SqlitePool, sqlite::SqliteConnectOptions};
use tracing::{debug, error, info, info_span, warn};
use tracing_subscriber::{EnvFilter, prelude::*, reload};

const BROKER_IP: &str = "192.168.1.195";
const BROKER_PORT: u16 = 1883;
const TOPIC_BASE: &str = "EZPlugV2_743EEC";
const CLIENT_ID: &str = "ezplugv2_sqlite_logger_dev";

// https://tasmota.github.io/docs/Peripherals/#update-interval
const TELE_PERIOD: u64 = 10; // seconds 10..3600

#[derive(Debug, Deserialize)]
struct Wifi {
    #[serde(rename = "SSId")]
    ssid: String,
    #[serde(rename = "RSSI")]
    rssi: i32,
}

#[derive(Debug, Deserialize)]
struct TeleState {
    #[serde(rename = "Time")]
    time: String,
    #[serde(rename = "Uptime")]
    uptime: String,
    #[serde(rename = "UptimeSec")]
    uptime_sec: u64,
    #[serde(rename = "Heap")]
    heap: u32,
    #[serde(rename = "SleepMode")]
    sleep_mode: String,
    #[serde(rename = "Sleep")]
    sleep: u32,
    #[serde(rename = "LoadAvg")]
    load_avg: u32,
    #[serde(rename = "MqttCount")]
    mqtt_count: u32,
    #[serde(rename = "POWER1")]
    power1: String,
    #[serde(rename = "Wifi")]
    wifi: Wifi,
}

#[derive(Debug, Deserialize)]
struct Energy {
    #[serde(rename = "TotalStartTime")]
    total_start_time: String,
    #[serde(rename = "Total")]
    total: f64,
    #[serde(rename = "Yesterday")]
    yesterday: f64,
    #[serde(rename = "Today")]
    today: f64,
    #[serde(rename = "Period")]
    period: i64,
    #[serde(rename = "Power")]
    power: f64,
    #[serde(rename = "ApparentPower")]
    apparent_power: f64,
    #[serde(rename = "ReactivePower")]
    reactive_power: f64,
    #[serde(rename = "Factor")]
    factor: f64,
    #[serde(rename = "Voltage")]
    voltage: i64,
    #[serde(rename = "Current")]
    current: f64,
}

#[derive(Debug, Deserialize)]
struct TeleSensor {
    #[serde(rename = "Time")]
    time: String,
    #[serde(rename = "ENERGY")]
    energy: Energy,
}

type ReloadHandle = reload::Handle<EnvFilter, tracing_subscriber::Registry>;

#[tokio::main]
pub async fn main() -> Result<(), Box<dyn Error>> {
    // Initialize logging with dynamic reload support
    let reload_handle = init_tracing();
    spawn_config_watcher(reload_handle);

    info!("ezv2: SQLite Logger starting");

    // Set up database and MQTT connections
    let pool = init_db("ezplug.db").await?;
    let (_client, mut eventloop) = setup_mqtt().await?;

    let state_topic = format!("tele/{TOPIC_BASE}/STATE");
    let sensor_topic = format!("tele/{TOPIC_BASE}/SENSOR");

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

/// Watch log_config.txt for changes and reload the tracing filter when modified.
/// Spawns a file watcher thread and a tokio task to handle reload events.
fn spawn_config_watcher(reload_handle: ReloadHandle) {
    let (tx, mut rx) = tokio::sync::mpsc::channel::<()>(10);

    // Spawn file watcher in a separate thread (notify requires blocking)
    std::thread::spawn(move || {
        let tx_clone = tx.clone();
        let mut watcher = RecommendedWatcher::new(
            move |res: Result<NotifyEvent, notify::Error>| {
                if let Ok(event) = res {
                    // Check if event is related to log_config.txt
                    let is_log_config = event.paths.iter().any(|p| {
                        p.file_name()
                            .and_then(|n| n.to_str())
                            .map(|n| n == "log_config.txt")
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
            match fs::read_to_string("log_config.txt") {
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
                                info!("Log configuration reloaded from log_config.txt: {}", level)
                            }
                            Err(e) => error!("Failed to reload log configuration: {e}"),
                        }
                    } else {
                        warn!(
                            "log_config.txt contains no valid configuration (only comments/empty lines)"
                        );
                    }
                }
                Err(e) => {
                    // If file doesn't exist, just ignore (keep current log level)
                    if e.kind() != std::io::ErrorKind::NotFound {
                        error!("Failed to read log_config.txt: {e}");
                    }
                }
            }
        }
    });
}

/// Connect to SQLite database and create tables if they don't exist.
async fn init_db(filename: impl AsRef<Path>) -> Result<SqlitePool, Box<dyn Error>> {
    info!("ezv2: connect_to_db({})", filename.as_ref().display());
    let options = SqliteConnectOptions::new()
        .filename(&filename)
        .create_if_missing(true);
    let pool = SqlitePool::connect_with(options).await?;
    info!("ezv2: connected to {}", filename.as_ref().display());

    info!("Create/connect to state table");
    sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS state (
            id          INTEGER PRIMARY KEY AUTOINCREMENT,
            time        TEXT NOT NULL,
            uptime      TEXT NOT NULL,
            uptime_sec  INTEGER NOT NULL,
            heap        INTEGER NOT NULL,
            sleep_mode  TEXT NOT NULL,
            sleep       INTEGER NOT NULL,
            load_avg    INTEGER NOT NULL,
            mqtt_count  INTEGER NOT NULL,
            power1      TEXT NOT NULL,
            wifi_ssid   TEXT NOT NULL,
            wifi_rssi   INTEGER NOT NULL
        );
        "#,
    )
    .execute(&pool)
    .await?;

    info!("Create/connect to sensor table");
    sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS sensor (
            id               INTEGER PRIMARY KEY AUTOINCREMENT,
            time             TEXT NOT NULL,
            total_start_time TEXT NOT NULL,
            total            REAL NOT NULL,
            yesterday        REAL NOT NULL,
            today            REAL NOT NULL,
            period           INTEGER NOT NULL,
            power            REAL NOT NULL,
            apparent_power   REAL NOT NULL,
            reactive_power   REAL NOT NULL,
            factor           REAL NOT NULL,
            voltage          INTEGER NOT NULL,
            current          REAL NOT NULL
        );
        "#,
    )
    .execute(&pool)
    .await?;

    Ok(pool)
}

/// Set up MQTT client, subscribe to topics, and configure telemetry period.
async fn setup_mqtt() -> Result<(AsyncClient, EventLoop), Box<dyn Error>> {
    info!("Setting up MQTT client");
    let mut mqttoptions = MqttOptions::new(CLIENT_ID, BROKER_IP, BROKER_PORT);
    mqttoptions.set_keep_alive(Duration::from_secs(10));

    let (client, eventloop) = AsyncClient::new(mqttoptions, 10);

    client
        .subscribe(format!("tele/{TOPIC_BASE}/STATE"), QoS::AtMostOnce)
        .await?;
    client
        .subscribe(format!("tele/{TOPIC_BASE}/SENSOR"), QoS::AtMostOnce)
        .await?;
    info!("Subscribed to tele/{TOPIC_BASE}/STATE and tele/{TOPIC_BASE}/SENSOR");

    // Set TelePeriod to TELE_PERIOD seconds
    info!("Setting TelePeriod to {TELE_PERIOD} seconds");
    client
        .publish(
            format!("cmnd/{TOPIC_BASE}/TelePeriod"),
            QoS::AtLeastOnce,
            false,
            "{TELE_PERIOD}",
        )
        .await?;
    info!("TelePeriod command sent ({TELE_PERIOD} seconds)");

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

/// Insert a TeleState record into the state table.
async fn save_state(pool: &SqlitePool, s: &TeleState) -> Result<(), sqlx::Error> {
    sqlx::query(
        r#"
        INSERT INTO state (
            time, uptime, uptime_sec, heap,
            sleep_mode, sleep, load_avg, mqtt_count,
            power1, wifi_ssid, wifi_rssi
        ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)
        "#,
    )
    .bind(&s.time)
    .bind(&s.uptime)
    .bind(s.uptime_sec as i64)
    .bind(s.heap as i64)
    .bind(&s.sleep_mode)
    .bind(s.sleep as i64)
    .bind(s.load_avg as i64)
    .bind(s.mqtt_count as i64)
    .bind(&s.power1)
    .bind(&s.wifi.ssid)
    .bind(s.wifi.rssi)
    .execute(pool)
    .await?;
    Ok(())
}

/// Insert a TeleSensor record into the sensor table.
async fn save_sensor(pool: &SqlitePool, s: &TeleSensor) -> Result<(), sqlx::Error> {
    let e = &s.energy;
    sqlx::query(
        r#"
        INSERT INTO sensor (
            time, total_start_time, total, yesterday, today,
            period, power, apparent_power, reactive_power,
            factor, voltage, current
        ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)
        "#,
    )
    .bind(&s.time)
    .bind(&e.total_start_time)
    .bind(e.total)
    .bind(e.yesterday)
    .bind(e.today)
    .bind(e.period)
    .bind(e.power)
    .bind(e.apparent_power)
    .bind(e.reactive_power)
    .bind(e.factor)
    .bind(e.voltage)
    .bind(e.current)
    .execute(pool)
    .await?;
    Ok(())
}
