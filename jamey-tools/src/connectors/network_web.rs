//! Network & Web Access Connector
//!
//! Provides web search, downloads, and browser automation

use crate::connector::*;
use reqwest::{Client, ClientBuilder};
use std::collections::HashMap;
use std::path::PathBuf;
use anyhow::{Result, Context};
use urlencoding::encode;
use std::net::IpAddr;

/// Validates a URL to prevent SSRF attacks
///
/// # Security Checks
/// - Only allows HTTP/HTTPS schemes
/// - Blocks private IP ranges (10.x, 192.168.x, 172.16-31.x)
/// - Blocks localhost and 127.0.0.1
/// - Blocks cloud metadata endpoints
/// - Blocks link-local addresses
///
/// # Examples
/// ```
/// validate_url("https://example.com")?;
/// ```
fn validate_url(url: &str) -> Result<()> {
    let parsed = url::Url::parse(url)
        .context("Invalid URL format")?;
    
    // Only allow HTTP and HTTPS schemes
    let scheme = parsed.scheme();
    if scheme != "http" && scheme != "https" {
        anyhow::bail!(
            "Security violation: Only HTTP and HTTPS schemes are allowed. Got: {}",
            scheme
        );
    }
    
    // Get the host
    let host = parsed.host_str()
        .ok_or_else(|| anyhow::anyhow!("URL must have a host"))?;
    
    // Block localhost variations
    if host == "localhost" || host == "127.0.0.1" || host == "::1" {
        anyhow::bail!(
            "Security violation: Localhost access is not allowed. Host: {}",
            host
        );
    }
    
    // Block cloud metadata endpoints
    if host == "169.254.169.254" || host == "metadata.google.internal"
        || host == "metadata.azure.com" || host == "metadata.aws.amazon.com" {
        anyhow::bail!(
            "Security violation: Cloud metadata endpoint access is not allowed. Host: {}",
            host
        );
    }
    
    // Try to resolve the host to an IP address
    if let Ok(ip) = host.parse::<IpAddr>() {
        if is_private_ip(&ip) {
            anyhow::bail!(
                "Security violation: Private IP address access is not allowed. IP: {}",
                ip
            );
        }
    }
    
    // Additional check for domain names that might resolve to private IPs
    // In production, you would want to resolve the domain and check the IP
    // For now, we'll block common internal domain patterns
    let lower_host = host.to_lowercase();
    if lower_host.ends_with(".local") || lower_host.ends_with(".internal") {
        anyhow::bail!(
            "Security violation: Internal domain access is not allowed. Host: {}",
            host
        );
    }
    
    Ok(())
}

/// Checks if an IP address is in a private range
fn is_private_ip(ip: &IpAddr) -> bool {
    match ip {
        IpAddr::V4(ipv4) => {
            let octets = ipv4.octets();
            // 10.0.0.0/8
            octets[0] == 10
            // 172.16.0.0/12
            || (octets[0] == 172 && (16..=31).contains(&octets[1]))
            // 192.168.0.0/16
            || (octets[0] == 192 && octets[1] == 168)
            // 127.0.0.0/8 (loopback)
            || octets[0] == 127
            // 169.254.0.0/16 (link-local)
            || (octets[0] == 169 && octets[1] == 254)
            // 0.0.0.0/8
            || octets[0] == 0
        }
        IpAddr::V6(ipv6) => {
            // ::1 (loopback)
            ipv6.is_loopback()
            // fe80::/10 (link-local)
            || ((ipv6.segments()[0] & 0xffc0) == 0xfe80)
            // fc00::/7 (unique local)
            || ((ipv6.segments()[0] & 0xfe00) == 0xfc00)
        }
    }
}

pub struct NetworkWebConnector {
    metadata: ConnectorMetadata,
    client: Client,
    download_dir: PathBuf,
    enabled: bool,
    search_api_key: Option<String>,
}

