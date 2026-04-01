//! Git commands: /branch, /commit, /commit-push-pr, /diff, /status, /init,
//! /pr-comments.
//!
//! Ported from ref/commands/branch, commit.ts, commit-push-pr.ts, diff, status,
//! init.ts, pr_comments.

use crate::types::command::{
    Command, LocalJsxCommandData, PromptCommandData, PromptCommandSource,
};

use super::{base, base_with_aliases};

// ============================================================================
// /branch
// ============================================================================

/// `/branch` -- Create a branch of the current conversation at this point.
///
/// Type: local-jsx | Aliases: fork
pub fn branch() -> Command {
    let mut b = base_with_aliases(
        "branch",
        "Create a branch of the current conversation at this point",
        vec!["fork"],
    );
    b.argument_hint = Some("[name]".into());
    Command::LocalJsx(LocalJsxCommandData { base: b })
}

// ============================================================================
// /commit
// ============================================================================

/// The prompt content for the /commit command.
///
/// At runtime the `!`-prefixed shell commands inside the prompt are
/// expanded before being sent to the model.
const COMMIT_PROMPT: &str = r#"## Context

- Current git status: !`git status`
- Current git diff (staged and unstaged changes): !`git diff HEAD`
- Current branch: !`git branch --show-current`
- Recent commits: !`git log --oneline -10`

## Git Safety Protocol

- NEVER update the git config
- NEVER skip hooks (--no-verify, --no-gpg-sign, etc) unless the user explicitly requests it
- CRITICAL: ALWAYS create NEW commits. NEVER use git commit --amend, unless the user explicitly requests it
- Do not commit files that likely contain secrets (.env, credentials.json, etc). Warn the user if they specifically request to commit those files
- If there are no changes to commit (i.e., no untracked files and no modifications), do not create an empty commit
- Never use git commands with the -i flag (like git rebase -i or git add -i) since they require interactive input which is not supported

## Your task

Based on the above changes, create a single git commit:

1. Analyze all staged changes and draft a commit message:
   - Look at the recent commits above to follow this repository's commit message style
   - Summarize the nature of the changes (new feature, enhancement, bug fix, refactoring, test, docs, etc.)
   - Ensure the message accurately reflects the changes and their purpose (i.e. "add" means a wholly new feature, "update" means an enhancement to an existing feature, "fix" means a bug fix, etc.)
   - Draft a concise (1-2 sentences) commit message that focuses on the "why" rather than the "what"

2. Stage relevant files and create the commit using HEREDOC syntax:
```
git commit -m "$(cat <<'EOF'
Commit message here.
EOF
)"
```

You have the capability to call multiple tools in a single response. Stage and create the commit using a single message. Do not use any other tools or do anything else. Do not send any other text or messages besides these tool calls."#;

/// Allowed tools for /commit.
const COMMIT_ALLOWED_TOOLS: &[&str] = &[
    "Bash(git add:*)",
    "Bash(git status:*)",
    "Bash(git commit:*)",
];

/// `/commit` -- Create a git commit.
///
/// Type: prompt | Source: builtin
pub fn commit() -> Command {
    let b = base("commit", "Create a git commit");
    Command::Prompt(PromptCommandData {
        base: b,
        progress_message: "creating commit".into(),
        content_length: COMMIT_PROMPT.len(),
        arg_names: None,
        allowed_tools: Some(
            COMMIT_ALLOWED_TOOLS
                .iter()
                .map(|s| s.to_string())
                .collect(),
        ),
        model: None,
        source: PromptCommandSource::Builtin,
        plugin_info: None,
        disable_non_interactive: None,
        hooks: None,
        skill_root: None,
        context: None,
        agent: None,
        effort: None,
        paths: None,
    })
}

// ============================================================================
// /commit-push-pr
// ============================================================================

/// The prompt content for the /commit-push-pr command.
///
/// At runtime `{defaultBranch}` is replaced with the actual default branch,
/// and shell commands prefixed with `!` are expanded.
const COMMIT_PUSH_PR_PROMPT: &str = r#"## Context

