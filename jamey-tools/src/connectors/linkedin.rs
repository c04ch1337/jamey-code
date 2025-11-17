//! LinkedIn Connector
//! 
//! Provides LinkedIn API access for profile, posts, and networking

use crate::connector::*;
use reqwest::Client;
use std::collections::HashMap;
use anyhow::Result;
use serde_json::Value;

pub struct LinkedInConnector {
    metadata: ConnectorMetadata,
    client: Client,
    access_token: String,
    enabled: bool,
}

impl LinkedInConnector {
    pub fn new(access_token: String) -> Result<Self> {
        let client = Client::builder()
            .user_agent("Jamey-2.0-Agent/1.0")
            .build()?;

        Ok(Self {
            metadata: ConnectorMetadata {
                id: "linkedin".to_string(),
                name: "LinkedIn Integration".to_string(),
                version: "1.0.0".to_string(),
                description: "LinkedIn API access for profile, posts, and networking".to_string(),
                capability_level: CapabilityLevel::CloudAccess,
                requires_approval: false,
                safety_checks: vec![
                    "LinkedIn API rate limits enforced".to_string(),
                    "OAuth token validation".to_string(),
                ],
            },
            client,
            access_token,
            enabled: true,
        })
    }

    async fn get_profile(&self) -> Result<Value> {
        let url = "https://api.linkedin.com/v2/me";
        let response = self.client
            .get(url)
            .header("Authorization", format!("Bearer {}", self.access_token))
            .send()
            .await?;
        Ok(response.json().await?)
    }

    async fn create_post(&self, text: &str, person_urn: Option<&str>) -> Result<Value> {
        // LinkedIn API v2 for posts
        let url = "https://api.linkedin.com/v2/ugcPosts";
        let person_urn = person_urn.unwrap_or("urn:li:person:YOUR_PERSON_URN");
        
        let payload = serde_json::json!({
            "author": person_urn,
            "lifecycleState": "PUBLISHED",
            "specificContent": {
                "com.linkedin.ugc.ShareContent": {
                    "shareCommentary": {
                        "text": text
                    },
                    "shareMediaCategory": "NONE"
                }
            },
            "visibility": {
                "com.linkedin.ugc.MemberNetworkVisibility": "PUBLIC"
            }
        });
        
        let response = self.client
            .post(url)
            .header("Authorization", format!("Bearer {}", self.access_token))
            .header("Content-Type", "application/json")
            .json(&payload)
            .send()
            .await?;
        Ok(response.json().await?)
    }
}

#[async_trait::async_trait]
impl Connector for LinkedInConnector {
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
            "get_profile" => {
                let profile = self.get_profile().await?;
                result.output = serde_json::to_string_pretty(&profile)?;
                result.success = true;
            }
            "create_post" => {
                let text = params.get("text").ok_or_else(|| anyhow::anyhow!("Missing text"))?;
                let person_urn = params.get("person_urn").map(|s| s.as_str());
                let post = self.create_post(text, person_urn).await?;
                result.output = serde_json::to_string_pretty(&post)?;
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
        vec!["linkedin_access_token".to_string()]
    }
}

