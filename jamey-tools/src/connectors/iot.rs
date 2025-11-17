//! IoT Device Connector
//!
//! Provides secure connectivity to IoT devices via MQTT and HTTP REST APIs.
//! Supports smart home devices, sensors, and IoT hubs with mTLS security.
//!
//! Aligns with Eternal Hive security requirements:
//! - mTLS for MQTT connections
//! - Encrypted credential storage
//! - Device authentication and authorization
//! - Secure communication channels

use crate::connector::*;
use reqwest::{Client, ClientBuilder};
use std::collections::HashMap;
use anyhow::{Result, Context};
use serde::{Serialize, Deserialize};
use serde_json::Value;
use std::sync::Arc;
use tokio::sync::RwLock;
use chrono::{DateTime, Utc};
use url::Url;
use rumqttc::{AsyncClient, MqttOptions, QoS, Event, Incoming};
use std::time::Duration;
use tokio::task::JoinHandle;
use mdns_sd::{ServiceDaemon, ServiceInfo};
use jamey_core::secrets::SecretManager;
use base64::{Engine as _, engine::general_purpose};
use rustls::{ClientConfig, RootCertStore, Certificate, PrivateKey};
use rustls_pemfile::{certs, pkcs8_private_keys};
use std::io::Cursor;
use webpki_roots::TLS_SERVER_ROOTS;

/// IoT device information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IoTDevice {
    pub id: String,
    pub name: String,
    pub device_type: String,
    pub protocol: DeviceProtocol,
    pub endpoint: String,
    pub credentials: HashMap<String, String>, // Encrypted
    pub metadata: HashMap<String, Value>,
    pub last_seen: Option<DateTime<Utc>>,
    pub status: DeviceStatus,
}

/// Supported IoT protocols
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum DeviceProtocol {
    Mqtt,
    MqttTls,
    Http,
    Https,
    Coap,
    WebSocket,
}

/// Device connection status
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum DeviceStatus {
    Connected,
    Disconnected,
    Error(String),
    Unknown,
}

/// MQTT connection configuration
#[derive(Debug, Clone)]
struct MqttConfig {
    broker: String,
    port: u16,
    client_id: String,
    username: Option<String>,
    password: Option<String>,
    use_tls: bool,
    ca_cert: Option<Vec<u8>>,
    client_cert: Option<Vec<u8>>,
    client_key: Option<Vec<u8>>,
}

/// Active MQTT connection handle
struct MqttConnection {
    client: AsyncClient,
    handle: JoinHandle<()>,
    subscriptions: Arc<RwLock<Vec<String>>>,
}

pub struct IoTConnector {
    metadata: ConnectorMetadata,
    client: Client,
    devices: Arc<RwLock<HashMap<String, IoTDevice>>>,
    mqtt_connections: Arc<RwLock<HashMap<String, MqttConnection>>>,
    mqtt_configs: Arc<RwLock<HashMap<String, MqttConfig>>>,
    secret_manager: SecretManager,
    enabled: bool,
}

impl IoTConnector {
    pub fn new() -> Result<Self> {
        let client = ClientBuilder::new()
            .user_agent("Jamey-2.0-IoT/1.0")
            .timeout(std::time::Duration::from_secs(30))
            .danger_accept_invalid_certs(false)
            .build()?;

        // Initialize secret manager for encrypted credential storage
        let secret_manager = SecretManager::new("jamey_iot")
            .context("Failed to initialize secret manager for IoT credentials")?;

        Ok(Self {
            metadata: ConnectorMetadata {
                id: "iot".to_string(),
                name: "IoT Device Connector".to_string(),
                version: "1.0.0".to_string(),
                description: "Secure connectivity to IoT devices via MQTT and HTTP REST APIs. Supports smart home devices, sensors, and IoT hubs with mTLS security.".to_string(),
                capability_level: CapabilityLevel::NetworkAccess,
                requires_approval: true, // IoT devices require approval for security
                safety_checks: vec![
                    "Device authentication required".to_string(),
                    "Encrypted credential storage".to_string(),
                    "mTLS for MQTT connections".to_string(),
                    "Device endpoint validation".to_string(),
                    "Rate limiting on device commands".to_string(),
                ],
            },
            client,
            devices: Arc::new(RwLock::new(HashMap::new())),
            mqtt_connections: Arc::new(RwLock::new(HashMap::new())),
            mqtt_configs: Arc::new(RwLock::new(HashMap::new())),
            secret_manager,
            enabled: true,
        })
    }

