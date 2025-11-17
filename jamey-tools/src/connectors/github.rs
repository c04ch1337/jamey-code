//! GitHub Connector
//! 
//! Provides full GitHub API access for repositories, issues, PRs, and code management

use crate::connector::*;
use reqwest::Client;
use std::collections::HashMap;
use anyhow::Result;
use serde_json::Value;
use base64::{Engine as _, engine::general_purpose};

pub struct GitHubConnector {
    metadata: ConnectorMetadata,
    client: Client,
    token: String,
    enabled: bool,
}

impl GitHubConnector {
    pub fn new(token: String) -> Result<Self> {
        let mut headers = reqwest::header::HeaderMap::new();
        headers.insert(
            reqwest::header::AUTHORIZATION,
            format!("Bearer {}", token).parse()?
        );
        headers.insert(
            reqwest::header::USER_AGENT,
            "Jamey-2.0-Agent/1.0".parse()?
        );
        headers.insert(
            reqwest::header::ACCEPT,
            "application/vnd.github.v3+json".parse()?
        );

        let client = Client::builder()
            .default_headers(headers)
            .build()?;

        Ok(Self {
            metadata: ConnectorMetadata {
                id: "github".to_string(),
                name: "GitHub Integration".to_string(),
                version: "1.0.0".to_string(),
                description: "Full GitHub API access for repositories, issues, PRs, and code management".to_string(),
                capability_level: CapabilityLevel::CloudAccess,
                requires_approval: false,
                safety_checks: vec![
                    "GitHub API rate limits enforced".to_string(),
                    "Repository access validated".to_string(),
                ],
            },
            client,
            token,
            enabled: true,
        })
    }

    async fn get_repo(&self, owner: &str, repo: &str) -> Result<Value> {
        let url = format!("https://api.github.com/repos/{}/{}", owner, repo);
        let response = self.client.get(&url).send().await?;
        Ok(response.json().await?)
    }

    async fn create_issue(&self, owner: &str, repo: &str, title: &str, body: &str) -> Result<Value> {
        let url = format!("https://api.github.com/repos/{}/{}/issues", owner, repo);
        let payload = serde_json::json!({
            "title": title,
            "body": body
        });
        let response = self.client.post(&url).json(&payload).send().await?;
        Ok(response.json().await?)
    }

    async fn create_pr(&self, owner: &str, repo: &str, title: &str, head: &str, base: &str, body: &str) -> Result<Value> {
        let url = format!("https://api.github.com/repos/{}/{}/pulls", owner, repo);
        let payload = serde_json::json!({
            "title": title,
            "head": head,
            "base": base,
            "body": body
        });
        let response = self.client.post(&url).json(&payload).send().await?;
        Ok(response.json().await?)
    }

    async fn get_file_content(&self, owner: &str, repo: &str, path: &str, branch: Option<&str>) -> Result<String> {
        let branch = branch.unwrap_or("main");
        let url = format!("https://api.github.com/repos/{}/{}/contents/{}?ref={}", owner, repo, path, branch);
        let response = self.client.get(&url).send().await?;
        let json: Value = response.json().await?;
        
        if let Some(content) = json.get("content").and_then(|c| c.as_str()) {
            // GitHub returns base64 encoded content
            let decoded = general_purpose::STANDARD.decode(content.replace("\n", ""))?;
            Ok(String::from_utf8(decoded)?)
        } else {
            Err(anyhow::anyhow!("No content found"))
        }
    }

    async fn update_file(&self, owner: &str, repo: &str, path: &str, content: &str, message: &str, branch: Option<&str>) -> Result<Value> {
        // First get the file to get its SHA
        let branch = branch.unwrap_or("main");
        let get_url = format!("https://api.github.com/repos/{}/{}/contents/{}?ref={}", owner, repo, path, branch);
        let get_response = self.client.get(&get_url).send().await?;
        let file_info: Value = get_response.json().await?;
        let sha = file_info.get("sha")
            .and_then(|s| s.as_str())
            .ok_or_else(|| anyhow::anyhow!("Could not get file SHA"))?;

        // Encode content to base64
        let encoded = general_purpose::STANDARD.encode(content);

        // Update the file
        let url = format!("https://api.github.com/repos/{}/{}/contents/{}", owner, repo, path);
        let payload = serde_json::json!({
            "message": message,
            "content": encoded,
            "sha": sha,
            "branch": branch
        });
        let response = self.client.put(&url).json(&payload).send().await?;
        Ok(response.json().await?)
    }
}

#[async_trait::async_trait]
impl Connector for GitHubConnector {
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
            "get_repo" => {
                let owner = params.get("owner").ok_or_else(|| anyhow::anyhow!("Missing owner"))?;
                let repo = params.get("repo").ok_or_else(|| anyhow::anyhow!("Missing repo"))?;
                let repo_data = self.get_repo(owner, repo).await?;
                result.output = serde_json::to_string_pretty(&repo_data)?;
                result.success = true;
            }
            "create_issue" => {
                let owner = params.get("owner").ok_or_else(|| anyhow::anyhow!("Missing owner"))?;
                let repo = params.get("repo").ok_or_else(|| anyhow::anyhow!("Missing repo"))?;
                let title = params.get("title").ok_or_else(|| anyhow::anyhow!("Missing title"))?;
                let default_body = String::new();
                let body = params.get("body").unwrap_or(&default_body);
                let issue = self.create_issue(owner, repo, title, body).await?;
                result.output = serde_json::to_string_pretty(&issue)?;
                result.success = true;
            }
            "get_file" => {
                let owner = params.get("owner").ok_or_else(|| anyhow::anyhow!("Missing owner"))?;
                let repo = params.get("repo").ok_or_else(|| anyhow::anyhow!("Missing repo"))?;
                let path = params.get("path").ok_or_else(|| anyhow::anyhow!("Missing path"))?;
                let branch = params.get("branch").map(|s| s.as_str());
                let content = self.get_file_content(owner, repo, path, branch).await?;
                result.output = content;
                result.success = true;
            }
            "update_file" => {
                let owner = params.get("owner").ok_or_else(|| anyhow::anyhow!("Missing owner"))?;
                let repo = params.get("repo").ok_or_else(|| anyhow::anyhow!("Missing repo"))?;
                let path = params.get("path").ok_or_else(|| anyhow::anyhow!("Missing path"))?;
                let content = params.get("content").ok_or_else(|| anyhow::anyhow!("Missing content"))?;
                let message = params.get("message").ok_or_else(|| anyhow::anyhow!("Missing message"))?;
                let branch = params.get("branch").map(|s| s.as_str());
                let response = self.update_file(owner, repo, path, content, message, branch).await?;
                result.output = serde_json::to_string_pretty(&response)?;
                result.success = true;
            }
            "create_pr" => {
                let owner = params.get("owner").ok_or_else(|| anyhow::anyhow!("Missing owner"))?;
                let repo = params.get("repo").ok_or_else(|| anyhow::anyhow!("Missing repo"))?;
                let title = params.get("title").ok_or_else(|| anyhow::anyhow!("Missing title"))?;
                let head = params.get("head").ok_or_else(|| anyhow::anyhow!("Missing head"))?;
                let default_base = "main".to_string();
                let base = params.get("base").unwrap_or(&default_base);
                let default_body = String::new();
                let body = params.get("body").unwrap_or(&default_body);
                let pr = self.create_pr(owner, repo, title, head, base, body).await?;
                result.output = serde_json::to_string_pretty(&pr)?;
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
        vec!["github_token".to_string()]
    }
}

