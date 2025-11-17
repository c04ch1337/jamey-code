# Agent Orchestration Guide

The Agent Orchestration connector enables Jamey 2.0 to coordinate tasks across multiple AI agents with secure communication. This allows for distributed task execution, load balancing, and specialized agent collaboration.

> ⚠️ **Important**: Agent-to-agent communication requires HTTPS with TLS 1.2+ and mandatory API key authentication.

## Table of Contents

- [Overview](#overview)
- [Configuration](#configuration)
- [Agent Registration](#agent-registration)
- [Task Delegation](#task-delegation)
- [Broadcast Operations](#broadcast-operations)
- [Security Architecture](#security-architecture)
- [Usage Examples](#usage-examples)
- [Best Practices](#best-practices)
- [Troubleshooting](#troubleshooting)
- [Security Considerations](#security-considerations)

## Overview

**Connector ID**: `agent_orchestration`  
**Capability Level**: `AgentOrchestration`  
**Requires Approval**: ❌ No (but all operations are logged)  
**Source**: [`jamey-tools/src/connectors/agent_orchestration.rs`](../../jamey-tools/src/connectors/agent_orchestration.rs)

### Key Features

- ✅ **Agent Registration**: Register remote agents with authentication
- ✅ **Task Delegation**: Send tasks to specific agents
- ✅ **Broadcast Operations**: Send tasks to all registered agents
- ✅ **HTTPS Enforcement**: TLS 1.2+ minimum for all communication
- ✅ **API Key Authentication**: Mandatory authentication for all agents
- ✅ **Certificate Validation**: Strict certificate checking enabled

### Capabilities

| Feature | Description | Security Control |
|---------|-------------|------------------|
| Register Agent | Add agent to registry | HTTPS validation, API key required |
| Send Task | Delegate task to agent | TLS 1.2+, certificate validation |
| Broadcast | Send to all agents | Same as send task |

## Configuration

### Environment Variables

No specific environment variables required. Agent endpoints are registered programmatically.

### Runtime Configuration

```rust
use jamey_runtime::config::RuntimeConfig;

let config = RuntimeConfig::from_env()?;
// Agent orchestration is always available when runtime is initialized
```

### Initialization

```rust
use jamey_tools::connectors::AgentOrchestrationConnector;

// Create connector (TLS 1.2+ enforced automatically)
let connector = AgentOrchestrationConnector::new()?;
```

## Agent Registration

### Agent Endpoint Structure

```rust
pub struct AgentEndpoint {
    pub id: String,              // Unique agent identifier
    pub name: String,            // Human-readable name
    pub url: String,             // HTTPS URL (required)
    pub api_key: String,         // API key (required)
    pub capabilities: Vec<String>, // Agent capabilities
}
```

### Register an Agent

```rust
use std::collections::HashMap;

let mut params = HashMap::new();
params.insert("action".to_string(), "register_agent".to_string());
params.insert("agent_id".to_string(), "agent-001".to_string());
params.insert("name".to_string(), "Data Processor".to_string());
params.insert("url".to_string(), "https://agent1.example.com".to_string());
params.insert("api_key".to_string(), "secret_key_here".to_string());
params.insert("capabilities".to_string(), "data_processing,analysis".to_string());

let result = orchestrator
    .execute_connector("agent_orchestration", params)
    .await?;

if result.success {
    println!("✅ Agent registered successfully");
} else {
    eprintln!("❌ Registration failed: {:?}", result.errors);
}
```

### Registration Validation

The system validates:

1. **HTTPS Requirement**: URL must use HTTPS scheme
2. **API Key Presence**: API key cannot be empty
3. **URL Format**: Must be a valid URL
4. **No Duplicates**: Agent ID must be unique

```rust
// ✅ Valid registration
url: "https://agent.example.com"
api_key: "sk_live_abc123..."

// ❌ Invalid - HTTP not allowed
url: "http://agent.example.com"

// ❌ Invalid - empty API key
api_key: ""
```

## Task Delegation

### Send Task to Specific Agent

```rust
use std::collections::HashMap;

let mut params = HashMap::new();
params.insert("action".to_string(), "send_task".to_string());
params.insert("agent_id".to_string(), "agent-001".to_string());
params.insert("task".to_string(), "process_data".to_string());

// Task-specific parameters (prefixed with "param_")
params.insert("param_dataset".to_string(), "sales_2024".to_string());
params.insert("param_format".to_string(), "json".to_string());

let result = orchestrator
    .execute_connector("agent_orchestration", params)
    .await?;

if result.success {
    let response: serde_json::Value = serde_json::from_str(&result.output)?;
    println!("Agent response: {:?}", response);
} else {
    eprintln!("Task failed: {:?}", result.errors);
}
```

### Task Request Format

Tasks are sent as JSON to the agent's API endpoint:

```json
POST https://agent.example.com/api/v1/tasks
Authorization: Bearer {api_key}
Content-Type: application/json

{
  "task": "process_data",
  "params": {
    "dataset": "sales_2024",
    "format": "json"
  }
}
```

### Handle Task Response

```rust
async fn delegate_and_process(
    agent_id: &str,
    task: &str,
    params: HashMap<String, String>
) -> anyhow::Result<serde_json::Value> {
    let mut task_params = HashMap::new();
    task_params.insert("action".to_string(), "send_task".to_string());
    task_params.insert("agent_id".to_string(), agent_id.to_string());
    task_params.insert("task".to_string(), task.to_string());
    
    // Add task-specific parameters
    for (key, value) in params {
        task_params.insert(format!("param_{}", key), value);
    }

    let result = orchestrator
        .execute_connector("agent_orchestration", task_params)
        .await?;

    if result.success {
        Ok(serde_json::from_str(&result.output)?)
    } else {
        Err(anyhow::anyhow!("Task failed: {:?}", result.errors))
    }
}
```

## Broadcast Operations

### Broadcast to All Agents

Send a task to all registered agents simultaneously:

```rust
use std::collections::HashMap;

let mut params = HashMap::new();
params.insert("action".to_string(), "broadcast".to_string());
params.insert("task".to_string(), "health_check".to_string());

let result = orchestrator
    .execute_connector("agent_orchestration", params)
    .await?;

// Parse results from all agents
let responses: HashMap<String, serde_json::Value> = 
    serde_json::from_str(&result.output)?;

for (agent_id, response) in responses {
    println!("Agent {}: {:?}", agent_id, response);
}
```

### Broadcast with Parameters

```rust
let mut params = HashMap::new();
params.insert("action".to_string(), "broadcast".to_string());
params.insert("task".to_string(), "update_config".to_string());
params.insert("param_version".to_string(), "2.0.0".to_string());
params.insert("param_restart".to_string(), "true".to_string());

let result = orchestrator
    .execute_connector("agent_orchestration", params)
    .await?;
```

### Handle Broadcast Failures

```rust
async fn broadcast_with_error_handling(
    task: &str,
    params: HashMap<String, String>
) -> anyhow::Result<()> {
    let mut broadcast_params = HashMap::new();
    broadcast_params.insert("action".to_string(), "broadcast".to_string());
    broadcast_params.insert("task".to_string(), task.to_string());
    
    for (key, value) in params {
        broadcast_params.insert(format!("param_{}", key), value);
    }

    let result = orchestrator
        .execute_connector("agent_orchestration", broadcast_params)
        .await?;

    let responses: HashMap<String, serde_json::Value> = 
        serde_json::from_str(&result.output)?;

    let mut successes = 0;
    let mut failures = 0;

    for (agent_id, response) in responses {
        if response.get("error").is_some() {
            eprintln!("❌ Agent {} failed: {:?}", agent_id, response);
            failures += 1;
        } else {
            println!("✅ Agent {} succeeded", agent_id);
            successes += 1;
        }
    }

    println!("Broadcast complete: {} succeeded, {} failed", successes, failures);
    Ok(())
}
```

## Security Architecture

### TLS Enforcement

All agent communication uses HTTPS with strict TLS requirements:

```rust
// HTTP client configuration
let client = ClientBuilder::new()
    .min_tls_version(reqwest::tls::Version::TLS_1_2)  // Minimum TLS 1.2
    .danger_accept_invalid_certs(false)                // Never accept invalid certs
    .timeout(std::time::Duration::from_secs(30))
    .build()?;
```

**Security Controls**:
- ✅ TLS 1.2 minimum (TLS 1.3 preferred)
- ✅ Certificate validation enabled
- ✅ No self-signed certificates accepted
- ✅ 30-second timeout to prevent hanging

### API Key Authentication

Every agent request includes authentication:

```rust
let request = client.post(&format!("{}/api/v1/tasks", agent.url))
    .header("Authorization", format!("Bearer {}", agent.api_key))
    .json(&payload);
```

**Requirements**:
- API key is **mandatory** (cannot be empty)
- Transmitted via Authorization header
- Never logged or exposed in error messages

### URL Validation

Agent URLs are validated during registration:

```rust
async fn register_agent(&self, agent: AgentEndpoint) -> Result<()> {
    // Parse and validate URL
    let parsed_url = url::Url::parse(&agent.url)?;
    
    // ❌ Only HTTPS allowed
    if parsed_url.scheme() != "https" {
        bail!("Agent URLs must use HTTPS. Got: {}", parsed_url.scheme());
    }
    
    // ❌ API key cannot be empty
    if agent.api_key.trim().is_empty() {
        bail!("API key cannot be empty");
    }
    
    // ✅ Register agent
    let mut agents = self.registered_agents.write().await;
    agents.insert(agent.id.clone(), agent);
    Ok(())
}
```

## Usage Examples

### Example 1: Multi-Agent Data Processing

```rust
async fn distributed_processing(data: Vec<String>) -> anyhow::Result<Vec<String>> {
    // Register processing agents
    for i in 1..=3 {
        let mut params = HashMap::new();
        params.insert("action".to_string(), "register_agent".to_string());
        params.insert("agent_id".to_string(), format!("processor-{}", i));
        params.insert("name".to_string(), format!("Processor {}", i));
        params.insert("url".to_string(), 
            format!("https://processor{}.example.com", i));
        params.insert("api_key".to_string(), 
            std::env::var(format!("PROCESSOR_{}_KEY", i))?);
        params.insert("capabilities".to_string(), "data_processing".to_string());
        
        orchestrator.execute_connector("agent_orchestration", params).await?;
    }

    // Distribute work
    let chunk_size = data.len() / 3;
    let mut results = Vec::new();

    for (i, chunk) in data.chunks(chunk_size).enumerate() {
        let agent_id = format!("processor-{}", i + 1);
        let mut params = HashMap::new();
        params.insert("action".to_string(), "send_task".to_string());
        params.insert("agent_id".to_string(), agent_id);
        params.insert("task".to_string(), "process".to_string());
        params.insert("param_data".to_string(), 
            serde_json::to_string(&chunk)?);

        let result = orchestrator
            .execute_connector("agent_orchestration", params)
            .await?;

        let processed: Vec<String> = serde_json::from_str(&result.output)?;
        results.extend(processed);
    }

    Ok(results)
}
```

### Example 2: Health Check All Agents

```rust
async fn health_check_all() -> anyhow::Result<()> {
    let mut params = HashMap::new();
    params.insert("action".to_string(), "broadcast".to_string());
    params.insert("task".to_string(), "health_check".to_string());

    let result = orchestrator
        .execute_connector("agent_orchestration", params)
        .await?;

    let responses: HashMap<String, serde_json::Value> = 
        serde_json::from_str(&result.output)?;

    println!("Health Check Results:");
    for (agent_id, response) in responses {
        if let Some(status) = response.get("status") {
            println!("  {} - {}", agent_id, status);
        } else if let Some(error) = response.get("error") {
            eprintln!("  {} - ERROR: {}", agent_id, error);
        }
    }

    Ok(())
}
```

### Example 3: Specialized Agent Coordination

```rust
async fn coordinate_workflow() -> anyhow::Result<()> {
    // Step 1: Data collection agent
    let mut params = HashMap::new();
    params.insert("action".to_string(), "send_task".to_string());
    params.insert("agent_id".to_string(), "collector".to_string());
    params.insert("task".to_string(), "collect_data".to_string());
    
    let data = orchestrator
        .execute_connector("agent_orchestration", params)
        .await?;

    // Step 2: Analysis agent
    let mut params = HashMap::new();
    params.insert("action".to_string(), "send_task".to_string());
    params.insert("agent_id".to_string(), "analyzer".to_string());
    params.insert("task".to_string(), "analyze".to_string());
    params.insert("param_data".to_string(), data.output);
    
    let analysis = orchestrator
        .execute_connector("agent_orchestration", params)
        .await?;

    // Step 3: Report generation agent
    let mut params = HashMap::new();
    params.insert("action".to_string(), "send_task".to_string());
    params.insert("agent_id".to_string(), "reporter".to_string());
    params.insert("task".to_string(), "generate_report".to_string());
    params.insert("param_analysis".to_string(), analysis.output);
    
    let report = orchestrator
        .execute_connector("agent_orchestration", params)
        .await?;

    println!("Workflow complete. Report: {}", report.output);
    Ok(())
}
```

## Best Practices

### ✅ Do

1. **Use HTTPS Only**
   ```rust
   // ✅ Correct
   url: "https://agent.example.com"
   
   // ❌ Wrong
   url: "http://agent.example.com"
   ```

2. **Secure API Keys**
   ```rust
   // Store in environment variables
   let api_key = std::env::var("AGENT_API_KEY")?;
   
   // Never hardcode
   // let api_key = "sk_live_abc123..."; // ❌ Don't do this
   ```

3. **Handle Agent Failures**
   ```rust
   match send_task(agent_id, task).await {
       Ok(result) => process(result),
       Err(e) => {
           eprintln!("Agent failed: {}", e);
           // Implement fallback or retry
       }
   }
   ```

4. **Monitor Agent Health**
   ```rust
   // Regular health checks
   tokio::spawn(async {
       loop {
           health_check_all().await.ok();
           tokio::time::sleep(Duration::from_secs(300)).await;
       }
   });
   ```

5. **Log All Operations**
   ```rust
   tracing::info!("Registering agent: {} at {}", name, url);
   tracing::warn!("Sending task to agent: {}", agent_id);
   ```

### ❌ Don't

1. **Don't Use HTTP**
   - HTTPS is mandatory
   - HTTP will be rejected

2. **Don't Share API Keys**
   - Each agent should have unique keys
   - Rotate keys regularly

3. **Don't Ignore Errors**
   - Always handle agent failures
   - Implement retry logic

4. **Don't Bypass Certificate Validation**
   - Certificate validation is mandatory
   - Use proper certificates

5. **Don't Send Sensitive Data Without Encryption**
   - TLS encrypts transport
   - Consider additional encryption for sensitive data

## Troubleshooting

### Issue: "Security violation: Agent URLs must use HTTPS"

**Cause**: Attempted to register agent with HTTP URL

**Solution**: Use HTTPS only
```rust
// ❌ Wrong
url: "http://agent.example.com"

// ✅ Correct
url: "https://agent.example.com"
```

### Issue: "Security violation: API key cannot be empty"

**Cause**: Missing or empty API key

**Solution**: Provide valid API key
```rust
params.insert("api_key".to_string(), "sk_live_abc123...".to_string());
```

### Issue: "Agent returned error status: 401"

**Cause**: Invalid or missing API key

**Solution**: Verify API key is correct and not expired

### Issue: "Agent returned error status: 404"

**Cause**: Invalid agent URL or endpoint

**Solution**: Verify agent URL and API endpoint path

### Issue: "Failed to send task to agent"

**Cause**: Network error, timeout, or agent offline

**Solution**:
```rust
// Implement retry logic
for attempt in 1..=3 {
    match send_task(agent_id, task).await {
        Ok(result) => return Ok(result),
        Err(e) if attempt < 3 => {
            eprintln!("Attempt {} failed: {}", attempt, e);
            tokio::time::sleep(Duration::from_secs(2)).await;
        }
        Err(e) => return Err(e),
    }
}
```

### Issue: "Certificate validation failed"

**Cause**: Invalid or self-signed certificate

**Solution**: Use proper TLS certificates from trusted CA

## Security Considerations

### Threat Model

**Threats**:
- Man-in-the-middle attacks
- API key theft
- Unauthorized agent access
- Data interception
- Agent impersonation

**Mitigations**:
- TLS 1.2+ enforcement (prevents MITM)
- Certificate validation (prevents impersonation)
- API key authentication (prevents unauthorized access)
- Secure key storage (prevents theft)
- Audit logging (tracks all operations)

### Security Controls

1. **TLS Enforcement**
   - Minimum TLS 1.2
   - Certificate validation enabled
   - No self-signed certificates

2. **API Key Authentication**
   - Mandatory for all agents
   - Transmitted securely via HTTPS
   - Never logged or exposed

3. **URL Validation**
   - HTTPS scheme required
   - Proper URL format validation
   - No localhost or private IPs

4. **Timeout Control**
   - 30-second timeout prevents hanging
   - Enables graceful failure
   - Prevents resource exhaustion

5. **Audit Logging**
   ```rust
   tracing::info!("Registering agent: {} at {}", name, url);
   tracing::warn!("Sending task to agent: {}", agent_id);
   ```

### Security Checklist

- [ ] All agents use HTTPS
- [ ] API keys stored securely (environment variables)
- [ ] Certificate validation enabled
- [ ] TLS 1.2+ enforced
- [ ] Audit logging enabled and monitored
- [ ] Agent health monitoring configured
- [ ] Error handling implemented
- [ ] Retry logic in place
- [ ] API key rotation schedule established

## Related Documentation

- [AI Agent Overview](README.md) - Overview of all agent capabilities
- [Security Best Practices](security-best-practices.md) - Security guidelines
- [24/7 Service Mode](always-on.md) - Continuous operation
- [Network Access](network-access.md) - Web and network operations

## API Reference

### Actions

#### `register_agent`
Register a new agent in the orchestration system.

**Parameters**:
- `action`: `"register_agent"`
- `agent_id`: Unique agent identifier
- `name`: Human-readable agent name
- `url`: HTTPS URL (required)
- `api_key`: API key (required)
- `capabilities`: Comma-separated capabilities (optional)

**Returns**: Success message

#### `send_task`
Send a task to a specific agent.

**Parameters**:
- `action`: `"send_task"`
- `agent_id`: Target agent ID
- `task`: Task name
- `param_*`: Task-specific parameters (prefix with `param_`)

**Returns**: Agent response as JSON

#### `broadcast`
Send a task to all registered agents.

**Parameters**:
- `action`: `"broadcast"`
- `task`: Task name
- `param_*`: Task-specific parameters (prefix with `param_`)

**Returns**: Map of agent IDs to responses

---

**Last Updated**: 2025-11-17  
**Version**: 1.0.0  
**Status**: ✅ Complete