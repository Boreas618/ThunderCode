//! Output style / theme constants.

use serde::{Deserialize, Serialize};

/// The name of the default (no-op) output style.
pub const DEFAULT_OUTPUT_STYLE_NAME: &str = "default";

/// Where an output style was loaded from.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum OutputStyleSource {
    BuiltIn,
    UserSettings,
    ProjectSettings,
    PolicySettings,
    Plugin,
}

/// Configuration for a single output style / theme.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OutputStyleConfig {
    pub name: String,
    pub description: String,
    pub prompt: String,
    pub source: OutputStyleSource,
    /// When `true`, the standard coding-instruction section is still included
    /// even when a custom output-style prompt replaces the intro.
    #[serde(default)]
    pub keep_coding_instructions: bool,
    /// If `true`, this style is auto-applied when its plugin is enabled.
    #[serde(default)]
    pub force_for_plugin: bool,
}

/// Build the built-in "Explanatory" output style config.
pub fn explanatory_style() -> OutputStyleConfig {
    OutputStyleConfig {
        name: "Explanatory".into(),
        description: "AI explains its implementation choices and codebase patterns".into(),
        source: OutputStyleSource::BuiltIn,
        keep_coding_instructions: true,
        force_for_plugin: false,
        prompt: concat!(
            "You are an interactive CLI tool that helps users with software engineering tasks. ",
            "In addition to software engineering tasks, you should provide educational insights ",
            "about the codebase along the way.\n\n",
            "You should be clear and educational, providing helpful explanations while remaining ",
            "focused on the task. Balance educational content with task completion. When providing ",
            "insights, you may exceed typical length constraints, but remain focused and relevant.\n\n",
            "# Explanatory Style Active\n",
            "## Insights\n",
            "In order to encourage learning, before and after writing code, always provide brief ",
            "educational explanations about implementation choices."
        )
        .into(),
    }
}

/// Build the built-in "Learning" output style config.
pub fn learning_style() -> OutputStyleConfig {
    OutputStyleConfig {
        name: "Learning".into(),
        description: "AI pauses and asks you to write small pieces of code for hands-on practice".into(),
        source: OutputStyleSource::BuiltIn,
        keep_coding_instructions: true,
        force_for_plugin: false,
        prompt: concat!(
            "You are an interactive CLI tool that helps users with software engineering tasks. ",
            "In addition to software engineering tasks, you should help users learn more about ",
            "the codebase through hands-on practice and educational insights.\n\n",
            "You should be collaborative and encouraging. Balance task completion with learning ",
            "by requesting user input for meaningful design decisions while handling routine ",
            "implementation yourself."
        )
        .into(),
    }
}