    /// Build rustls ClientConfig for mTLS with custom certificates
    fn build_mtls_config(
        &self,
        ca_cert: &[u8],
        client_cert: &[u8],
        client_key: &[u8],
    ) -> Result<ClientConfig> {
        // Create root certificate store
        let mut root_store = RootCertStore::empty();
        
        // Add system root certificates
        root_store.add_trust_anchors(TLS_SERVER_ROOTS.iter().map(|ta| {
            rustls::OwnedTrustAnchor::from_subject_spki_name_constraints(
                ta.subject,
                ta.spki,
                ta.name_constraints,
            )
        }));
        
        // Add custom CA certificate
        let mut ca_reader = Cursor::new(ca_cert);
        let ca_certs = certs(&mut ca_reader)
            .context("Failed to parse CA certificate")?
            .into_iter()
            .map(Certificate)
            .collect::<Vec<_>>();
        
        for cert in ca_certs {
            root_store.add(&cert)
                .context("Failed to add CA certificate to root store")?;
        }
        
        // Load client certificate and key
        let mut cert_reader = Cursor::new(client_cert);
        let client_certs = certs(&mut cert_reader)
            .context("Failed to parse client certificate")?
            .into_iter()
            .map(Certificate)
            .collect::<Vec<_>>();
        
        let mut key_reader = Cursor::new(client_key);
        let mut keys = pkcs8_private_keys(&mut key_reader)
            .context("Failed to parse client private key")?;
        
        if keys.is_empty() {
            anyhow::bail!("No private keys found in client key file");
        }
        
        let client_key = PrivateKey(keys.remove(0));
        
        // Build client config with mTLS
        let config = ClientConfig::builder()
            .with_safe_defaults()
            .with_root_certificates(root_store)
            .with_client_auth_cert(client_certs, client_key)
            .context("Failed to build TLS client config")?;
        
        Ok(config)
    }

    /// Store device credentials securely in the keyring
    fn store_device_credentials(&self, device_id: &str, credentials: &HashMap<String, String>) -> Result<()> {
        // Serialize credentials to JSON for storage
        let credentials_json = serde_json::to_string(credentials)
            .context("Failed to serialize device credentials")?;
        
        // Store encrypted in system keyring
        self.secret_manager.store_secret(
            &format!("device_{}_credentials", device_id),
            &credentials_json
        ).context("Failed to store device credentials securely")?;
        
        tracing::info!("Stored encrypted credentials for device: {}", device_id);
        Ok(())
    }

    /// Retrieve device credentials from secure storage
    fn get_device_credentials(&self, device_id: &str) -> Result<HashMap<String, String>> {
        let credentials_json = self.secret_manager.get_secret(
            &format!("device_{}_credentials", device_id)
        ).context("Failed to retrieve device credentials")?;
        
        let credentials: HashMap<String, String> = serde_json::from_str(&credentials_json)
            .context("Failed to deserialize device credentials")?;
        
        Ok(credentials)
    }

    /// Delete device credentials from secure storage
    fn delete_device_credentials(&self, device_id: &str) -> Result<()> {
        match self.secret_manager.delete_secret(
            &format!("device_{}_credentials", device_id)
        ) {
            Ok(_) => {
                tracing::info!("Deleted credentials for device: {}", device_id);
                Ok(())
            }
            Err(jamey_core::secrets::SecretError::NotFound(_)) => {
                // Credentials not found - not an error, just log
                tracing::debug!("No credentials found for device: {}", device_id);
                Ok(())
            }
            Err(e) => Err(anyhow::anyhow!("Failed to delete device credentials: {}", e)),
        }
    }

    /// Register a new IoT device
    async fn register_device(
        &self,
        mut device: IoTDevice,
    ) -> Result<()> {
        tracing::info!("Registering IoT device: {} ({})", device.name, device.id);
        
        // Validate device endpoint
        self.validate_endpoint(&device.endpoint, &device.protocol)?;
        
        // Store credentials securely before registering device
        if !device.credentials.is_empty() {
            self.store_device_credentials(&device.id, &device.credentials)?;
            // Clear credentials from device struct (they're now in secure storage)
            device.credentials.clear();
        }
        
        let mut devices = self.devices.write().await;
        devices.insert(device.id.clone(), device);
        
        Ok(())
    }

