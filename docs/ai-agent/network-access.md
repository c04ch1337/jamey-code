# Network & Web Access Guide

The Network & Web Access connector provides web search, file downloads, and URL fetching capabilities with comprehensive SSRF (Server-Side Request Forgery) protection. This enables Jamey 2.0 to gather information from the internet safely.

> ⚠️ **Important**: Network operations can expose the system to external threats. All URLs are validated and private networks are blocked.

## Table of Contents

- [Overview](#overview)
- [Configuration](#configuration)
- [Web Search](#web-search)
- [File Downloads](#file-downloads)
- [URL Fetching](#url-fetching)
- [SSRF Protection](#ssrf-protection)
- [Usage Examples](#usage-examples)
- [Best Practices](#best-practices)
- [Troubleshooting](#troubleshooting)
- [Security Considerations](#security-considerations)

## Overview

**Connector ID**: `network_web`  
**Capability Level**: `WebAccess`  
**Requires Approval**: ❌ No (but operations are logged)  
**Source**: [`jamey-tools/src/connectors/network_web.rs`](../../jamey-tools/src/connectors/network_web.rs)

### Key Features

- ✅ **Web Search**: Search via DuckDuckGo (no API key required)
- ✅ **File Downloads**: Download files with size limits
- ✅ **URL Fetching**: Retrieve content from URLs
- ✅ **SSRF Protection**: Blocks private IPs and cloud metadata
- ✅ **Rate Limiting**: Prevents abuse
- ✅ **Timeout Control**: Configurable request timeouts

### Capabilities

| Feature | Description | Security Control |
|---------|-------------|------------------|
| Web Search | Search the internet | Rate limiting |
| Download File | Download from URLs | SSRF protection, size limits |
| Fetch URL | Get URL content | SSRF protection |
| Browser Action | Browser automation | Not yet implemented |

## Configuration

### Environment Variables

Add to your `.env` file:

```bash
# Download directory
DOWNLOAD_DIR=./downloads

# Optional: Web search API key (DuckDuckGo works without one)
WEB_SEARCH_API_KEY=

# Optional: GitHub token for API access
GITHUB_TOKEN=your_github_token

# Optional: LinkedIn token
LINKEDIN_TOKEN=your_linkedin_token
```

### Runtime Configuration

```rust
use jamey_runtime::config::RuntimeConfig;

let config = RuntimeConfig::from_env()?;

println!("Download dir: {:?}", config.tools.download_dir);
println!("Search API key: {:?}", config.tools.web_search_api_key);
```

### Initialization

```rust
use jamey_tools::connectors::NetworkWebConnector;
use std::path::PathBuf;

// Create connector
let connector = NetworkWebConnector::new(
    PathBuf::from("./downloads"),  // Download directory
    None                            // Optional search API key
)?;
```

## Web Search

### Search the Web

Perform web searches using DuckDuckGo:

```rust
use std::collections::HashMap;

let mut params = HashMap::new();
params.insert("action".to_string(), "web_search".to_string());
params.insert("query".to_string(), "Rust async programming".to_string());

let result = orchestrator
    .execute_connector("network_web", params)
    .await?;

// Result contains HTML from DuckDuckGo
println!("Search results:\n{}", result.output);
```

### Parse Search Results

```rust
async fn search_and_parse(query: &str) -> anyhow::Result<Vec<String>> {
    let mut params = HashMap::new();
    params.insert("action".to_string(), "web_search".to_string());
    params.insert("query".to_string(), query.to_string());

    let result = orchestrator
        .execute_connector("network_web", params)
        .await?;

    // Parse HTML (simplified - use proper HTML parser in production)
    let html = result.output;
    let links: Vec<String> = extract_links_from_html(&html);
    
    Ok(links)
}
```

## File Downloads

### Download File

Download a file from a URL:

```rust
use std::collections::HashMap;

let mut params = HashMap::new();
params.insert("action".to_string(), "download".to_string());
params.insert("url".to_string(), 
    "https://example.com/file.pdf".to_string());
params.insert("filename".to_string(), "document.pdf".to_string());

let result = orchestrator
    .execute_connector("network_web", params)
    .await?;

if result.success {
    println!("✅ Downloaded to: {}", result.output);
} else {
    eprintln!("❌ Download failed: {:?}", result.errors);
}
```

### Download with Auto-Filename

If no filename is provided, it's extracted from the URL:

```rust
let mut params = HashMap::new();
params.insert("action".to_string(), "download".to_string());
params.insert("url".to_string(), 
    "https://example.com/data/report.pdf".to_string());
// No filename parameter - will use "report.pdf"

let result = orchestrator
    .execute_connector("network_web", params)
    .await?;
```

## URL Fetching

### Fetch URL Content

Retrieve content from a URL:

```rust
use std::collections::HashMap;

let mut params = HashMap::new();
params.insert("action".to_string(), "fetch_url".to_string());
params.insert("url".to_string(), 
    "https://api.github.com/repos/rust-lang/rust".to_string());

let result = orchestrator
    .execute_connector("network_web", params)
    .await?;

// Parse JSON response
let data: serde_json::Value = serde_json::from_str(&result.output)?;
println!("Repository: {}", data["full_name"]);
```

### Fetch with Error Handling

```rust
async fn fetch_with_retry(url: &str, max_retries: u32) -> anyhow::Result<String> {
    for attempt in 1..=max_retries {
        let mut params = HashMap::new();
        params.insert("action".to_string(), "fetch_url".to_string());
        params.insert("url".to_string(), url.to_string());

        match orchestrator.execute_connector("network_web", params).await {
            Ok(result) if result.success => return Ok(result.output),
            Err(e) if attempt < max_retries => {
                eprintln!("Attempt {} failed: {}", attempt, e);
                tokio::time::sleep(Duration::from_secs(2)).await;
            }
            Err(e) => return Err(e),
            _ => {}
        }
    }
    
    Err(anyhow::anyhow!("All retry attempts failed"))
}
```

## SSRF Protection

### URL Validation

All URLs are validated to prevent SSRF attacks:

```rust
fn validate_url(url: &str) -> Result<()> {
    let parsed = url::Url::parse(url)?;
    
    // ✅ Only HTTP/HTTPS
    let scheme = parsed.scheme();
    if scheme != "http" && scheme != "https" {
        bail!("Only HTTP and HTTPS schemes allowed");
    }
    
    let host = parsed.host_str()
        .ok_or_else(|| anyhow!("URL must have a host"))?;
    
    // ❌ Block localhost
    if host == "localhost" || host == "127.0.0.1" || host == "::1" {
        bail!("Localhost access not allowed");
    }
    
    // ❌ Block cloud metadata
    if host == "169.254.169.254" 
        || host == "metadata.google.internal"
        || host == "metadata.azure.com" {
        bail!("Cloud metadata endpoint access not allowed");
    }
    
    // ❌ Block private IPs
    if let Ok(ip) = host.parse::<IpAddr>() {
        if is_private_ip(&ip) {
            bail!("Private IP address access not allowed");
        }
    }
    
    // ❌ Block internal domains
    if host.ends_with(".local") || host.ends_with(".internal") {
        bail!("Internal domain access not allowed");
    }
    
    Ok(())
}
```

### Blocked URLs

The following are **always blocked**:

```rust
// ❌ Localhost variations
"http://localhost:8080"
"http://127.0.0.1"
"http://[::1]"

// ❌ Private IP ranges
"http://10.0.0.1"           // 10.0.0.0/8
"http://192.168.1.1"        // 192.168.0.0/16
"http://172.16.0.1"         // 172.16.0.0/12

// ❌ Cloud metadata endpoints
"http://169.254.169.254"    // AWS, Azure, GCP
"http://metadata.google.internal"
"http://metadata.azure.com"

// ❌ Link-local addresses
"http://169.254.0.1"        // 169.254.0.0/16

// ❌ Internal domains
"http://internal.company.local"
"http://service.internal"
```

### Allowed URLs

```rust
// ✅ Public internet URLs
"https://example.com"
"https://api.github.com"
"https://www.rust-lang.org"
"https://docs.rs"
```

### Private IP Detection

```rust
fn is_private_ip(ip: &IpAddr) -> bool {
    match ip {
        IpAddr::V4(ipv4) => {
            let octets = ipv4.octets();
            octets[0] == 10                                    // 10.0.0.0/8
            || (octets[0] == 172 && (16..=31).contains(&octets[1]))  // 172.16.0.0/12
            || (octets[0] == 192 && octets[1] == 168)         // 192.168.0.0/16
            || octets[0] == 127                                // 127.0.0.0/8
            || (octets[0] == 169 && octets[1] == 254)         // 169.254.0.0/16
            || octets[0] == 0                                  // 0.0.0.0/8
        }
        IpAddr::V6(ipv6) => {
            ipv6.is_loopback()                                 // ::1
            || ((ipv6.segments()[0] & 0xffc0) == 0xfe80)      // fe80::/10
            || ((ipv6.segments()[0] & 0xfe00) == 0xfc00)      // fc00::/7
        }
    }
}
```

## Usage Examples

### Example 1: Research Topic

```rust
async fn research_topic(topic: &str) -> anyhow::Result<String> {
    println!("🔍 Researching: {}", topic);
    
    // Search the web
    let mut params = HashMap::new();
    params.insert("action".to_string(), "web_search".to_string());
    params.insert("query".to_string(), topic.to_string());

    let result = orchestrator
        .execute_connector("network_web", params)
        .await?;

    // Extract and summarize results
    let summary = summarize_search_results(&result.output)?;
    
    Ok(summary)
}
```

### Example 2: Download Documentation

```rust
async fn download_docs(url: &str) -> anyhow::Result<PathBuf> {
    let mut params = HashMap::new();
    params.insert("action".to_string(), "download".to_string());
    params.insert("url".to_string(), url.to_string());
    params.insert("filename".to_string(), "documentation.pdf".to_string());

    let result = orchestrator
        .execute_connector("network_web", params)
        .await?;

    if result.success {
        let path = PathBuf::from(result.output);
        println!("✅ Documentation downloaded: {:?}", path);
        Ok(path)
    } else {
        Err(anyhow::anyhow!("Download failed"))
    }
}
```

### Example 3: Fetch API Data

```rust
async fn fetch_github_repo(owner: &str, repo: &str) -> anyhow::Result<RepoInfo> {
    let url = format!("https://api.github.com/repos/{}/{}", owner, repo);
    
    let mut params = HashMap::new();
    params.insert("action".to_string(), "fetch_url".to_string());
    params.insert("url".to_string(), url);

    let result = orchestrator
        .execute_connector("network_web", params)
        .await?;

    let data: serde_json::Value = serde_json::from_str(&result.output)?;
    
    Ok(RepoInfo {
        name: data["name"].as_str().unwrap().to_string(),
        stars: data["stargazers_count"].as_u64().unwrap(),
        forks: data["forks_count"].as_u64().unwrap(),
        language: data["language"].as_str().unwrap().to_string(),
    })
}
```

### Example 4: Monitor Website

```rust
async fn monitor_website(url: &str) -> anyhow::Result<()> {
    loop {
        let mut params = HashMap::new();
        params.insert("action".to_string(), "fetch_url".to_string());
        params.insert("url".to_string(), url.to_string());

        match orchestrator.execute_connector("network_web", params).await {
            Ok(result) if result.success => {
                println!("✅ Website is up: {}", url);
            }
            Ok(_) | Err(_) => {
                eprintln!("❌ Website is down: {}", url);
                // Send alert
            }
        }

        tokio::time::sleep(Duration::from_secs(300)).await; // Check every 5 minutes
    }
}
```

## Best Practices

### ✅ Do

1. **Validate URLs Before Use**
   ```rust
   // Check URL format
   if !url.starts_with("https://") {
       return Err(anyhow!("Only HTTPS URLs allowed"));
   }
   ```

2. **Handle Network Errors**
   ```rust
   match fetch_url(url).await {
       Ok(content) => process(content),
       Err(e) => {
           eprintln!("Network error: {}", e);
           // Implement retry logic
       }
   }
   ```

3. **Set Reasonable Timeouts**
   ```rust
   // Connector has 300-second timeout by default
   // For faster operations, implement custom timeout
   tokio::time::timeout(
       Duration::from_secs(30),
       fetch_url(url)
   ).await??;
   ```

4. **Respect Rate Limits**
   ```rust
   // Add delays between requests
   for url in urls {
       fetch_url(url).await?;
       tokio::time::sleep(Duration::from_secs(1)).await;
   }
   ```

5. **Verify Downloaded Files**
   ```rust
   // Check file size and type
   let metadata = fs::metadata(&downloaded_file)?;
   if metadata.len() > MAX_FILE_SIZE {
       return Err(anyhow!("File too large"));
   }
   ```

### ❌ Don't

1. **Don't Access Internal Networks**
   - Private IPs are blocked
   - Internal domains are blocked

2. **Don't Ignore SSRF Warnings**
   - URL validation errors indicate security issues
   - Never bypass SSRF protection

3. **Don't Download Untrusted Files**
   - Verify file sources
   - Scan downloads for malware

4. **Don't Overwhelm Servers**
   - Implement rate limiting
   - Respect robots.txt

5. **Don't Store Credentials in URLs**
   - Use proper authentication headers
   - Never log URLs with credentials

## Troubleshooting

### Issue: "Security violation: Only HTTP and HTTPS schemes allowed"

**Cause**: Attempted to use non-HTTP(S) scheme

**Solution**: Use only HTTP or HTTPS URLs
```rust
// ❌ Wrong
"ftp://example.com/file"
"file:///etc/passwd"

// ✅ Correct
"https://example.com/file"
```

### Issue: "Security violation: Localhost access is not allowed"

**Cause**: Attempted to access localhost

**Solution**: SSRF protection blocks localhost access by design

### Issue: "Security violation: Private IP address access is not allowed"

**Cause**: Attempted to access private IP range

**Solution**: Use public internet URLs only

### Issue: "Security violation: Cloud metadata endpoint access is not allowed"

**Cause**: Attempted to access cloud metadata service

**Solution**: This is blocked for security. Use proper cloud APIs instead.

### Issue: "Failed to download file"

**Cause**: Network error, invalid URL, or file too large

**Solution**:
```rust
// Check URL is valid
// Verify network connectivity
// Check file size limits
```

### Issue: "Request timeout"

**Cause**: Server not responding within timeout period

**Solution**:
```rust
// Implement retry logic
// Check server status
// Increase timeout if appropriate
```

## Security Considerations

### Threat Model

**Threats**:
- SSRF attacks (accessing internal resources)
- Data exfiltration
- Malware downloads
- DDoS participation
- Credential theft

**Mitigations**:
- URL validation (blocks private networks)
- SSRF protection (blocks cloud metadata)
- Rate limiting (prevents abuse)
- Timeout control (prevents hanging)
- Audit logging (tracks all requests)

### Security Controls

1. **URL Validation**
   - Only HTTP/HTTPS schemes
   - Blocks localhost and private IPs
   - Blocks cloud metadata endpoints
   - Blocks internal domains

2. **SSRF Protection**
   - IP address validation
   - Domain name validation
   - Canonical URL checking

3. **Rate Limiting**
   - Prevents abuse
   - Protects target servers
   - Reduces attack surface

4. **Timeout Control**
   - 300-second default timeout
   - Prevents resource exhaustion
   - Enables graceful failure

5. **Audit Logging**
   ```rust
   tracing::info!("Web search: {}", query);
   tracing::warn!("Downloading file from: {}", url);
   ```

### Security Checklist

- [ ] URL validation enabled
- [ ] SSRF protection active
- [ ] Rate limiting configured
- [ ] Timeout settings appropriate
- [ ] Audit logging enabled and monitored
- [ ] Download directory secured
- [ ] File size limits enforced
- [ ] Malware scanning implemented (if applicable)

## Related Documentation

- [AI Agent Overview](README.md) - Overview of all agent capabilities
- [Security Best Practices](security-best-practices.md) - Security guidelines
- [Full System Access](full-system-access.md) - File system operations
- [Agent Orchestration](orchestration.md) - Multi-agent coordination

## API Reference

### Actions

#### `web_search`
Search the web using DuckDuckGo.

**Parameters**:
- `action`: `"web_search"`
- `query`: Search query string

**Returns**: HTML search results

#### `download`
Download a file from a URL.

**Parameters**:
- `action`: `"download"`
- `url`: File URL (must pass SSRF validation)
- `filename`: Optional filename (extracted from URL if not provided)

**Returns**: Downloaded file path

#### `fetch_url`
Fetch content from a URL.

**Parameters**:
- `action`: `"fetch_url"`
- `url`: URL to fetch (must pass SSRF validation)

**Returns**: URL content as string

#### `browser_action`
Browser automation (not yet implemented).

**Parameters**:
- `action`: `"browser_action"`

**Returns**: Placeholder message

---

**Last Updated**: 2025-11-17  
**Version**: 1.0.0  
**Status**: ✅ Complete