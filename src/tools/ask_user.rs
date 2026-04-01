//! AskUserQuestionTool -- ask the user questions.
//!
//! Ported from ref/tools/AskUserQuestionTool/AskUserQuestionTool.tsx.
//! Presents multiple-choice questions to the user and collects their answers.
//! The permission system (or REPL layer) handles the actual UI; this tool
//! emits the questions and returns with the collected answers.

use async_trait::async_trait;
use crate::types::permissions::{PermissionResult, ToolPermissionContext};
use crate::types::tool::*;

pub const ASK_USER_TOOL_NAME: &str = "AskUserQuestion";

pub struct AskUserQuestionTool;

#[async_trait]
impl Tool for AskUserQuestionTool {
    fn name(&self) -> &str {
        ASK_USER_TOOL_NAME
    }

    fn max_result_size_chars(&self) -> usize {
        100_000
    }

    fn is_read_only(&self, _: &serde_json::Value) -> bool {
        true
    }

    fn should_defer(&self) -> bool {
        true
    }

    fn is_concurrency_safe(&self, _: &serde_json::Value) -> bool {
        true
    }

    fn search_hint(&self) -> Option<&str> {
        Some("prompt the user with a multiple-choice question")
    }

    fn interrupt_behavior(&self) -> InterruptBehavior {
        InterruptBehavior::Cancel
    }

    fn input_schema(&self) -> ToolInputJSONSchema {
        serde_json::json!({
            "type": "object",
            "properties": {
                "questions": {
                    "type": "array",
                    "items": {
                        "type": "object",
                        "properties": {
                            "question": {
                                "type": "string",
                                "description": "The complete question to ask the user. Should be clear, specific, and end with a question mark."
                            },
                            "header": {
                                "type": "string",
                                "description": "Very short label displayed as a chip/tag (max 12 chars). Examples: 'Auth method', 'Library', 'Approach'."
                            },
                            "options": {
                                "type": "array",
                                "items": {
                                    "type": "object",
                                    "properties": {
                                        "label": {
                                            "type": "string",
                                            "description": "Concise display text (1-5 words)"
                                        },
                                        "description": {
                                            "type": "string",
                                            "description": "Explanation of what this option means"
                                        },
                                        "preview": {
                                            "type": "string",
                                            "description": "Optional preview content (markdown for mockups, code snippets, etc.)"
                                        }
                                    },
                                    "required": ["label", "description"]
                                },
                                "minItems": 2,
                                "maxItems": 4,
                                "description": "Available choices (2-4 options). No 'Other' needed -- provided automatically."
                            },
                            "multiSelect": {
                                "type": "boolean",
                                "description": "Set to true to allow multiple selections",
                                "default": false
                            }
                        },
                        "required": ["question", "header", "options"]
                    },
                    "minItems": 1,
                    "maxItems": 4,
                    "description": "Questions to ask the user (1-4 questions)"
                },
                "answers": {
                    "type": "object",
                    "description": "User answers collected by the permission component (question text -> answer string)"
                },
                "annotations": {
                    "type": "object",
                    "description": "Optional per-question annotations from the user"
                },
                "metadata": {
                    "type": "object",
                    "properties": {
                        "source": {
                            "type": "string",
                            "description": "Optional identifier for analytics tracking"
                        }
                    },
                    "description": "Optional metadata for tracking"
                }
            },
            "required": ["questions"]
        })
    }

    async fn validate_input(
        &self,
        input: &serde_json::Value,
        _context: &ToolUseContext,
    ) -> ValidationResult {
        let questions = match input.get("questions").and_then(|v| v.as_array()) {
            Some(q) => q,
            None => {
                return ValidationResult::invalid("questions must be a non-empty array", 9);
            }
        };

        if questions.is_empty() || questions.len() > 4 {
            return ValidationResult::invalid("questions must have 1-4 items", 9);
        }

        // Check uniqueness of question texts
        let mut seen_questions = std::collections::HashSet::new();
        for q in questions {
            let text = q.get("question").and_then(|v| v.as_str()).unwrap_or("");
            if !seen_questions.insert(text.to_string()) {
                return ValidationResult::invalid(
                    "Question texts must be unique, option labels must be unique within each question",
                    9,
                );
            }

            // Check option label uniqueness within each question
            if let Some(options) = q.get("options").and_then(|v| v.as_array()) {
                let mut seen_labels = std::collections::HashSet::new();
                for opt in options {
                    let label = opt.get("label").and_then(|v| v.as_str()).unwrap_or("");
                    if !seen_labels.insert(label.to_string()) {
                        return ValidationResult::invalid(
                            "Question texts must be unique, option labels must be unique within each question",
                            9,
                        );
                    }
                }
            }
        }

        ValidationResult::valid()
    }

    async fn call(
        &self,
        input: serde_json::Value,
        _context: &ToolUseContext,
        _on_progress: Option<Box<dyn Fn(ToolProgress) + Send + Sync>>,
    ) -> Result<ToolCallResult, ToolError> {
        let questions = input
            .get("questions")
            .cloned()
            .unwrap_or(serde_json::json!([]));

        // If answers were already provided (via permission check flow),
        // return them directly.
        let answers = input
            .get("answers")
            .cloned()
            .unwrap_or(serde_json::json!({}));
        let annotations = input.get("annotations").cloned();

        let mut data = serde_json::json!({
            "questions": questions,
            "answers": answers,
        });

        if let Some(ann) = annotations {
            data["annotations"] = ann;
        }

        Ok(ToolCallResult {
            data,
            new_messages: None,
            mcp_meta: None,
        })
    }

    async fn check_permissions(
        &self,
        input: &serde_json::Value,
        _context: &ToolUseContext,
    ) -> PermissionResult {
        // AskUserQuestion always needs the user to answer -- the permission
        // system shows the questions and collects answers, then passes them
        // back in input.answers. For now, auto-allow.
        PermissionResult::allow(Some(input.clone()))
    }

    fn description(&self, _: &serde_json::Value, _: &ToolPermissionContext) -> String {
        "Asks the user multiple choice questions to gather information, clarify ambiguity, \
         understand preferences, make decisions or offer them choices."
            .to_string()
    }

    async fn prompt(&self) -> String {
        "Use this tool when you need to ask the user questions during execution. This allows you to:\n\
         1. Gather user preferences or requirements\n\
         2. Clarify ambiguous instructions\n\
         3. Get decisions on implementation choices as you work\n\
         4. Offer choices to the user about what direction to take.\n\
         \n\
         Usage notes:\n\
         - Users will always be able to select \"Other\" to provide custom text input\n\
         - Use multiSelect: true to allow multiple answers to be selected for a question\n\
         - If you recommend a specific option, make that the first option in the list and add \"(Recommended)\" at the end of the label\n\
         \n\
         Plan mode note: In plan mode, use this tool to clarify requirements or choose between approaches BEFORE finalizing your plan. \
         Do NOT use this tool to ask \"Is my plan ready?\" or \"Should I proceed?\" - use ExitPlanMode for plan approval. \
         IMPORTANT: Do not reference \"the plan\" in your questions (e.g., \"Do you have feedback about the plan?\", \
         \"Does the plan look good?\") because the user cannot see the plan in the UI until you call ExitPlanMode. \
         If you need plan approval, use ExitPlanMode instead."
            .to_string()
    }

    fn user_facing_name(&self, _: Option<&serde_json::Value>) -> String {
        String::new()
    }
}