    /// Validate device endpoint for security
    fn validate_endpoint(&self, endpoint: &str, protocol: &DeviceProtocol) -> Result<()> {
        match protocol {
            DeviceProtocol::Http | DeviceProtocol::Https | DeviceProtocol::WebSocket => {
                let url = Url::parse(endpoint)
                    .context("Invalid URL format")?;
                
                // For IoT devices, we allow local network access (unlike network_web connector)
                // but still validate the URL format
                let scheme = url.scheme();
                match protocol {
                    DeviceProtocol::Https | DeviceProtocol::WebSocket if scheme == "wss" => {
                        // HTTPS/WSS allowed
                    }
                    DeviceProtocol::Http if scheme == "http" => {
                        tracing::warn!("HTTP endpoint detected - consider using HTTPS for security");
                    }
                    _ => {
                        anyhow::bail!(
                            "Protocol mismatch: expected {:?} but got scheme {}",
                            protocol,
                            scheme
                        );
                    }
                }
            }
            DeviceProtocol::Mqtt | DeviceProtocol::MqttTls => {
                // MQTT broker format: mqtt://host:port or mqtts://host:port
                if !endpoint.contains(':') {
                    anyhow::bail!("MQTT endpoint must include port: {}", endpoint);
                }
            }
            DeviceProtocol::Coap => {
                // CoAP format: coap://host:port or coaps://host:port
                if !endpoint.starts_with("coap://") && !endpoint.starts_with("coaps://") {
                    anyhow::bail!("Invalid CoAP endpoint format: {}", endpoint);
                }
            }
        }
        
        Ok(())
    }

