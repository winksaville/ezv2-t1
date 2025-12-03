use rumqttc::{AsyncClient, Event, Incoming, MqttOptions, QoS};
use std::time::Duration;

const BROKER_IP: &str = "192.168.1.195";
const BROKER_PORT: u16 = 1883;
const TOPIC_BASE: &str = "EZPlugV2_743EEC";

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // 1. Connect to broker
    let mut mqttoptions = MqttOptions::new(
        "ezplugv2_listener",
        BROKER_IP,
        BROKER_PORT,
    );
    mqttoptions.set_keep_alive(Duration::from_secs(10));

    let (client, mut eventloop) = AsyncClient::new(mqttoptions, 10);

    // 2. Subscribe to everything this plug publishes
    //    Tasmota convention: stat/<topic>/..., tele/<topic>/...
    client
        .subscribe(format!("stat/{TOPIC_BASE}/#"), QoS::AtMostOnce)
        .await?;
    client
        .subscribe(format!("tele/{TOPIC_BASE}/#"), QoS::AtMostOnce)
        .await?;

    println!("Subscribed to stat/{TOPIC_BASE}/# and tele/{TOPIC_BASE}/#");

    // 3. Ask Tasmota to dump full status (list of info) on its stat/* topics
    //    This is equivalent to running: topic: cmnd/EZPlugV2_743EEC/STATUS  payload: 0
    client
        .publish(
            format!("cmnd/{TOPIC_BASE}/STATUS"),
            QoS::AtLeastOnce,
            false,
            "0",
        )
        .await?;
    println!("Sent STATUS 0 command to cmnd/{TOPIC_BASE}/STATUS");

    // 4. Process incoming messages and print those from our plug
    loop {
        match eventloop.poll().await {
            Ok(Event::Incoming(Incoming::Publish(p))) => {
                let payload = String::from_utf8_lossy(&p.payload);
                println!("{} => {}", p.topic, payload);
            }
            Ok(_other) => {
                // ignore pings/acks/etc
            }
            Err(e) => {
                eprintln!("MQTT error: {e}");
                break;
            }
        }
    }

    Ok(())
}
