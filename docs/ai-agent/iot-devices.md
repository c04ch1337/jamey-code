# IoT Device Connectivity

**Jamey 2.0** includes comprehensive IoT device connectivity capabilities, allowing secure communication with smart home devices, sensors, and IoT hubs via MQTT, HTTP/REST, and other protocols.

> ⚠️ **Security Note**: IoT devices require approval for security. All device credentials are encrypted and stored securely using the system keyring.

## Overview

The IoT Device Connector provides:

- **Multi-Protocol Support**: MQTT, MQTT-TLS, HTTP, HTTPS, CoAP, WebSocket
- **Secure Credential Storage**: Encrypted credentials in system keyring
- **mTLS Support**: Mutual TLS authentication for MQTT connections
- **Device Discovery**: Automatic discovery via mDNS/Bonjour
- **Device Management**: Register, connect, disconnect, and monitor devices
- **Real-time Communication**: Publish/subscribe to MQTT topics, HTTP commands

**Capability Level**: `NetworkAccess` | **Requires Approval**: ✅ Yes

## Supported Protocols

| Protocol | Description | Security | Use Cases |
|----------|-------------|----------|-----------|
| `Mqtt` | MQTT over TCP | Username/password | Local IoT devices |
| `MqttTls` | MQTT over TLS | mTLS + username/password | Production IoT |
| `Http` | HTTP REST API | Bearer token, API key, Basic auth | Smart home hubs |
| `Https` | HTTPS REST API | Bearer token, API key, Basic auth | Cloud IoT services |
| `Coap` | Constrained Application Protocol | DTLS (future) | Low-power sensors |
| `WebSocket` | WebSocket connection | WSS (TLS) | Real-time dashboards |

## Quick Start

### 1. Register a Device

```rust
use std::collections::HashMap;
use jamey_tools::connectors::iot::{IoTDevice, DeviceProtocol, DeviceStatus};

// Create device configuration
let device = IoTDevice {
    id: "smart-light-01".to_string(),
    name: "Living Room Light".to_string(),
    device_type: "smart_light".to_string(),
    protocol: DeviceProtocol::MqttTls,
    endpoint: "mqtts://homeassistant.local:8883".to_string(),
    credentials: {
        let mut creds = HashMap::new();
        creds.insert("username".to_string(), "jamey".to_string());
        creds.insert("password".to_string(), "secure_password".to_string());
        // Optional: mTLS certificates (base64 encoded)
        // creds.insert("ca_cert".to_string(), base64_ca_cert);
        // creds.insert("client_cert".to_string(), base64_client_cert);
        // creds.insert("client_key".to_string(), base64_client_key);
        creds
    },
    metadata: HashMap::new(),
    last_seen: None,
    status: DeviceStatus::Unknown,
};

// Register device
let mut params = HashMap::new();
params.insert("action".to_string(), "register_device".to_string());
params.insert("device".to_string(), serde_json::to_string(&device)?);

let result = state.hybrid_orchestrator
    .lock()
    .await
    .execute_connector("iot", params)
    .await?;
```

### 2. Connect to Device

```rust
let mut params = HashMap::new();
params.insert("action".to_string(), "connect_device".to_string());
params.insert("device_id".to_string(), "smart-light-01".to_string());

let result = state.hybrid_orchestrator
    .lock()
    .await
    .execute_connector("iot", params)
    .await?;
```

### 3. Send Commands

**MQTT Publish:**
```rust
let mut params = HashMap::new();
params.insert("action".to_string(), "mqtt_publish".to_string());
params.insert("device_id".to_string(), "smart-light-01".to_string());
params.insert("topic".to_string(), "home/lights/living_room/set".to_string());
params.insert("payload".to_string(), r#"{"state": "on", "brightness": 255}"#.to_string());
params.insert("qos".to_string(), "1".to_string()); // 0, 1, or 2

let result = state.hybrid_orchestrator
    .lock()
    .await
    .execute_connector("iot", params)
    .await?;
```

**HTTP Command:**
```rust
let mut params = HashMap::new();
params.insert("action".to_string(), "http_command".to_string());
params.insert("device_id".to_string(), "smart-light-01".to_string());
params.insert("method".to_string(), "POST".to_string());
params.insert("path".to_string(), "/api/lights/living_room".to_string());
params.insert("body".to_string(), r#"{"state": "on"}"#.to_string());

let result = state.hybrid_orchestrator
    .lock()
    .await
    .execute_connector("iot", params)
    .await?;
```

## Available Actions

### Device Management

| Action | Parameters | Description |
|--------|------------|-------------|
| `register_device` | `device` (JSON) | Register a new IoT device |
| `connect_device` | `device_id` | Connect to a registered device |
| `disconnect_device` | `device_id` | Disconnect from a device |
| `remove_device` | `device_id` | Remove device and clean up credentials |
| `list_devices` | (none) | List all registered devices |
| `get_status` | `device_id` | Get connection status of a device |
| `update_status` | `device_id`, `status` (JSON) | Manually update device status |