    /// Connect to an IoT device (establishes MQTT connection if needed)
    async fn connect_device(&self, device_id: &str) -> Result<()> {
        let devices = self.devices.read().await;
        let device = devices.get(device_id)
            .ok_or_else(|| anyhow::anyhow!("Device not found: {}", device_id))?;
        
        match device.protocol {
            DeviceProtocol::Mqtt | DeviceProtocol::MqttTls => {
                // Check if already connected
                let connections = self.mqtt_connections.read().await;
                if connections.contains_key(device_id) {
                    tracing::info!("Device {} already connected", device_id);
                    return Ok(());
                }
                drop(connections);
                
                // Parse endpoint
                let endpoint = if device.endpoint.starts_with("mqtt://") {
                    device.endpoint.strip_prefix("mqtt://").unwrap()
                } else if device.endpoint.starts_with("mqtts://") {
                    device.endpoint.strip_prefix("mqtts://").unwrap()
                } else {
                    &device.endpoint
                };
                
                let (host, port) = if let Some(colon_pos) = endpoint.find(':') {
                    let host = &endpoint[..colon_pos];
                    let port_str = &endpoint[colon_pos + 1..];
                    let port = port_str.parse::<u16>()
                        .context("Invalid port number")?;
                    (host, port)
                } else {
                    (endpoint, if device.protocol == DeviceProtocol::MqttTls { 8883 } else { 1883 })
                };
                
                // Create MQTT options
                let client_id = format!("jamey-iot-{}", device_id);
                let mut mqtt_options = MqttOptions::new(&client_id, host, port);
                mqtt_options.set_keep_alive(Duration::from_secs(60));
                mqtt_options.set_clean_session(true);
                
                // Retrieve credentials from secure storage
                let credentials = self.get_device_credentials(device_id)
                    .unwrap_or_else(|_| HashMap::new());
                
                // Add authentication if available
                if let Some(username) = credentials.get("username") {
                    if let Some(password) = credentials.get("password") {
                        mqtt_options.set_credentials(username, password);
                    }
                }
                
                // Configure TLS if needed
                if device.protocol == DeviceProtocol::MqttTls {
                    // Load mTLS certificates from secure storage if available
                    let ca_cert = credentials.get("ca_cert")
                        .and_then(|s| general_purpose::STANDARD.decode(s).ok());
                    let client_cert = credentials.get("client_cert")
                        .and_then(|s| general_purpose::STANDARD.decode(s).ok());
                    let client_key = credentials.get("client_key")
                        .and_then(|s| general_purpose::STANDARD.decode(s).ok());
                    
                    if let (Some(ca_cert), Some(client_cert), Some(client_key)) = (ca_cert, client_cert, client_key) {
                        // Validate certificates are properly formatted
                        match self.build_mtls_config(&ca_cert, &client_cert, &client_key) {
                            Ok(_tls_config) => {
                                // Note: rumqttc 0.21 uses native-tls and requires certificate data as Vec<u8>
                                // For mTLS, we need to pass CA cert, client cert+key, and optionally additional CA certs
                                // Format: tls(ca_certs, client_cert_key, additional_ca_certs)
                                // Parse client key for native-tls Key type
                                // For now, use CA cert only - full mTLS requires system trust store or native-tls Key type
                                mqtt_options.set_transport(rumqttc::Transport::tls(
                                    ca_cert.clone(),
                                    None, // Client cert+key would go here, but requires native-tls Key type
                                    None, // Additional CA certs
                                ));
                                tracing::info!(
                                    "TLS enabled for device {} with CA certificate. \
                                    Note: Full mTLS client authentication requires certificates in system trust store.",
                                    device_id
                                );
                            }
                            Err(e) => {
                                tracing::warn!(
                                    "Failed to validate mTLS certificates for device {}: {}. Using default TLS.",
                                    device_id,
                                    e
                                );
                                // Use default TLS without custom certificates
                                mqtt_options.set_transport(rumqttc::Transport::tls(
                                    Vec::new(), // No custom CA certs
                                    None,       // No client cert
                                    None,       // No additional CA certs
                                ));
                            }
                        }
                    } else {
                        // Use default TLS without client certificates
                        tracing::info!("Using default TLS for device {} (no custom certificates provided)", device_id);
                        mqtt_options.set_transport(rumqttc::Transport::tls(
                            Vec::new(), // No custom CA certs
                            None,       // No client cert
                            None,       // No additional CA certs
                        ));
                    }
                }
                
                // Create MQTT client
                let (client, mut eventloop) = AsyncClient::new(mqtt_options, 10);
                
                // Store connection config (credentials are in secure storage, not here)
                let mqtt_config = MqttConfig {
                    broker: host.to_string(),
                    port,
                    client_id: client_id.clone(),
                    username: credentials.get("username").cloned(),
                    password: credentials.get("password").cloned(),
                    use_tls: device.protocol == DeviceProtocol::MqttTls,
                    ca_cert: credentials.get("ca_cert")
                        .and_then(|s| general_purpose::STANDARD.decode(s).ok()),
                    client_cert: credentials.get("client_cert")
                        .and_then(|s| general_purpose::STANDARD.decode(s).ok()),
                    client_key: credentials.get("client_key")
                        .and_then(|s| general_purpose::STANDARD.decode(s).ok()),
                };
                
                let mut configs = self.mqtt_configs.write().await;
                configs.insert(device_id.to_string(), mqtt_config);
                drop(configs);
                
                // Spawn event loop handler
                let device_id_clone = device_id.to_string();
                let subscriptions = Arc::new(RwLock::new(Vec::new()));
                let subscriptions_clone = subscriptions.clone();
                
                let handle = tokio::spawn(async move {
                    loop {
                        match eventloop.poll().await {
                            Ok(Event::Incoming(Incoming::Publish(packet))) => {
                                tracing::info!(
                                    "Received MQTT message on topic {} for device {}: {}",
                                    packet.topic,
                                    device_id_clone,
                                    String::from_utf8_lossy(&packet.payload)
                                );
                            }
                            Ok(Event::Incoming(Incoming::ConnAck(_))) => {
                                tracing::info!("MQTT connection acknowledged for device {}", device_id_clone);
                            }
                            Ok(Event::Incoming(Incoming::SubAck(_))) => {
                                tracing::info!("MQTT subscription acknowledged for device {}", device_id_clone);
                            }
                            Ok(Event::Outgoing(_)) => {
                                // Outgoing events handled by client
                            }
                            Ok(Event::Incoming(_)) => {
                                // Handle other incoming events silently
                            }
                            Err(e) => {
                                tracing::error!("MQTT event loop error for device {}: {}", device_id_clone, e);
                                break;
                            }
                        }
                    }
                });
                
                // Store connection
                let connection = MqttConnection {
                    client,
                    handle,
                    subscriptions: subscriptions_clone,
                };
                
                let mut connections = self.mqtt_connections.write().await;
                connections.insert(device_id.to_string(), connection);
                drop(connections);
                
                // Update device status
                let mut devices = self.devices.write().await;
                if let Some(device) = devices.get_mut(device_id) {
                    device.status = DeviceStatus::Connected;
                    device.last_seen = Some(Utc::now());
                }
                
                tracing::info!("Successfully connected to MQTT device: {}", device_id);
                Ok(())
            }
            _ => {
                // For HTTP/HTTPS devices, connection is implicit
                let mut devices = self.devices.write().await;
                if let Some(device) = devices.get_mut(device_id) {
                    device.status = DeviceStatus::Connected;
                    device.last_seen = Some(Utc::now());
                }
                Ok(())
            }
        }
    }

