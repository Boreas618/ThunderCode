//! Command registry -- central lookup for all slash commands.
//!
//! Ported from ref/commands.ts: `COMMANDS`, `findCommand`, `getCommands`,
//! `meetsAvailabilityRequirement`, and related helpers.

use crate::types::command::{Command, CommandAvailability};

use crate::commands::commands;

// ============================================================================
// CommandRegistry
// ============================================================================

/// Central registry of all built-in slash commands.
///
/// At startup the registry is populated with every known command via
/// [`CommandRegistry::new`].  The REPL uses [`find`](CommandRegistry::find)
/// to dispatch user input, and [`list_available`](CommandRegistry::list_available)
/// to populate typeahead / `/help` output.
pub struct CommandRegistry {
    commands: Vec<Command>,
}

impl CommandRegistry {
    /// Create a new registry pre-populated with all built-in commands.
    pub fn new() -> Self {
        Self {
            commands: Self::get_all_commands(),
        }
    }

    /// Register an additional command (e.g. from a plugin or MCP server).
    pub fn register(&mut self, cmd: Command) {
        self.commands.push(cmd);
    }

    /// Find a command by exact name or alias.
    ///
    /// Returns `None` if no command matches.  The lookup checks:
    /// 1. The canonical `name` field.
    /// 2. The optional `user_facing_name`.
    /// 3. Every entry in `aliases`.
    pub fn find(&self, name: &str) -> Option<&Command> {
        self.commands.iter().find(|cmd| {
            let base = cmd.base();
            if base.name == name {
                return true;
            }
            if let Some(ref ufn) = base.user_facing_name {
                if ufn == name {
                    return true;
                }
            }
            if let Some(ref aliases) = base.aliases {
                if aliases.iter().any(|a| a == name) {
                    return true;
                }
            }
            false
        })
    }

    /// Return all commands available to the current authentication context.
    ///
    /// - Commands without an `availability` restriction are always included.
    /// - Commands with `availability` are included only when at least one of
    ///   the listed availabilities matches `auth` (e.g. `"primary-ai"` or
    ///   `"console"`).
    /// - Disabled commands (`is_enabled == false`) are excluded.
    /// - Hidden commands (`is_hidden == true`) are excluded.
    pub fn list_available(&self, auth: Option<&str>) -> Vec<&Command> {
        self.commands
            .iter()
            .filter(|cmd| {
                let base = cmd.base();

                // Skip disabled commands.
                if !cmd.is_enabled() {
                    return false;
                }

                // Skip hidden commands.
                if base.is_hidden == Some(true) {
                    return false;
                }

                // Check availability restrictions.
                if let Some(ref avail) = base.availability {
                    meets_availability(avail, auth)
                } else {
                    true
                }
            })
            .collect()
    }

    /// Return every command in the registry (no filtering).
    pub fn all(&self) -> &[Command] {
        &self.commands
    }

    /// Return a mutable reference to the command list.
    pub fn all_mut(&mut self) -> &mut Vec<Command> {
        &mut self.commands
    }

    /// Whether a command with the given name or alias exists.
    pub fn has(&self, name: &str) -> bool {
        self.find(name).is_some()
    }

    /// Remove all dynamically-registered commands and reload built-ins.
    pub fn reload(&mut self) {
        self.commands = Self::get_all_commands();
    }

