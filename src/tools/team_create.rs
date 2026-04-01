//! TeamCreateTool -- create a new agent team/swarm.
//!
//! Ported from ref/tools/TeamCreateTool/TeamCreateTool.ts.
//! Creates a team of agents with designated roles and communication channels.

use async_trait::async_trait;
use crate::types::permissions::{PermissionResult, ToolPermissionContext};
use crate::types::tool::*;
use std::sync::LazyLock;
use std::sync::Mutex;

/// A team definition.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct Team {
    pub name: String,
    pub description: String,
    pub members: Vec<TeamMember>,
    pub created_at: String,
}

/// A team member.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct TeamMember {
    pub name: String,
    pub role: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub agent_id: Option<String>,
}

/// In-memory team store.
pub static TEAM_STORE: LazyLock<Mutex<Vec<Team>>> =
    LazyLock::new(|| Mutex::new(Vec::new()));

pub const TEAM_CREATE_TOOL_NAME: &str = "TeamCreate";

pub struct TeamCreateTool;

#[async_trait]
impl Tool for TeamCreateTool {
    fn name(&self) -> &str {
        TEAM_CREATE_TOOL_NAME
    }

    fn max_result_size_chars(&self) -> usize {
        100_000
    }

    fn should_defer(&self) -> bool {
        true
    }

    fn search_hint(&self) -> Option<&str> {
        Some("create a team of agents for parallel work")
    }

    fn input_schema(&self) -> ToolInputJSONSchema {
        serde_json::json!({
            "type": "object",
            "properties": {
                "name": {
                    "type": "string",
                    "description": "Team name (unique identifier)"
                },
                "description": {
                    "type": "string",
                    "description": "What the team is working on"
                },
                "members": {
                    "type": "array",
                    "items": {
                        "type": "object",
                        "properties": {
                            "name": {
                                "type": "string",
                                "description": "Member name (used for SendMessage addressing)"
                            },
                            "role": {
                                "type": "string",
                                "description": "The member's role description"
                            }
                        },
                        "required": ["name", "role"]
                    },
                    "description": "Team members to create"
                }
            },
            "required": ["name"]
        })
    }

    async fn validate_input(
        &self,
        input: &serde_json::Value,
        _context: &ToolUseContext,
    ) -> ValidationResult {
        let name = input.get("name").and_then(|v| v.as_str()).unwrap_or("");
        if name.is_empty() {
            return ValidationResult::invalid("team name must not be empty", 9);
        }

        // Check for duplicate team names
        let store = TEAM_STORE.lock().unwrap();
        if store.iter().any(|t| t.name.eq_ignore_ascii_case(name)) {
            return ValidationResult::invalid(
                &format!("Team '{}' already exists", name),
                9,
            );
        }

        ValidationResult::valid()
    }

    async fn call(
        &self,
        input: serde_json::Value,
        _context: &ToolUseContext,
        _on_progress: Option<Box<dyn Fn(ToolProgress) + Send + Sync>>,
    ) -> Result<ToolCallResult, ToolError> {
        let name = input
            .get("name")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        let description = input
            .get("description")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();

        let members: Vec<TeamMember> = input
            .get("members")
            .and_then(|v| v.as_array())
            .map(|arr| {
                arr.iter()
                    .filter_map(|m| {
                        let member_name = m.get("name").and_then(|v| v.as_str())?;
                        let role = m.get("role").and_then(|v| v.as_str()).unwrap_or("");
                        Some(TeamMember {
                            name: member_name.to_string(),
                            role: role.to_string(),
                            agent_id: None,
                        })
                    })
                    .collect()
            })
            .unwrap_or_default();

        let member_names: Vec<String> = members.iter().map(|m| m.name.clone()).collect();

        let team = Team {
            name: name.clone(),
            description: description.clone(),
            members,
            created_at: chrono::Utc::now().to_rfc3339(),
        };

        {
            let mut store = TEAM_STORE.lock().unwrap();
            store.push(team);
        }

        Ok(ToolCallResult {
            data: serde_json::json!({
                "teamName": name,
                "description": description,
                "members": member_names,
                "created": true,
                "message": format!("Team '{}' created with {} member(s)", name, member_names.len()),
            }),
            new_messages: None,
            mcp_meta: None,
        })
    }

    async fn check_permissions(
        &self,
        input: &serde_json::Value,
        _: &ToolUseContext,
    ) -> PermissionResult {
        PermissionResult::allow(Some(input.clone()))
    }

    fn description(&self, input: &serde_json::Value, _: &ToolPermissionContext) -> String {
        let name = input.get("name").and_then(|v| v.as_str()).unwrap_or("");
        format!("Create team: {name}")
    }

    async fn prompt(&self) -> String {
        "Create a new team of agents for parallel collaborative work.\n\
         \n\
         Each team member has a name (used for SendMessage addressing) and a role.\n\
         After creating a team, use the Agent tool to spawn agents for each member, \n\
         and SendMessage to coordinate between them."
            .to_string()
    }

    fn user_facing_name(&self, _: Option<&serde_json::Value>) -> String {
        "TeamCreate".to_string()
    }
}