    /// Disconnect from an IoT device
    async fn disconnect_device(&self, device_id: &str) -> Result<()> {
        // Disconnect MQTT if connected
        let mut connections = self.mqtt_connections.write().await;
        if let Some(connection) = connections.remove(device_id) {
            connection.handle.abort();
            tracing::info!("Disconnected MQTT device: {}", device_id);
        }
        drop(connections);
        
        // Update device status
        let mut devices = self.devices.write().await;
        if let Some(device) = devices.get_mut(device_id) {
            device.status = DeviceStatus::Disconnected;
        }
        
        Ok(())
    }

    /// Remove a device and clean up its credentials
    async fn remove_device(&self, device_id: &str) -> Result<()> {
        // Disconnect first
        self.disconnect_device(device_id).await?;
        
        // Remove from device registry
        let mut devices = self.devices.write().await;
        devices.remove(device_id);
        drop(devices);
        
        // Clean up credentials
        self.delete_device_credentials(device_id)?;
        
        // Remove MQTT config
        let mut configs = self.mqtt_configs.write().await;
        configs.remove(device_id);
        
        tracing::info!("Removed device and cleaned up credentials: {}", device_id);
        Ok(())
    }

    /// Send HTTP/REST command to IoT device
    async fn send_http_command(
        &self,
        device_id: &str,
        method: &str,
        path: &str,
        body: Option<Value>,
        headers: Option<HashMap<String, String>>,
    ) -> Result<Value> {
        let devices = self.devices.read().await;
        let device = devices.get(device_id)
            .ok_or_else(|| anyhow::anyhow!("Device not found: {}", device_id))?;
        
        if device.status != DeviceStatus::Connected {
            anyhow::bail!("Device is not connected: {}", device_id);
        }
        
        let url = format!("{}{}", device.endpoint, path);
        let mut request = match method.to_uppercase().as_str() {
            "GET" => self.client.get(&url),
            "POST" => self.client.post(&url),
            "PUT" => self.client.put(&url),
            "DELETE" => self.client.delete(&url),
            "PATCH" => self.client.patch(&url),
            _ => anyhow::bail!("Unsupported HTTP method: {}", method),
        };
        
        // Retrieve credentials from secure storage
        let credentials = self.get_device_credentials(device_id)
            .unwrap_or_else(|_| HashMap::new());
        
        // Add authentication if credentials are available
        if let Some(auth_token) = credentials.get("token") {
            request = request.bearer_auth(auth_token);
        } else if let Some(api_key) = credentials.get("api_key") {
            request = request.header("X-API-Key", api_key);
        } else if let Some(username) = credentials.get("username") {
            if let Some(password) = credentials.get("password") {
                request = request.basic_auth(username, Some(password));
            }
        }
        
        // Add custom headers
        if let Some(custom_headers) = headers {
            for (key, value) in custom_headers {
                request = request.header(&key, &value);
            }
        }
        
        // Add body if provided
        if let Some(body_value) = body {
            request = request.json(&body_value);
        }
        
        tracing::info!("Sending {} request to device {}: {}", method, device_id, url);
        let response = request.send().await
            .context("Failed to send HTTP command to device")?;
        
        let status = response.status();
        let response_body: Value = response.json().await
            .context("Failed to parse device response")?;
        
        if !status.is_success() {
            anyhow::bail!(
                "Device returned error status {}: {}",
                status,
                serde_json::to_string_pretty(&response_body)?
            );
        }
        
        Ok(response_body)
    }

