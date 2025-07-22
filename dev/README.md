# Development MQTT Broker

Local EMQ X broker setup for testing mqtt-typed-client examples and development.

## Quick Start

1. **Start MQTT broker:**
   ```bash
   cd dev
   docker-compose up -d
   ```

2. **Verify broker is running:**
   ```bash
   docker-compose ps
   # Should show emqx container as "Up"
   ```

3. **Run examples:**
   ```bash
   # From project root
   cargo run --example 000_hello_world
   cargo run --example 004_hello_world_tls  # Requires TLS setup
   ```

4. **Stop broker:**
   ```bash
   cd dev
   docker-compose down
   ```

## MQTT Broker Access

- **MQTT (plain):** `localhost:1883`
- **MQTTS (TLS):** `localhost:8883` 
- **WebSocket:** `localhost:8083`
- **WebSocket Secure:** `localhost:8084`
- **Management Dashboard:** http://localhost:18083
  - Username: `admin`
  - Password: `public`

## TLS Configuration

The broker includes self-signed certificates for TLS testing:
- CA Certificate: `dev/certs/ca.pem`
- Server Certificate: `dev/certs/cert.pem`
- Server Key: `dev/certs/key.pem`

**Note:** These are development certificates only - never use in production!

## Testing Tools

Optional mosquitto clients for manual testing:
```bash
docker-compose --profile tools up -d
docker exec -it mqtt-typed-client-tools mosquitto_pub -h emqx -t test/topic -m "Hello World"
docker exec -it mqtt-typed-client-tools mosquitto_sub -h emqx -t test/topic
```

## Troubleshooting

- **Port conflicts:** If ports 1883/8883 are in use, modify `docker-compose.yml`
- **TLS issues:** Ensure examples use the correct CA certificate path
- **Connection failures:** Check that broker is running with `docker-compose ps`
