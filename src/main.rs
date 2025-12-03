use tokio::{time::{sleep, Duration}};
use rumqttc::{Client, MqttOptions, QoS, Event, Packet};

#[tokio::main]
async fn main() {
    // MQTT connection options
    let mut mqttoptions = MqttOptions::new("ezplug_listener", "192.168.1.195", 1883);
    mqttoptions.set_keep_alive(Duration::from_secs(10));

    // Create client + event loop
    let (client, mut eventloop) = Client::new(mqttoptions, 10);

    // Subscribe to all relevant EZPlug topics
    client.subscribe("EZPlugV2_743EEC/#", QoS::AtLeastOnce).unwrap();
    client.subscribe("cmnd/EZPlugV2_743EEC/#", QoS::AtLeastOnce).unwrap();
    client.subscribe("cmnd/ezplugs/#", QoS::AtLeastOnce).unwrap();

    println!("Listening for MQTT messages from EZPlugV2_743EEC...");

    // Poll for incoming MQTT events
    loop {
        match eventloop.poll().await {
            Ok(Event::Incoming(Packet::Publish(p))) => {
                let topic = p.topic;
                let payload = String::from_utf8_lossy(&p.payload);

                println!("📡 Topic: {}\n   Payload: {}\n", topic, payload);
            }
            Ok(_) => {}
            Err(e) => {
                eprintln!("MQTT error: {:?}", e);
                sleep(Duration::from_secs(2)).await;
            }
        }
    }
}