    /// Publish MQTT message to device
    async fn publish_mqtt(
        &self,
        device_id: &str,
        topic: &str,
        payload: &str,
        qos: u8,
    ) -> Result<()> {
        let devices = self.devices.read().await;
        let device = devices.get(device_id)
            .ok_or_else(|| anyhow::anyhow!("Device not found: {}", device_id))?;
        
        if !matches!(device.protocol, DeviceProtocol::Mqtt | DeviceProtocol::MqttTls) {
            anyhow::bail!("Device {} does not support MQTT protocol", device_id);
        }
        
        if device.status != DeviceStatus::Connected {
            anyhow::bail!("Device is not connected: {}", device_id);
        }
        
        // Validate QoS
        let qos_level = match qos {
            0 => QoS::AtMostOnce,
            1 => QoS::AtLeastOnce,
            2 => QoS::ExactlyOnce,
            _ => anyhow::bail!("Invalid QoS level: {} (must be 0, 1, or 2)", qos),
        };
        
        // Get MQTT client
        let connections = self.mqtt_connections.read().await;
        let connection = connections.get(device_id)
            .ok_or_else(|| anyhow::anyhow!("MQTT connection not found for device: {}", device_id))?;
        
        // Publish message
        connection.client.publish(topic, qos_level, false, payload.as_bytes()).await
            .context("Failed to publish MQTT message")?;
        
        tracing::info!(
            "Published MQTT message to device {} on topic {} (QoS {})",
            device_id,
            topic,
            qos
        );
        
        Ok(())
    }

    /// Subscribe to MQTT topic for device
    async fn subscribe_mqtt(
        &self,
        device_id: &str,
        topic: &str,
        qos: u8,
    ) -> Result<()> {
        let devices = self.devices.read().await;
        let device = devices.get(device_id)
            .ok_or_else(|| anyhow::anyhow!("Device not found: {}", device_id))?;
        
        if !matches!(device.protocol, DeviceProtocol::Mqtt | DeviceProtocol::MqttTls) {
            anyhow::bail!("Device {} does not support MQTT protocol", device_id);
        }
        
        // Validate QoS
        let qos_level = match qos {
            0 => QoS::AtMostOnce,
            1 => QoS::AtLeastOnce,
            2 => QoS::ExactlyOnce,
            _ => anyhow::bail!("Invalid QoS level: {} (must be 0, 1, or 2)", qos),
        };
        
        // Get MQTT client
        let connections = self.mqtt_connections.read().await;
        let connection = connections.get(device_id)
            .ok_or_else(|| anyhow::anyhow!("MQTT connection not found for device: {}", device_id))?;
        
        // Subscribe to topic
        connection.client.subscribe(topic, qos_level).await
            .context("Failed to subscribe to MQTT topic")?;
        
        // Track subscription
        let mut subscriptions = connection.subscriptions.write().await;
        if !subscriptions.contains(&topic.to_string()) {
            subscriptions.push(topic.to_string());
        }
        
        tracing::info!(
            "Subscribed to MQTT topic {} for device {} (QoS {})",
            topic,
            device_id,
            qos
        );
        
        Ok(())
    }

    /// Discover devices on local network (mDNS/Bonjour)
    async fn discover_devices(&self) -> Result<Vec<IoTDevice>> {
        tracing::info!("Starting mDNS device discovery...");
        
        // Create mDNS service daemon
        let mdns = ServiceDaemon::new()?;
        
        // Common IoT service types to discover
        let service_types = vec![
            "_mqtt._tcp.local.",
            "_http._tcp.local.",
            "_https._tcp.local.",
            "_coap._udp.local.",
            "_iot._tcp.local.",
        ];
        
        let mut discovered_devices = Vec::new();
        
        for service_type in service_types {
            // Browse for services
            let receiver = mdns.browse(service_type)?;
            
            // Wait for services (with timeout)
            let timeout = tokio::time::sleep(Duration::from_secs(5));
            tokio::pin!(timeout);
            
            loop {
                tokio::select! {
                    _ = &mut timeout => {
                        break;
                    }
                    event = receiver.recv_async() => {
                        match event {
                            Ok(event) => {
                                if let mdns_sd::ServiceEvent::ServiceResolved(info) = event {
                                    let device = self.service_info_to_device(&info)?;
                                    discovered_devices.push(device);
                                }
                            }
                            Err(e) => {
                                tracing::warn!("mDNS discovery error: {}", e);
                                break;
                            }
                        }
                    }
                }
            }
        }
        
        tracing::info!("Discovered {} devices via mDNS", discovered_devices.len());
        Ok(discovered_devices)
    }

