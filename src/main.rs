mod client;
mod message_serializer;
mod routing;
mod topic;
//#[cfg(test)]

use std::time::Duration;

use bincode::{Decode, Encode};
use client::MqttAsyncClient;
use message_serializer::BincodeSerializer;
//use mqtt_async_client::MqttAsyncClient;
use serde::{Deserialize, Serialize};
use tokio::time;

#[derive(Serialize, Deserialize, Debug, Encode, Decode, PartialEq)]
struct MyData {
	id: u32,
}

pub async fn test_main() -> Result<(), Box<dyn std::error::Error>> {
	let client = MqttAsyncClient::<BincodeSerializer>::new(
		"mqtt://broker.mqtt.cool:1883?client_id=rumqtt-async",
	)
	.await?;

	let publisher = client.get_publisher::<MyData>("hello/typed")?;
	let mut subscriber = client.subscribe::<MyData>("hello/typed").await?;

	tokio::spawn(async move {
		for i in 0 .. 1000 {
			println!("PUBLISH:{i}");

			let data = MyData { id: i };

			let res = publisher.publish(&data).await;
			match res {
				| Ok(()) => (),
				| Err(err) => println!("PUBLISH ERROR:{err:?}"),
			}
			time::sleep(Duration::from_secs(1)).await;
		}
	});

	let mut client_opt = Some(client);
	let mut count = 0;
	while let Some((topic, data)) = subscriber.receive().await {
		if count == 10 {
			// if let Err(err) = subscriber.cancel().await {
			//     eprintln!("Failed to cancel subscription: {err}");
			// }
			if let Some(client) = client_opt.take() {
				let _res = client.shutdown().await;
				eprintln!("Client shutdown res = {_res:?}");
			}
			//break;
		}
		if let Ok(data) = data {
			println!("XXXX Received on topic '{}': data={data:?}", topic);
		} else {
			println!("XXXX Failed to deserialize data");
		}
		count += 1;
	}
	eprintln!("Exit from subscriber listen loop");
	//subscriber.cancel();
	time::sleep(Duration::from_secs(20)).await;
	Ok(())
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
	test_main().await
}
