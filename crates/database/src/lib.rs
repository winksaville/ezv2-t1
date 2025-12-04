use std::path::Path;

use serde::Deserialize;
use sqlx::{SqlitePool, sqlite::SqliteConnectOptions};
use tracing::info;

#[derive(Debug, Deserialize)]
pub struct Wifi {
    #[serde(rename = "SSId")]
    pub ssid: String,
    #[serde(rename = "RSSI")]
    pub rssi: i32,
}

#[derive(Debug, Deserialize)]
pub struct TeleState {
    #[serde(rename = "Time")]
    pub time: String,
    #[serde(rename = "Uptime")]
    pub uptime: String,
    #[serde(rename = "UptimeSec")]
    pub uptime_sec: u64,
    #[serde(rename = "Heap")]
    pub heap: u32,
    #[serde(rename = "SleepMode")]
    pub sleep_mode: String,
    #[serde(rename = "Sleep")]
    pub sleep: u32,
    #[serde(rename = "LoadAvg")]
    pub load_avg: u32,
    #[serde(rename = "MqttCount")]
    pub mqtt_count: u32,
    #[serde(rename = "POWER1")]
    pub power1: String,
    #[serde(rename = "Wifi")]
    pub wifi: Wifi,
}

#[derive(Debug, Deserialize)]
pub struct Energy {
    #[serde(rename = "TotalStartTime")]
    pub total_start_time: String,
    #[serde(rename = "Total")]
    pub total: f64,
    #[serde(rename = "Yesterday")]
    pub yesterday: f64,
    #[serde(rename = "Today")]
    pub today: f64,
    #[serde(rename = "Period")]
    pub period: i64,
    #[serde(rename = "Power")]
    pub power: f64,
    #[serde(rename = "ApparentPower")]
    pub apparent_power: f64,
    #[serde(rename = "ReactivePower")]
    pub reactive_power: f64,
    #[serde(rename = "Factor")]
    pub factor: f64,
    #[serde(rename = "Voltage")]
    pub voltage: i64,
    #[serde(rename = "Current")]
    pub current: f64,
}

#[derive(Debug, Deserialize)]
pub struct TeleSensor {
    #[serde(rename = "Time")]
    pub time: String,
    #[serde(rename = "ENERGY")]
    pub energy: Energy,
}

/// Connect to SQLite database and create tables if they don't exist.
pub async fn init_db(filename: impl AsRef<Path>) -> Result<SqlitePool, sqlx::Error> {
    info!("database: connecting to {}", filename.as_ref().display());
    let options = SqliteConnectOptions::new()
        .filename(&filename)
        .create_if_missing(true);
    let pool = SqlitePool::connect_with(options).await?;
    info!("database: connected to {}", filename.as_ref().display());

    info!("database: creating state table if needed");
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

    info!("database: creating sensor table if needed");
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

/// Insert a TeleState record into the state table.
pub async fn save_state(pool: &SqlitePool, s: &TeleState) -> Result<(), sqlx::Error> {
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
pub async fn save_sensor(pool: &SqlitePool, s: &TeleSensor) -> Result<(), sqlx::Error> {
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