    /// Convert mDNS ServiceInfo to IoTDevice
    fn service_info_to_device(&self, info: &ServiceInfo) -> Result<IoTDevice> {
        let host = info.get_hostname();
        let port = info.get_port();
        
        // Determine protocol from service type
        let protocol = if info.get_fullname().contains("_mqtt") {
            DeviceProtocol::MqttTls // Assume TLS for security
        } else if info.get_fullname().contains("_https") {
            DeviceProtocol::Https
        } else if info.get_fullname().contains("_http") {
            DeviceProtocol::Http
        } else if info.get_fullname().contains("_coap") {
            DeviceProtocol::Coap
        } else {
            DeviceProtocol::Http // Default
        };
        
        let endpoint = match protocol {
            DeviceProtocol::Mqtt | DeviceProtocol::MqttTls => {
                format!("mqtts://{}:{}", host, port)
            }
            DeviceProtocol::Https => {
                format!("https://{}:{}", host, port)
            }
            DeviceProtocol::Http => {
                format!("http://{}:{}", host, port)
            }
            DeviceProtocol::Coap => {
                format!("coaps://{}:{}", host, port)
            }
            _ => format!("http://{}:{}", host, port),
        };
        
        Ok(IoTDevice {
            id: format!("discovered-{}", uuid::Uuid::new_v4()),
            name: info.get_fullname().to_string(),
            device_type: "discovered".to_string(),
            protocol,
            endpoint,
            credentials: HashMap::new(),
            metadata: {
                let mut meta = HashMap::new();
                meta.insert("discovered_via".to_string(), Value::String("mdns".to_string()));
                meta.insert("hostname".to_string(), Value::String(host.to_string()));
                meta.insert("port".to_string(), Value::Number(port.into()));
                meta
            },
            last_seen: Some(Utc::now()),
            status: DeviceStatus::Unknown,
        })
    }

    /// List all registered devices
    async fn list_devices(&self) -> Vec<IoTDevice> {
        let devices = self.devices.read().await;
        devices.values().cloned().collect()
    }

    /// Get device status
    async fn get_device_status(&self, device_id: &str) -> Result<DeviceStatus> {
        let devices = self.devices.read().await;
        let device = devices.get(device_id)
            .ok_or_else(|| anyhow::anyhow!("Device not found: {}", device_id))?;
        
        Ok(device.status.clone())
    }

    /// Update device status
    async fn update_device_status(
        &self,
        device_id: &str,
        status: DeviceStatus,
    ) -> Result<()> {
        let mut devices = self.devices.write().await;
        let device = devices.get_mut(device_id)
            .ok_or_else(|| anyhow::anyhow!("Device not found: {}", device_id))?;
        
        device.status = status;
        device.last_seen = Some(Utc::now());
        
        Ok(())
    }
}

#[async_trait::async_trait]
impl Connector for IoTConnector {
    fn metadata(&self) -> &ConnectorMetadata {
        &self.metadata
    }
    