- `git status`: !`git status`
- `git diff HEAD`: !`git diff HEAD`
- `git branch --show-current`: !`git branch --show-current`
- `git diff main...HEAD`: !`git diff main...HEAD`
- `gh pr view --json number 2>/dev/null || true`: !`gh pr view --json number 2>/dev/null || true`

## Git Safety Protocol

- NEVER update the git config
- NEVER run destructive/irreversible git commands (like push --force, hard reset, etc) unless the user explicitly requests them
- NEVER skip hooks (--no-verify, --no-gpg-sign, etc) unless the user explicitly requests it
- NEVER run force push to main/master, warn the user if they request it
- Do not commit files that likely contain secrets (.env, credentials.json, etc)
- Never use git commands with the -i flag (like git rebase -i or git add -i) since they require interactive input which is not supported

## Your task

Analyze all changes that will be included in the pull request, making sure to look at all relevant commits (NOT just the latest commit, but ALL commits that will be included in the pull request from the git diff main...HEAD output above).

Based on the above changes:
1. Create a new branch if on main (use username for the branch name prefix, e.g., `username/feature-name`)
2. Create a single commit with an appropriate message using heredoc syntax:
```
git commit -m "$(cat <<'EOF'
Commit message here.
EOF
)"
```
3. Push the branch to origin
4. If a PR already exists for this branch (check the gh pr view output above), update the PR title and body using `gh pr edit` to reflect the current diff. Otherwise, create a pull request using `gh pr create` with heredoc syntax for the body.
   - IMPORTANT: Keep PR titles short (under 70 characters). Use the body for details.
```
gh pr create --title "Short, descriptive title" --body "$(cat <<'EOF'
## Summary
<1-3 bullet points>

## Test plan
[Bulleted markdown checklist of TODOs for testing the pull request...]
EOF
)"
```

You have the capability to call multiple tools in a single response. You MUST do all of the above in a single message.

Return the PR URL when you're done, so the user can see it."#;

/// Allowed tools for /commit-push-pr.
const COMMIT_PUSH_PR_ALLOWED_TOOLS: &[&str] = &[
    "Bash(git checkout --branch:*)",
    "Bash(git checkout -b:*)",
    "Bash(git add:*)",
    "Bash(git status:*)",
    "Bash(git push:*)",
    "Bash(git commit:*)",
    "Bash(gh pr create:*)",
    "Bash(gh pr edit:*)",
    "Bash(gh pr view:*)",
    "Bash(gh pr merge:*)",
    "ToolSearch",
];

/// `/commit-push-pr` -- Commit, push, and open a PR.
///
/// Type: prompt | Source: builtin
pub fn commit_push_pr() -> Command {
    let b = base("commit-push-pr", "Commit, push, and open a PR");
    Command::Prompt(PromptCommandData {
        base: b,
        progress_message: "creating commit and PR".into(),
        content_length: COMMIT_PUSH_PR_PROMPT.len(),
        arg_names: None,
        allowed_tools: Some(
            COMMIT_PUSH_PR_ALLOWED_TOOLS
                .iter()
                .map(|s| s.to_string())
                .collect(),
        ),
        model: None,
        source: PromptCommandSource::Builtin,
        plugin_info: None,
        disable_non_interactive: None,
        hooks: None,
        skill_root: None,
        context: None,
        agent: None,
        effort: None,
        paths: None,
    })
}

// ============================================================================
// /diff
// ============================================================================

/// `/diff` -- View uncommitted changes and per-turn diffs.
///
/// Type: local-jsx
pub fn diff() -> Command {
    let b = base("diff", "View uncommitted changes and per-turn diffs");
    Command::LocalJsx(LocalJsxCommandData { base: b })
}

// ============================================================================
// /status
// ============================================================================

/// `/status` -- Show ThunderCode status including version, model, account, etc.
///
/// Type: local-jsx | Immediate: true
pub fn status() -> Command {
    let mut b = base(
        "status",
        "Show ThunderCode status including version, model, account, API connectivity, and tool statuses",
    );
    b.immediate = Some(true);
    Command::LocalJsx(LocalJsxCommandData { base: b })
}

// ============================================================================
// /init
// ============================================================================

