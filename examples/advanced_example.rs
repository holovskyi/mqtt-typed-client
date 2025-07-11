//! # IoT Monitoring System
//!
//! Demonstration of mqtt_typed_client power with automatic topic parameter extraction,
//! type safety, and elegant API.

use std::{
	sync::Arc,
	time::{Duration, SystemTime, UNIX_EPOCH},
};

use bincode::{Decode, Encode};
use mqtt_typed_client::prelude::*;
use mqtt_typed_client::topic::topic_match::TopicMatch;
use mqtt_typed_client_macros::mqtt_topic;
use serde::{Deserialize, Serialize};
use tokio::{signal, time};

// === Data Types ===

#[derive(Serialize, Deserialize, Encode, Decode, Debug, Clone)]
struct SensorReading {
	value: f32,
	unit: String,
	timestamp: u64,
}

#[derive(Serialize, Deserialize, Encode, Decode, Debug)]
struct AlertConfig {
	threshold: f32,
	enabled: bool,
}

#[derive(Serialize, Deserialize, Encode, Decode, Debug)]
struct DeviceStatus {
	online: bool,
	battery: Option<usize>,
	last_seen: u64,
}

// === MQTT structures with auto-generation ===

/// Temperature sensors by buildings and rooms
#[derive(Debug)]
#[mqtt_topic("sensors/{building}/{floor}/temp/{sensor_id}")]
struct TemperatureSensor {
	building: String,  // Automatically extracted from topic
	floor: u32,        // Parsed to correct type
	sensor_id: String, // Sensor ID
	payload: SensorReading, // Data from message
	                   //topic: Arc<TopicMatch>, // Access to full topic
}

/// Device control commands
#[allow(dead_code)]
#[derive(Debug)]
#[mqtt_topic("control/{building}/devices/{device_type}/{device_id}", publisher)]
struct DeviceCommand {
	building: String,
	device_type: String, // "hvac", "lights", "security"
	device_id: String,
	payload: String, // JSON command
}

/// Device statuses (subscriber only)
#[derive(Debug)]
#[mqtt_topic("status/{building}/+/{device_id}/#", subscriber)]
struct DeviceStatusMsg {
	building: String,
	device_id: String,
	payload: DeviceStatus,
	topic: Arc<TopicMatch>, // For getting full path
}

/// Alerts with multi-level parameters (subscriber only!)
#[derive(Debug)]
#[mqtt_topic("alerts/{severity}/{category}/{details:#}", subscriber)]
struct Alert {
	severity: String, // "critical", "warning", "info"
	category: String, // "temperature", "security", "power"
	details: String,  // Everything after category/ -> "building1/floor2/room5"
	payload: String,  // Alert description
}

/// Alert commands (for publisher)
#[derive(Debug)]
#[mqtt_topic("alerts/{severity}/{category}", publisher)]
#[allow(dead_code)]
struct AlertCommand {
	severity: String,
	category: String,
	payload: String,
}

// === Usage demonstration ===