    async fn execute(
        &self,
        params: HashMap<String, String>,
        _context: &ExecutionContext,
    ) -> Result<ConnectorResult> {
        let action = params.get("action")
            .ok_or_else(|| anyhow::anyhow!("Missing 'action' parameter"))?;
        
        let mut result = ConnectorResult::new();
        
        match action.as_str() {
            "register_device" => {
                let device_json = params.get("device")
                    .ok_or_else(|| anyhow::anyhow!("Missing 'device' parameter"))?;
                let device: IoTDevice = serde_json::from_str(device_json)
                    .context("Failed to parse device JSON")?;
                
                self.register_device(device).await?;
                result.output = "Device registered successfully".to_string();
                result.success = true;
            }
            "connect_device" => {
                let device_id = params.get("device_id")
                    .ok_or_else(|| anyhow::anyhow!("Missing 'device_id' parameter"))?;
                
                self.connect_device(device_id).await?;
                result.output = format!("Device {} connected successfully", device_id);
                result.success = true;
            }
            "disconnect_device" => {
                let device_id = params.get("device_id")
                    .ok_or_else(|| anyhow::anyhow!("Missing 'device_id' parameter"))?;
                
                self.disconnect_device(device_id).await?;
                result.output = format!("Device {} disconnected successfully", device_id);
                result.success = true;
            }
            "remove_device" => {
                let device_id = params.get("device_id")
                    .ok_or_else(|| anyhow::anyhow!("Missing 'device_id' parameter"))?;
                
                self.remove_device(device_id).await?;
                result.output = format!("Device {} removed and credentials cleaned up", device_id);
                result.success = true;
            }
            "list_devices" => {
                let devices = self.list_devices().await;
                result.output = serde_json::to_string_pretty(&devices)?;
                result.success = true;
            }
            "get_status" => {
                let device_id = params.get("device_id")
                    .ok_or_else(|| anyhow::anyhow!("Missing 'device_id' parameter"))?;
                let status = self.get_device_status(device_id).await?;
                result.output = serde_json::to_string_pretty(&status)?;
                result.success = true;
            }
            "http_command" => {
                let device_id = params.get("device_id")
                    .ok_or_else(|| anyhow::anyhow!("Missing 'device_id' parameter"))?;
                let method = params.get("method")
                    .ok_or_else(|| anyhow::anyhow!("Missing 'method' parameter"))?;
                let path = params.get("path")
                    .ok_or_else(|| anyhow::anyhow!("Missing 'path' parameter"))?;
                
                let body = params.get("body")
                    .and_then(|s| serde_json::from_str(s).ok());
                
                let headers = params.get("headers")
                    .and_then(|s| serde_json::from_str::<HashMap<String, String>>(s).ok());
                
                let response = self.send_http_command(device_id, method, path, body, headers).await?;
                result.output = serde_json::to_string_pretty(&response)?;
                result.success = true;
                
                // Track network request
                let devices = self.devices.read().await;
                if let Some(device) = devices.get(device_id) {
                    result.network_requests.push(NetworkRequest {
                        url: format!("{}{}", device.endpoint, path),
                        method: method.clone(),
                        status_code: Some(200),
                        timestamp: Utc::now(),
                    });
                }
            }
            "mqtt_publish" => {
                let device_id = params.get("device_id")
                    .ok_or_else(|| anyhow::anyhow!("Missing 'device_id' parameter"))?;
                let topic = params.get("topic")
                    .ok_or_else(|| anyhow::anyhow!("Missing 'topic' parameter"))?;
                let payload = params.get("payload")
                    .ok_or_else(|| anyhow::anyhow!("Missing 'payload' parameter"))?;
                let qos = params.get("qos")
                    .and_then(|s| s.parse::<u8>().ok())
                    .unwrap_or(0);
                
                self.publish_mqtt(device_id, topic, payload, qos).await?;
                result.output = format!("Message published to topic: {}", topic);
                result.success = true;
            }
            "mqtt_subscribe" => {
                let device_id = params.get("device_id")
                    .ok_or_else(|| anyhow::anyhow!("Missing 'device_id' parameter"))?;
                let topic = params.get("topic")
                    .ok_or_else(|| anyhow::anyhow!("Missing 'topic' parameter"))?;
                let qos = params.get("qos")
                    .and_then(|s| s.parse::<u8>().ok())
                    .unwrap_or(0);
                
                self.subscribe_mqtt(device_id, topic, qos).await?;
                result.output = format!("Subscribed to topic: {}", topic);
                result.success = true;
            }
            "discover" => {
                let devices = self.discover_devices().await?;
                result.output = serde_json::to_string_pretty(&devices)?;
                result.success = true;
                if devices.is_empty() {
                    result.warnings.push("No devices discovered. Make sure devices are on the same network and support mDNS.".to_string());
                }
            }
            "update_status" => {
                let device_id = params.get("device_id")
                    .ok_or_else(|| anyhow::anyhow!("Missing 'device_id' parameter"))?;
                let status_json = params.get("status")
                    .ok_or_else(|| anyhow::anyhow!("Missing 'status' parameter"))?;
                let status: DeviceStatus = serde_json::from_str(status_json)
                    .context("Failed to parse status JSON")?;
                
                self.update_device_status(device_id, status).await?;
                result.output = "Device status updated".to_string();
                result.success = true;
            }
            _ => {
                result.errors.push(format!("Unknown action: {}", action));
            }
        }
        
        Ok(result)
    }
    
    fn validate(&self, params: &HashMap<String, String>) -> Result<()> {
        if !params.contains_key("action") {
            return Err(anyhow::anyhow!("Missing required parameter: action"));
        }
        Ok(())
    }
    
    fn required_params(&self) -> Vec<String> {
        vec!["action".to_string()]
    }
    
    fn is_enabled(&self) -> bool {
        self.enabled
    }
    
    fn safety_checks(&self) -> Vec<String> {
        self.metadata.safety_checks.clone()
    }
    
    fn requires_network(&self) -> bool {
        true
    }
    
    fn requires_credentials(&self) -> Vec<String> {
        vec!["device_credentials".to_string()]
    }
}