/// The prompt content for the /init command.
///
/// Ported from ref/commands/init.ts (OLD_INIT_PROMPT).
const INIT_PROMPT: &str = r#"Please analyze this codebase and create a RULES.md file, which will be given to future instances of ThunderCode to operate in this repository.

What to add:
1. Commands that will be commonly used, such as how to build, lint, and run tests. Include the necessary commands to develop in this codebase, such as how to run a single test.
2. High-level code architecture and structure so that future instances can be productive more quickly. Focus on the "big picture" architecture that requires reading multiple files to understand.

Usage notes:
- If there's already a RULES.md, suggest improvements to it.
- When you make the initial RULES.md, do not repeat yourself and do not include obvious instructions like "Provide helpful error messages to users", "Write unit tests for all new utilities", "Never include sensitive information (API keys, tokens) in code or commits".
- Avoid listing every component or file structure that can be easily discovered.
- Don't include generic development practices.
- If there are Cursor rules (in .cursor/rules/ or .cursorrules) or Copilot rules (in .github/copilot-instructions.md), make sure to include the important parts.
- If there is a README.md, make sure to include the important parts.
- Do not make up information such as "Common Development Tasks", "Tips for Development", "Support and Documentation" unless this is expressly included in other files that you read.
- Be sure to prefix the file with the following text:

```
# RULES.md

This file provides guidance to ThunderCode (primary.ai/code) when working with code in this repository.
```"#;

/// `/init` -- Initialize a new RULES.md file with codebase documentation.
///
/// Type: prompt | Source: builtin
pub fn init() -> Command {
    let b = base(
        "init",
        "Initialize a new RULES.md file with codebase documentation",
    );
    Command::Prompt(PromptCommandData {
        base: b,
        progress_message: "analyzing your codebase".into(),
        content_length: INIT_PROMPT.len(),
        arg_names: None,
        allowed_tools: None,
        model: None,
        source: PromptCommandSource::Builtin,
        plugin_info: None,
        disable_non_interactive: None,
        hooks: None,
        skill_root: None,
        context: None,
        agent: None,
        effort: None,
        paths: None,
    })
}

// ============================================================================
// /pr-comments
// ============================================================================

/// The prompt content for the /pr-comments command.
///
/// Ported from ref/commands/pr_comments/index.ts.
const PR_COMMENTS_PROMPT: &str = r#"You are an AI assistant integrated into a git-based version control system. Your task is to fetch and display comments from a GitHub pull request.

Follow these steps:

1. Use `gh pr view --json number,headRepository` to get the PR number and repository info
2. Use `gh api /repos/{owner}/{repo}/issues/{number}/comments` to get PR-level comments
3. Use `gh api /repos/{owner}/{repo}/pulls/{number}/comments` to get review comments. Pay particular attention to the following fields: `body`, `diff_hunk`, `path`, `line`, etc. If the comment references some code, consider fetching it using eg `gh api /repos/{owner}/{repo}/contents/{path}?ref={branch} | jq .content -r | base64 -d`
4. Parse and format all comments in a readable way
5. Return ONLY the formatted comments, with no additional text

Format the comments as:

## Comments

[For each comment thread:]
- @author file.ts#line:
  ```diff
  [diff_hunk from the API response]
  ```
  > quoted comment text

  [any replies indented]

If there are no comments, return "No comments found."

Remember:
1. Only show the actual comments, no explanatory text
2. Include both PR-level and code review comments
3. Preserve the threading/nesting of comment replies
4. Show the file and line number context for code review comments
5. Use jq to parse the JSON responses from the GitHub API"#;

/// `/pr-comments` -- Get comments from a GitHub pull request.
///
/// Type: prompt | Source: builtin
pub fn pr_comments() -> Command {
    let b = base("pr-comments", "Get comments from a GitHub pull request");
    Command::Prompt(PromptCommandData {
        base: b,
        progress_message: "fetching PR comments".into(),
        content_length: PR_COMMENTS_PROMPT.len(),
        arg_names: None,
        allowed_tools: None,
        model: None,
        source: PromptCommandSource::Builtin,
        plugin_info: None,
        disable_non_interactive: None,
        hooks: None,
        skill_root: None,
        context: None,
        agent: None,
        effort: None,
        paths: None,
    })
}