async fn smart_building_monitor() -> Result<()> {
	// Connection with automatic lifecycle management
	let (client, connection) = MqttClient::<BincodeSerializer>::connect(
		"mqtt://broker.mqtt.cool:1883?client_id=smart_building",
	)
	.await?;

	println!("🏢 Smart Building Monitor Started");
	println!("📡 Monitoring patterns:");
	println!("   🌡️  Temperatures: {}", TemperatureSensor::MQTT_PATTERN);
	println!("   📟 Device Status: {}", DeviceStatusMsg::MQTT_PATTERN);
	println!("   🚨 Alerts: {}", Alert::MQTT_PATTERN);

	// === Subscriptions with automatic parsing ===

	let mut temp_subscriber = TemperatureSensor::subscribe(&client).await?;
	let mut status_subscriber = DeviceStatusMsg::subscribe(&client).await?;
	let mut alert_subscriber = Alert::subscribe(&client).await?;

	// === Data simulation ===

	// Spawn task for sending test data
	let client_clone = client.clone();
	tokio::spawn(async move {
		let _ = simulate_building_data(client_clone).await;
	});

	// === Main monitoring loop ===
	// Add signal handling
	// let mut sigterm = tokio::signal::unix::signal(
	// 	tokio::signal::unix::SignalKind::terminate(),
	// )?;
	// let mut sigint = tokio::signal::unix::signal(
	// 	tokio::signal::unix::SignalKind::interrupt(),
	// )?;
	let mut ctrl_c = Box::pin(signal::ctrl_c());

	let mut message_count = 0;
	loop {
		tokio::select! {
			// Temperature sensor processing
			Some(temp_result) = temp_subscriber.receive() => {
				match temp_result {
					Ok(temp) => {
						println!(
							"🌡️  {}/Floor-{} Sensor-{}: {:.1}°C",
							temp.building, temp.floor, temp.sensor_id, temp.payload.value
						);

						// Automatic critical temperature check
						if temp.payload.value > 35.0 {
							AlertCommand::publish(
								&client,
								"critical",
								"temperature",
								&format!("High temperature: {:.1}°C in {}/floor{}/sensor{}",
									temp.payload.value, temp.building, temp.floor, temp.sensor_id)
							).await?;
						}
					}
					Err(e) => eprintln!("❌ Temperature parsing error: {e}"),
				}
			}

			// Device status processing
			Some(status_result) = status_subscriber.receive() => {
				match status_result {
					Ok(status) => {
						let battery_info = status.payload.battery
							.map(|b| format!(" (🔋{b}%)"))
							.unwrap_or_default();

						println!(
							"📟 {} Device-{}: {} {}",
							status.building,
							status.device_id,
							if status.payload.online { "🟢 Online" } else { "🔴 Offline" },
							battery_info
						);

						// Access to full topic for additional information
						if let Some(device_type) = status.topic.get_param(1) {
							if !status.payload.online {
								println!("   ⚠️  {} device offline in {}", device_type, status.building);
							}
						}
					}
					Err(e) => eprintln!("❌ Status parsing error: {e}"),
				}
			}

			// Alert processing
			Some(alert_result) = alert_subscriber.receive() => {
				match alert_result {
					Ok(alert) => {
						let icon = match alert.severity.as_str() {
							"critical" => "🚨",
							"warning" => "⚠️",
							_ => "ℹ️",
						};

						println!(
							"{} {} Alert [{}]: {} ({})",
							icon, alert.severity.to_uppercase(),
							alert.category, alert.payload, alert.details
						);
					}
					Err(e) => eprintln!("❌ Alert parsing error: {e}"),
				}
			}
			// Graceful shutdown signals
			_ = &mut ctrl_c => {
				println!("Received SIGTERM, shutting down gracefully...");
				break;
			}
		}

		message_count += 1;
		if message_count >= 15 {
			println!("\n🏁 Demo completed, shutting down...");
			break;
		}
	}

	// Graceful shutdown
	connection.shutdown().await?;
	println!("✅ Connection closed gracefully");

	Ok(())
}

async fn simulate_building_data(
	client: MqttClient<BincodeSerializer>,
) -> Result<()> {
	println!("🎭 Starting data simulation...\n");

	time::sleep(Duration::from_millis(500)).await;

	// Temperature sensor simulation
	for i in 0 .. 5 {
		let temp_data = SensorReading {
			value: 20.0 + (i as f32 * 3.2),
			unit: "°C".to_string(),
			timestamp: SystemTime::now()
				.duration_since(UNIX_EPOCH)
				.unwrap()
				.as_millis() as u64,
		};

		let building = if i % 2 == 0 { "TechHub" } else { "MainOffice" };
		let floor = (i % 3) + 1;
		let sensor_id = format!("TMP-{:03}", i + 100);

		// Publish through auto-generated method
		TemperatureSensor::publish(
			&client, building, floor, &sensor_id, &temp_data,
		)
		.await
		.ok();

		time::sleep(Duration::from_millis(300)).await;
	}

	// Device status simulation
	for i in 0 .. 4usize {
		let status = DeviceStatus {
			online: i % 3 != 0, // Some devices offline
			battery: if i % 2 == 0 { Some(85 - i * 10) } else { None },
			last_seen: SystemTime::now()
				.duration_since(UNIX_EPOCH)
				.unwrap()
				.as_millis() as u64,
		};

		let building = "TechHub";
		let device_type = ["hvac", "lights", "security"][i % 3usize];
		let device_id = format!("DEV-{:02}", i + 1);

		// Publish to pattern with + wildcard
		let topic = format!("status/{building}/{device_type}/{device_id}");
		client
			.get_publisher::<DeviceStatus>(&topic)?
			.publish(&status)
			.await
			.ok();

		time::sleep(Duration::from_millis(400)).await;
	}

	// Alert simulation
	for severity in ["warning", "critical", "info"] {
		AlertCommand::publish(
			&client,
			severity,
			"temperature",
			&format!(
				"Temperature anomaly detected by {severity} monitoring in \
				 TechHub"
			),
		)
		.await
		.ok();

		time::sleep(Duration::from_millis(200)).await;
	}
	Ok(())
}

#[tokio::main]
async fn main() -> Result<()> {
	// Simple logging
	tracing_subscriber::fmt()
		.with_max_level(tracing::Level::INFO)
		.compact()
		.init();

	println!("🚀 MQTT Typed Client - IoT Showcase");
	println!("=====================================\n");

	smart_building_monitor().await
}