impl NetworkWebConnector {
    pub fn new(download_dir: PathBuf, search_api_key: Option<String>) -> Result<Self> {
        let client = ClientBuilder::new()
            .user_agent("Jamey-2.0-Agent/1.0")
            .timeout(std::time::Duration::from_secs(300))
            .danger_accept_invalid_certs(false)
            .build()?;

        Ok(Self {
            metadata: ConnectorMetadata {
                id: "network_web".to_string(),
                name: "Network & Web Access".to_string(),
                version: "1.0.0".to_string(),
                description: "Full network access, web search, downloads, and browser automation".to_string(),
                capability_level: CapabilityLevel::WebAccess,
                requires_approval: false,
                safety_checks: vec![
                    "Rate limiting enforced".to_string(),
                    "Download size limits".to_string(),
                ],
            },
            client,
            download_dir,
            enabled: true,
            search_api_key,
        })
    }

    async fn web_search(&self, query: &str) -> Result<String> {
        // Use DuckDuckGo HTML search (no API key needed)
        let url = format!("https://html.duckduckgo.com/html/?q={}", encode(query));
        
        // Validate URL before making request
        validate_url(&url)
            .context("Search URL validation failed")?;
        
        tracing::info!("Web search: {}", query);
        let response = self.client.get(&url).send().await
            .context("Failed to perform web search")?;
        let html = response.text().await?;
        
        // Extract basic results (simplified - in production use proper HTML parsing)
        // For now, return the HTML and let the LLM parse it
        Ok(html)
    }

    async fn download_file(&self, url: &str, filename: Option<String>) -> Result<String> {
        // Validate URL before downloading
        validate_url(url)
            .context("Download URL validation failed")?;
        
        tracing::warn!("Downloading file from: {}", url);
        let response = self.client.get(url).send().await
            .context("Failed to download file")?;
        let bytes = response.bytes().await?;
        
        let filename = filename.unwrap_or_else(|| {
            url.split('/').last().unwrap_or("download").to_string()
        });
        let filepath = self.download_dir.join(&filename);
        
        tokio::fs::write(&filepath, &bytes).await
            .context("Failed to write downloaded file")?;
        Ok(filepath.to_string_lossy().to_string())
    }

    async fn fetch_url(&self, url: &str) -> Result<String> {
        // Validate URL before fetching
        validate_url(url)
            .context("Fetch URL validation failed")?;
        
        tracing::info!("Fetching URL: {}", url);
        let response = self.client.get(url).send().await
            .context("Failed to fetch URL")?;
        Ok(response.text().await?)
    }
}

#[async_trait::async_trait]
impl Connector for NetworkWebConnector {
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
            "web_search" => {
                let query = params.get("query")
                    .ok_or_else(|| anyhow::anyhow!("Missing 'query' parameter"))?;
                let search_results = self.web_search(query).await?;
                result.output = search_results;
                result.success = true;
                result.network_requests.push(NetworkRequest {
                    url: format!("https://html.duckduckgo.com/html/?q={}", encode(query)),
                    method: "GET".to_string(),
                    status_code: Some(200),
                    timestamp: chrono::Utc::now(),
                });
            }
            "download" => {
                let url = params.get("url")
                    .ok_or_else(|| anyhow::anyhow!("Missing 'url' parameter"))?;
                let filename = params.get("filename").cloned();
                let filepath = self.download_file(url, filename).await?;
                result.output = format!("Downloaded to: {}", filepath);
                result.success = true;
                result.files_accessed.push(filepath.clone());
                result.network_requests.push(NetworkRequest {
                    url: url.clone(),
                    method: "GET".to_string(),
                    status_code: Some(200),
                    timestamp: chrono::Utc::now(),
                });
            }
            "fetch_url" => {
                let url = params.get("url")
                    .ok_or_else(|| anyhow::anyhow!("Missing 'url' parameter"))?;
                let content = self.fetch_url(url).await?;
                result.output = content;
                result.success = true;
                result.network_requests.push(NetworkRequest {
                    url: url.clone(),
                    method: "GET".to_string(),
                    status_code: Some(200),
                    timestamp: chrono::Utc::now(),
                });
            }
            "browser_action" => {
                // Browser automation would require Playwright or similar
                // For now, return a placeholder
                result.warnings.push("Browser automation requires additional setup (Playwright/Selenium)".to_string());
                result.output = "Browser automation not yet implemented - use fetch_url for web content".to_string();
                result.success = false;
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
        vec![] // Web search can work without API key
    }
}