### Communication

| Action | Parameters | Description |
|--------|------------|-------------|
| `mqtt_publish` | `device_id`, `topic`, `payload`, `qos` (0-2) | Publish MQTT message |
| `mqtt_subscribe` | `device_id`, `topic`, `qos` (0-2) | Subscribe to MQTT topic |
| `http_command` | `device_id`, `method`, `path`, `body` (optional), `headers` (optional) | Send HTTP/REST command |

### Discovery

| Action | Parameters | Description |
|--------|------------|-------------|
| `discover` | (none) | Discover devices on local network via mDNS |

## Security Features

### 1. Encrypted Credential Storage

All device credentials are encrypted and stored in the system keyring:

```rust
// Credentials are automatically encrypted when registering
// They are never stored in plain text
device.credentials.insert("password".to_string(), "secret".to_string());
// After registration, credentials are cleared from device struct
// and stored securely in keyring
```

### 2. mTLS Support

For MQTT-TLS connections, you can provide custom certificates:

```rust
let mut credentials = HashMap::new();
credentials.insert("username".to_string(), "device_user".to_string());
credentials.insert("password".to_string(), "device_pass".to_string());

// Base64-encoded certificates
credentials.insert("ca_cert".to_string(), base64::encode(ca_cert_bytes));
credentials.insert("client_cert".to_string(), base64::encode(client_cert_bytes));
credentials.insert("client_key".to_string(), base64::encode(client_key_bytes));
```

### 3. Endpoint Validation

All device endpoints are validated for security:

- **MQTT**: Must include port (e.g., `mqtts://broker.local:8883`)
- **HTTP/HTTPS**: Must be valid URL format
- **Local Network**: Allowed for IoT devices (unlike web connector)

### 4. Authentication Methods

Supported authentication methods:

- **MQTT**: Username/password, mTLS certificates
- **HTTP/REST**: Bearer token, API key, Basic auth

```rust
// Bearer token
credentials.insert("token".to_string(), "your_bearer_token".to_string());

// API key
credentials.insert("api_key".to_string(), "your_api_key".to_string());

// Basic auth
credentials.insert("username".to_string(), "user".to_string());
credentials.insert("password".to_string(), "pass".to_string());
```

## Device Discovery

Automatically discover devices on your local network:

```rust
let mut params = HashMap::new();
params.insert("action".to_string(), "discover".to_string());

let result = state.hybrid_orchestrator
    .lock()
    .await
    .execute_connector("iot", params)
    .await?;

// Parse discovered devices
let devices: Vec<IoTDevice> = serde_json::from_str(&result.output)?;
```

**Supported Service Types:**
- `_mqtt._tcp.local.` - MQTT brokers
- `_http._tcp.local.` - HTTP devices
- `_https._tcp.local.` - HTTPS devices
- `_coap._udp.local.` - CoAP devices
- `_iot._tcp.local.` - Generic IoT devices

## Example: Smart Home Integration

Complete example for controlling a smart light:

```rust
use std::collections::HashMap;
use jamey_tools::connectors::iot::{IoTDevice, DeviceProtocol, DeviceStatus};

// 1. Register device
let device = IoTDevice {
    id: "hue-bridge-01".to_string(),
    name: "Philips Hue Bridge".to_string(),
    device_type: "hue_bridge".to_string(),
    protocol: DeviceProtocol::Https,
    endpoint: "https://192.168.1.100".to_string(),
    credentials: {
        let mut creds = HashMap::new();
        creds.insert("api_key".to_string(), "your_hue_api_key".to_string());
        creds
    },
    metadata: HashMap::new(),
    last_seen: None,
    status: DeviceStatus::Unknown,
};

let mut params = HashMap::new();
params.insert("action".to_string(), "register_device".to_string());
params.insert("device".to_string(), serde_json::to_string(&device)?);

state.hybrid_orchestrator
    .lock()
    .await
    .execute_connector("iot", params)
    .await?;

// 2. Connect
let mut params = HashMap::new();
params.insert("action".to_string(), "connect_device".to_string());
params.insert("device_id".to_string(), "hue-bridge-01".to_string());

state.hybrid_orchestrator
    .lock()
    .await
    .execute_connector("iot", params)
    .await?;

// 3. Turn on light
let mut params = HashMap::new();
params.insert("action".to_string(), "http_command".to_string());
params.insert("device_id".to_string(), "hue-bridge-01".to_string());
params.insert("method".to_string(), "PUT".to_string());
params.insert("path".to_string(), "/api/your_username/lights/1/state".to_string());
params.insert("body".to_string(), r#"{"on": true, "bri": 254}"#.to_string());

let result = state.hybrid_orchestrator
    .lock()
    .await
    .execute_connector("iot", params)
    .await?;
```

## Example: MQTT Sensor Monitoring

Monitor MQTT sensors and subscribe to topics:

```rust
// 1. Register MQTT device
let device = IoTDevice {
    id: "sensor-hub-01".to_string(),
    name: "Temperature Sensor Hub".to_string(),
    device_type: "sensor_hub".to_string(),
    protocol: DeviceProtocol::MqttTls,
    endpoint: "mqtts://mqtt.broker.local:8883".to_string(),
    credentials: {
        let mut creds = HashMap::new();
        creds.insert("username".to_string(), "sensor_user".to_string());
        creds.insert("password".to_string(), "sensor_pass".to_string());
        creds
    },
    metadata: HashMap::new(),
    last_seen: None,
    status: DeviceStatus::Unknown,
};

// Register and connect...

// 2. Subscribe to sensor topics
let mut params = HashMap::new();
params.insert("action".to_string(), "mqtt_subscribe".to_string());
params.insert("device_id".to_string(), "sensor-hub-01".to_string());
params.insert("topic".to_string(), "sensors/temperature/+/data".to_string());
params.insert("qos".to_string(), "1".to_string());

state.hybrid_orchestrator
    .lock()
    .await
    .execute_connector("iot", params)
    .await?;

// Messages will be logged automatically when received
// You can extend the connector to handle messages in your application
```

## Device Status

Device status is tracked automatically:

```rust
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum DeviceStatus {
    Connected,      // Device is connected and ready
    Disconnected,   // Device is disconnected
    Error(String),  // Connection error with message
    Unknown,        // Status not yet determined
}
```

Check device status:

```rust
let mut params = HashMap::new();
params.insert("action".to_string(), "get_status".to_string());
params.insert("device_id".to_string(), "smart-light-01".to_string());

let result = state.hybrid_orchestrator
    .lock()
    .await
    .execute_connector("iot", params)
    .await?;

let status: DeviceStatus = serde_json::from_str(&result.output)?;
```

## Best Practices

### ✅ Do

- **Use MQTT-TLS for production** - Always use encrypted connections
- **Store credentials securely** - Let the connector handle encryption
- **Validate device endpoints** - Ensure endpoints are correct before registering
- **Monitor device status** - Check connection status regularly
- **Use appropriate QoS levels** - QoS 1 or 2 for critical messages
- **Clean up unused devices** - Remove devices you no longer use
- **Test in development** - Validate device connections before production

### ❌ Don't

- **Don't store credentials in code** - Use the credential storage system
- **Don't use HTTP for sensitive data** - Prefer HTTPS or MQTT-TLS
- **Don't ignore connection errors** - Handle and log errors properly
- **Don't hardcode device IDs** - Use configuration or discovery
- **Don't skip endpoint validation** - Always validate before connecting
- **Don't use QoS 0 for critical messages** - Use QoS 1 or 2

## Troubleshooting

### Common Issues

**Issue**: "Device not found"
- **Cause**: Device not registered or wrong device ID
- **Solution**: List devices with `list_devices` action, verify device ID

**Issue**: "MQTT connection failed"
- **Cause**: Invalid credentials, network issue, or broker unreachable
- **Solution**: Verify credentials, check network connectivity, test broker connection

**Issue**: "TLS handshake failed"
- **Cause**: Invalid certificates or certificate mismatch
- **Solution**: Verify CA certificate, client certificate, and client key are correct

**Issue**: "Device is not connected"
- **Cause**: Device was disconnected or connection lost
- **Solution**: Reconnect using `connect_device` action

**Issue**: "No devices discovered"
- **Cause**: Devices not on same network or mDNS not supported
- **Solution**: Ensure devices are on same network, check mDNS support, manually register devices

### Debugging

Enable detailed logging:

```rust
// Set RUST_LOG environment variable
std::env::set_var("RUST_LOG", "jamey_tools::connectors::iot=debug");
```

Check logs for:
- MQTT connection events
- HTTP request/response details
- Certificate validation errors
- Device discovery events

## Integration with Eternal Hive

The IoT connector aligns with Eternal Hive security requirements:

- **TA-QR Crypto**: Credentials encrypted with AES-256-GCM
- **mTLS Support**: Full mutual TLS for MQTT connections
- **Secure Storage**: Credentials stored in system keyring
- **Zero-Trust**: All connections authenticated and authorized

### ORCH Army Integration

IoT devices can be integrated with ORCH nodes:

```rust
// ORCH node can register IoT devices
// ORCH node can monitor device status
// ORCH node can execute device commands
// All via secure MQTT-TLS communication
```

## Related Documentation

- [Network & Web Access](network-access.md) - Web connectivity
- [Security Best Practices](security-best-practices.md) - Security guidelines
- [Agent Orchestration](orchestration.md) - Multi-agent coordination
- [TA-QR Cryptography](../security/ta-qr/README.md) - Quantum-resistant crypto

## Version History

- **v1.0.0** (2025-01-XX) - Initial IoT device connectivity
  - MQTT and MQTT-TLS support
  - HTTP/REST API support
  - Device discovery via mDNS
  - Encrypted credential storage
  - mTLS certificate support

---

**Last Updated**: 2025-01-XX  
**Version**: 1.0.0  
**Status**: ✅ Complete  
**Maintained by**: Jamey Code Team