    /// Build the full list of built-in commands.
    ///
    /// This is the Rust equivalent of the memoized `COMMANDS()` array in
    /// the TypeScript reference.  Every command module exports a constructor
    /// that returns a `Command` value.
    ///
    /// The ordering mirrors the ref COMMANDS array for consistency.
    pub fn get_all_commands() -> Vec<Command> {
        let mut cmds = Vec::with_capacity(80);

        // -- Session --
        cmds.push(commands::session::add_dir());
        cmds.push(commands::session::btw());
        cmds.push(commands::session::color());
        cmds.push(commands::session::copy());
        cmds.push(commands::session::context());
        cmds.push(commands::session::context_non_interactive());
        cmds.push(commands::session::export());
        cmds.push(commands::session::rename());
        cmds.push(commands::session::resume());
        cmds.push(commands::session::session());
        cmds.push(commands::session::rewind());
        cmds.push(commands::session::tag());
        cmds.push(commands::session::summary());

        // -- Git --
        cmds.push(commands::git::branch());
        cmds.push(commands::git::commit());
        cmds.push(commands::git::commit_push_pr());
        cmds.push(commands::git::diff());
        cmds.push(commands::git::init());
        cmds.push(commands::git::pr_comments());
        cmds.push(commands::git::status());

        // -- Model / Inference --
        cmds.push(commands::model::advisor());
        cmds.push(commands::model::cost());
        cmds.push(commands::model::effort());
        cmds.push(commands::model::extra_usage());
        cmds.push(commands::model::extra_usage_non_interactive());
        cmds.push(commands::model::fast());
        cmds.push(commands::model::model());
        cmds.push(commands::model::passes());
        cmds.push(commands::model::rate_limit_options());
        cmds.push(commands::model::usage());

        // -- Tools / Integrations --
        cmds.push(commands::tools::agents());
        cmds.push(commands::tools::files());
        cmds.push(commands::tools::hooks());
        cmds.push(commands::tools::ide());
        cmds.push(commands::tools::mcp());
        cmds.push(commands::tools::memory());
        cmds.push(commands::tools::plugin());
        cmds.push(commands::tools::reload_plugins());
        cmds.push(commands::tools::skills());

        // -- Settings --
        cmds.push(commands::settings::config());
        cmds.push(commands::settings::keybindings());
        cmds.push(commands::settings::output_style());
        cmds.push(commands::settings::permissions());
        cmds.push(commands::settings::privacy_settings());
        cmds.push(commands::settings::remote_env());
        cmds.push(commands::settings::sandbox());
        cmds.push(commands::settings::theme());

        // -- Features --
        cmds.push(commands::features::compact());
        cmds.push(commands::features::plan());
        cmds.push(commands::features::tasks());
        cmds.push(commands::features::think_back());
        cmds.push(commands::features::thinkback_play());
        cmds.push(commands::features::vim());

        // -- System --
        cmds.push(commands::system::chrome());
        cmds.push(commands::system::clear());
        cmds.push(commands::system::desktop());
        cmds.push(commands::system::doctor());
        cmds.push(commands::system::exit());
        cmds.push(commands::system::feedback());
        cmds.push(commands::system::heapdump());
        cmds.push(commands::system::help());
        cmds.push(commands::system::install_github_app());
        cmds.push(commands::system::install_slack_app());
        cmds.push(commands::system::login());
        cmds.push(commands::system::logout());
        cmds.push(commands::system::mobile());
        cmds.push(commands::system::release_notes());
        cmds.push(commands::system::stats());
        cmds.push(commands::system::stickers());
        cmds.push(commands::system::terminal_setup());
        cmds.push(commands::system::upgrade());
        cmds.push(commands::system::version());

        // -- Review / Prompt --
        cmds.push(commands::review::insights());
        cmds.push(commands::review::review());
        cmds.push(commands::review::ultrareview());
        cmds.push(commands::review::security_review());
        cmds.push(commands::review::statusline());

        cmds
    }
}

impl Default for CommandRegistry {
    fn default() -> Self {
        Self::new()
    }
}

// ============================================================================
// Helpers
// ============================================================================

/// Check whether the given availability list is satisfied by the current
/// auth context string.
///
/// Ported from `meetsAvailabilityRequirement` in ref/commands.ts.
fn meets_availability(avail: &[CommandAvailability], auth: Option<&str>) -> bool {
    let auth = match auth {
        Some(a) => a,
        // No auth info -- only universal commands pass.
        None => return false,
    };

    for a in avail {
        match a {
            CommandAvailability::Authenticated => return true,
        }
    }
    false
}

// ============================================================================
// Built-in command name set
// ============================================================================

/// Returns the set of all built-in command names and aliases.
///
/// Useful for detecting collisions with user-defined or plugin commands.
pub fn builtin_command_names() -> std::collections::HashSet<String> {
    let cmds = CommandRegistry::get_all_commands();
    let mut names = std::collections::HashSet::new();
    for cmd in &cmds {
        let base = cmd.base();
        names.insert(base.name.clone());
        if let Some(ref aliases) = base.aliases {
            for a in aliases {
                names.insert(a.clone());
            }
        }
    }
    names
}

// ============================================================================
// Format helpers
// ============================================================================

/// Format a command description with its source annotation for user-facing UI.
///
/// Ported from `formatDescriptionWithSource` in ref/commands.ts.
pub fn format_description_with_source(cmd: &Command) -> String {
    match cmd {
        Command::Prompt(p) => {
            let desc = &p.base.description;
            match p.source {
                crate::types::command::PromptCommandSource::Plugin => {
                    if let Some(ref info) = p.plugin_info {
                        if let Some(name) = info.plugin_manifest.get("name").and_then(|v| v.as_str())
                        {
                            return format!("({name}) {desc}");
                        }
                    }
                    format!("{desc} (plugin)")
                }
                crate::types::command::PromptCommandSource::Bundled => {
                    format!("{desc} (bundled)")
                }
                crate::types::command::PromptCommandSource::Builtin
                | crate::types::command::PromptCommandSource::Mcp => desc.clone(),
                other => {
                    let source_name = format!("{other:?}").to_lowercase();
                    format!("{desc} ({source_name})")
                }
            }
        }
        _ => cmd.base().description.clone(),
    }
}
