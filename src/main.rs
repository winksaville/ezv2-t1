use std::{error::Error, path::Path};
use std::time::Duration;

use rumqttc::{AsyncClient, Event, Incoming, MqttOptions, QoS};
use serde::Deserialize;
use sqlx::{SqlitePool, sqlite::SqliteConnectOptions};

const BROKER_IP: &str = "192.168.1.195";
const BROKER_PORT: u16 = 1883;
const TOPIC_BASE: &str = "EZPlugV2_743EEC";

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

#[tokio::main]
pub async fn main() -> Result<(), Box<dyn Error>> {
    println!("ezv2: SQLite Logger starting");

    println!("ezv2: connect_to_db(ezplug.db)");
    let pool = connect_to_db("ezplug.db").await?;
    println!("ezv2: connected to ezplug.db");


    println!("Create/connect to state table");
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

    // Create/connect sensor table
    println!("Create/connect to sensor table");
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

    // ---------- MQTT SETUP ----------
    println!("Setting up MQTT client");
    let mut mqttoptions =
        MqttOptions::new("ezplugv2_sqlite_logger", BROKER_IP, BROKER_PORT);
    mqttoptions.set_keep_alive(Duration::from_secs(10));

    let (client, mut eventloop) = AsyncClient::new(mqttoptions, 10);

    client
        .subscribe(format!("tele/{TOPIC_BASE}/STATE"), QoS::AtMostOnce)
        .await?;
    client
        .subscribe(format!("tele/{TOPIC_BASE}/SENSOR"), QoS::AtMostOnce)
        .await?;
    println!("Subscribed to tele/{TOPIC_BASE}/STATE and tele/{TOPIC_BASE}/SENSOR");

    // Set TelePeriod to TELE_PERIOD seconds
    println!("Setting TelePeriod to {TELE_PERIOD} seconds");
    client
        .publish(
            format!("cmnd/{TOPIC_BASE}/TelePeriod"),
            QoS::AtLeastOnce,
            false,
            "{TELE_PERIOD}",
        )
        .await?;
    println!("TelePeriod command sent ({TELE_PERIOD} seconds)");

    let state_topic = format!("tele/{TOPIC_BASE}/STATE");
    let sensor_topic = format!("tele/{TOPIC_BASE}/SENSOR");

    let pool = pool.clone();

    // ---------- MAIN LOOP ----------
    loop {
        match eventloop.poll().await {
            Ok(Event::Incoming(Incoming::Publish(p))) => {
                let topic = p.topic.clone();
                let payload_str = String::from_utf8_lossy(&p.payload);

                if topic == state_topic {
                    match serde_json::from_str::<TeleState>(&payload_str) {
                        Ok(state) => {
                            if let Err(e) = save_state(&pool, &state).await {
                                eprintln!("Failed to save STATE: {e}");
                            } else {
                                println!("STATE saved: {}", state.time);
                            }
                        }
                        Err(e) => {
                            eprintln!("Failed to parse STATE JSON: {e}");
                            eprintln!("Payload: {payload_str}");
                        }
                    }
                } else if topic == sensor_topic {
                    match serde_json::from_str::<TeleSensor>(&payload_str) {
                        Ok(sensor) => {
                            if let Err(e) = save_sensor(&pool, &sensor).await {
                                eprintln!("Failed to save SENSOR: {e}");
                            } else {
                                println!(
                                    "SENSOR saved: {}  P={}W V={}V",
                                    sensor.time,
                                    sensor.energy.power,
                                    sensor.energy.voltage
                                );
                            }
                        }
                        Err(e) => {
                            eprintln!("Failed to parse SENSOR JSON: {e}");
                            eprintln!("Payload: {payload_str}");
                        }
                    }
                } else {
                    // Shouldn't happen with this subscribe list, but harmless
                    println!("Other topic {} => {}", topic, payload_str);
                }
            }
            Ok(_) => {
                // ignore pings/acks etc
            }
            Err(e) => {
                eprintln!("MQTT error: {e}");
                return Err(e.into());
            }
        }
    }
}

// ---------- DB HELPERS ----------
async fn connect_to_db(
    filename: impl AsRef<Path>,
) -> Result<SqlitePool, sqlx::Error> {
    let options = SqliteConnectOptions::new()
        .filename(filename)
        .create_if_missing(true);

    SqlitePool::connect_with(options).await
}

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
