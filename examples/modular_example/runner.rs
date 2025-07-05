use mqtt_typed_client::prelude::*;
use tokio::select;


// Variant A
//use super::topics::*;

// Variant B for more specific imports
use super::topics::TemperatureReading;
use super::topics::temperature_topic::*;



fn get_server(server: &str, client_id: &str) -> String {
	format!("{server}?client_id={client_id}&clean_session=true")
}
const _SERVER_COOL: &str = "mqtt://broker.mqtt.cool:1883";
const _SERVER_MODSQITO: &str = "mqtt://test.mosquitto.org:1883";
const SERVER: &str = _SERVER_COOL;

pub async fn run_example() -> Result<()> {
	let (client, connection) = MqttClient::<BincodeSerializer>::connect(
		&get_server(SERVER, "rust-publisher"),
	)
	.await?;

	let temp_client = client.temperature_topic();

	let temp = TemperatureReading {
		device_id: 42,
		temperature: 23.5,
		humidity: Some(45.0),
		battery_level: Some(80),
	};

    //temp_client.
	let publisher = temp_client.get_publisher("Home", "floor", 37)?;
	tokio::spawn(async move {
		//sleep for 100 ms for subscription to be ready
		tokio::time::sleep(std::time::Duration::from_millis(100)).await;
		publisher.publish(&temp).await.unwrap();
	});

    
    let mut subscriber_all = temp_client.subscribe().await?;
    
	let mut subscriber370 = temp_client
		.subscription()
		.for_device_id(370)
		.with_cache(100)
		.subscribe()
		.await?;



    loop {
        select! {
            Some(Ok(temp_msg)) = subscriber_all.receive() => {
                println!("Received a message from the all-sensors topic {:?}", temp_msg.topic );
            },
            Some(Ok(temp_msg)) = subscriber370.receive() => {
                println!("Received a message from the specific device topic {:?}", temp_msg.topic);
            }
        }
    }
	// if let Some(Ok(temp_msg)) = subscriber37.receive().await {
    //     println!("Received temperature message from topic: {}", temp_msg.topic.topic_path());
    //     println!("Location: {}", temp_msg.location);
    //     println!("Sensor Type: {}", temp_msg.sensor_type);
    //     println!("Device ID: {}", temp_msg.device_id);
	// 	println!("Temperature: {}", temp_msg.payload.temperature);
    //     println!("Humidity: {:?}", temp_msg.payload.humidity);
    //     println!("Battery Level: {:?}", temp_msg.payload.battery_level);
	// 	println!("Received temperature message: {temp_msg:?}");
	// } else {
	// 	println!("No temperature message received");
	// }

	// connection.shutdown().await?;
    // println!("✅ Connection closed gracefully");
	//Ok(())
}
