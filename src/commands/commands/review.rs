//! Review & prompt commands: /review, /ultrareview, /security-review,
//! /statusline, /insights.
//!
//! Ported from ref/commands/review.ts, security-review.ts, statusline.tsx,
//! insights.ts.

use crate::types::command::{Command, LocalJsxCommandData, PromptCommandData, PromptCommandSource};

use super::base;

// ============================================================================
// /review
// ============================================================================

/// The prompt template for the /review command.
///
/// Ported from ref/commands/review.ts LOCAL_REVIEW_PROMPT.
/// At runtime the user-supplied PR number is appended.
const REVIEW_PROMPT: &str = r#"
      You are an expert code reviewer. Follow these steps:

      1. If no PR number is provided in the args, run `gh pr list` to show open PRs
      2. If a PR number is provided, run `gh pr view <number>` to get PR details
      3. Run `gh pr diff <number>` to get the diff
      4. Analyze the changes and provide a thorough code review that includes:
         - Overview of what the PR does
         - Analysis of code quality and style
         - Specific suggestions for improvements
         - Any potential issues or risks

      Keep your review concise but thorough. Focus on:
      - Code correctness
      - Following project conventions
      - Performance implications
      - Test coverage
      - Security considerations

      Format your review with clear sections and bullet points.

      PR number: "#;

/// `/review` -- Review a pull request.
///
/// Type: prompt | Source: builtin
pub fn review() -> Command {
    let b = base("review", "Review a pull request");
    Command::Prompt(PromptCommandData {
        base: b,
        progress_message: "reviewing pull request".into(),
        content_length: REVIEW_PROMPT.len(),
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
// /ultrareview
// ============================================================================

/// `/ultrareview` -- Remote bughunter review (runs in ThunderCode on the web).
///
/// Type: local-jsx
/// In the TS reference, gated on isUltrareviewEnabled().
/// Default to disabled; runtime enables when conditions are met.
pub fn ultrareview() -> Command {
    let mut b = base(
        "ultrareview",
        "~10-20 min - Finds and verifies bugs in your branch. Runs in ThunderCode on the web.",
    );
    b.is_enabled = Some(false);
    b.is_hidden = Some(true);
    Command::LocalJsx(LocalJsxCommandData { base: b })
}

// ============================================================================
// /security-review
// ============================================================================

/// Full prompt for the security-review command.
///
/// Ported from ref/commands/security-review.ts (SECURITY_REVIEW_MARKDOWN).
const SECURITY_REVIEW_PROMPT: &str = r#"You are a senior security engineer conducting a focused security review of the changes on this branch.

GIT STATUS:

```
!`git status`
```

FILES MODIFIED:

```
!`git diff --name-only origin/HEAD...`
```

COMMITS:

```
!`git log --no-decorate origin/HEAD...`
```

DIFF CONTENT:

```
!`git diff origin/HEAD...`
```

Review the complete diff above. This contains all code changes in the PR.


OBJECTIVE:
Perform a security-focused code review to identify HIGH-CONFIDENCE security vulnerabilities that could have real exploitation potential. This is not a general code review - focus ONLY on security implications newly added by this PR. Do not comment on existing security concerns.

CRITICAL INSTRUCTIONS:
1. MINIMIZE FALSE POSITIVES: Only flag issues where you're >80% confident of actual exploitability
2. AVOID NOISE: Skip theoretical issues, style concerns, or low-impact findings
3. FOCUS ON IMPACT: Prioritize vulnerabilities that could lead to unauthorized access, data breaches, or system compromise
4. EXCLUSIONS: Do NOT report the following issue types:
   - Denial of Service (DOS) vulnerabilities, even if they allow service disruption
   - Secrets or sensitive data stored on disk (these are handled by other processes)
   - Rate limiting or resource exhaustion issues

SECURITY CATEGORIES TO EXAMINE:

**Input Validation Vulnerabilities:**
- SQL injection via unsanitized user input
- Command injection in system calls or subprocesses
- XXE injection in XML parsing
- Template injection in templating engines
- NoSQL injection in database queries
- Path traversal in file operations

**Authentication & Authorization Issues:**
- Authentication bypass logic
- Privilege escalation paths
- Session management flaws
- JWT token vulnerabilities
- Authorization logic bypasses

**Crypto & Secrets Management:**
- Hardcoded API keys, passwords, or tokens
- Weak cryptographic algorithms or implementations
- Improper key storage or management
- Cryptographic randomness issues
- Certificate validation bypasses

**Injection & Code Execution:**
- Remote code execution via deseralization
- Pickle injection in Python
- YAML deserialization vulnerabilities
- Eval injection in dynamic code execution
- XSS vulnerabilities in web applications (reflected, stored, DOM-based)

**Data Exposure:**
- Sensitive data logging or storage
- PII handling violations
- API endpoint data leakage
- Debug information exposure

Additional notes:
- Even if something is only exploitable from the local network, it can still be a HIGH severity issue

ANALYSIS METHODOLOGY:

Phase 1 - Repository Context Research (Use file search tools):
- Identify existing security frameworks and libraries in use
- Look for established secure coding patterns in the codebase
- Examine existing sanitization and validation patterns
- Understand the project's security model and threat model

Phase 2 - Comparative Analysis:
- Compare new code changes against existing security patterns
- Identify deviations from established secure practices
- Look for inconsistent security implementations
- Flag code that introduces new attack surfaces

Phase 3 - Vulnerability Assessment:
- Examine each modified file for security implications
- Trace data flow from user inputs to sensitive operations
- Look for privilege boundaries being crossed unsafely
- Identify injection points and unsafe deserialization

REQUIRED OUTPUT FORMAT:

You MUST output your findings in markdown. The markdown output should contain the file, line number, severity, category (e.g. `sql_injection` or `xss`), description, exploit scenario, and fix recommendation.

For example:

# Vuln 1: XSS: `foo.py:42`

* Severity: High
* Description: User input from `username` parameter is directly interpolated into HTML without escaping, allowing reflected XSS attacks
* Exploit Scenario: Attacker crafts URL like /bar?q=<script>alert(document.cookie)</script> to execute JavaScript in victim's browser, enabling session hijacking or data theft
* Recommendation: Use Flask's escape() function or Jinja2 templates with auto-escaping enabled for all user inputs rendered in HTML

SEVERITY GUIDELINES:
- **HIGH**: Directly exploitable vulnerabilities leading to RCE, data breach, or authentication bypass
- **MEDIUM**: Vulnerabilities requiring specific conditions but with significant impact
- **LOW**: Defense-in-depth issues or lower-impact vulnerabilities

CONFIDENCE SCORING:
- 0.9-1.0: Certain exploit path identified, tested if possible
- 0.8-0.9: Clear vulnerability pattern with known exploitation methods
- 0.7-0.8: Suspicious pattern requiring specific conditions to exploit
- Below 0.7: Don't report (too speculative)

FINAL REMINDER:
Focus on HIGH and MEDIUM findings only. Better to miss some theoretical issues than flood the report with false positives. Each finding should be something a security engineer would confidently raise in a PR review.

FALSE POSITIVE FILTERING:

> You do not need to run commands to reproduce the vulnerability, just read the code to determine if it is a real vulnerability. Do not use the bash tool or write to any files.
>
> HARD EXCLUSIONS - Automatically exclude findings matching these patterns:
> 1. Denial of Service (DOS) vulnerabilities or resource exhaustion attacks.
> 2. Secrets or credentials stored on disk if they are otherwise secured.
> 3. Rate limiting concerns or service overload scenarios.
> 4. Memory consumption or CPU exhaustion issues.
> 5. Lack of input validation on non-security-critical fields without proven security impact.
> 6. Input sanitization concerns for GitHub Action workflows unless they are clearly triggerable via untrusted input.
> 7. A lack of hardening measures. Code is not expected to implement all security best practices, only flag concrete vulnerabilities.
> 8. Race conditions or timing attacks that are theoretical rather than practical issues. Only report a race condition if it is concretely problematic.
> 9. Vulnerabilities related to outdated third-party libraries. These are managed separately and should not be reported here.
> 10. Memory safety issues such as buffer overflows or use-after-free-vulnerabilities are impossible in rust. Do not report memory safety issues in rust or any other memory safe languages.
> 11. Files that are only unit tests or only used as part of running tests.
> 12. Log spoofing concerns. Outputting un-sanitized user input to logs is not a vulnerability.
> 13. SSRF vulnerabilities that only control the path. SSRF is only a concern if it can control the host or protocol.
> 14. Including user-controlled content in AI system prompts is not a vulnerability.
> 15. Regex injection. Injecting untrusted content into a regex is not a vulnerability.
> 16. Regex DOS concerns.
> 16. Insecure documentation. Do not report any findings in documentation files such as markdown files.
> 17. A lack of audit logs is not a vulnerability.

START ANALYSIS:

Begin your analysis now. Do this in 3 steps:

1. Use a sub-task to identify vulnerabilities. Use the repository exploration tools to understand the codebase context, then analyze the PR changes for security implications. In the prompt for this sub-task, include all of the above.
2. Then for each vulnerability identified by the above sub-task, create a new sub-task to filter out false-positives. Launch these sub-tasks as parallel sub-tasks. In the prompt for these sub-tasks, include everything in the "FALSE POSITIVE FILTERING" instructions.
3. Filter out any vulnerabilities where the sub-task reported a confidence less than 8.

Your final reply must contain the markdown report and nothing else."#;

/// Allowed tools for /security-review.
const SECURITY_REVIEW_ALLOWED_TOOLS: &[&str] = &[
    "Bash(git diff:*)",
    "Bash(git status:*)",
    "Bash(git log:*)",
    "Bash(git show:*)",
    "Bash(git remote show:*)",
    "Read",
    "Glob",
    "Grep",
    "LS",
    "Task",
];

/// `/security-review` -- Complete a security review of pending branch changes.
///
/// Type: prompt | Source: builtin
pub fn security_review() -> Command {
    let b = base(
        "security-review",
        "Complete a security review of the pending changes on the current branch",
    );
    Command::Prompt(PromptCommandData {
        base: b,
        progress_message: "analyzing code changes for security risks".into(),
        content_length: SECURITY_REVIEW_PROMPT.len(),
        arg_names: None,
        allowed_tools: Some(
            SECURITY_REVIEW_ALLOWED_TOOLS
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
// /statusline
// ============================================================================

/// The prompt content for the /statusline command.
///
/// Ported from ref/commands/statusline.tsx.
const STATUSLINE_PROMPT: &str = r#"Create an Agent with subagent_type "statusline-setup" and the prompt "Configure my statusLine from my shell PS1 configuration""#;

/// Allowed tools for /statusline.
const STATUSLINE_ALLOWED_TOOLS: &[&str] = &[
    "Agent",
    "Read(~/**)",
    "Edit(~/.thundercode/settings.json)",
];

/// `/statusline` -- Set up ThunderCode's status line UI.
///
/// Type: prompt | Source: builtin | disableNonInteractive: true
pub fn statusline() -> Command {
    let b = base("statusline", "Set up ThunderCode's status line UI");
    Command::Prompt(PromptCommandData {
        base: b,
        progress_message: "setting up statusLine".into(),
        content_length: STATUSLINE_PROMPT.len(),
        arg_names: None,
        allowed_tools: Some(
            STATUSLINE_ALLOWED_TOOLS
                .iter()
                .map(|s| s.to_string())
                .collect(),
        ),
        model: None,
        source: PromptCommandSource::Builtin,
        plugin_info: None,
        disable_non_interactive: Some(true),
        hooks: None,
        skill_root: None,
        context: None,
        agent: None,
        effort: None,
        paths: None,
    })
}

// ============================================================================
// /insights
// ============================================================================

/// `/insights` -- Generate a report analyzing your ThunderCode sessions.
///
/// Type: prompt | Source: builtin
/// Lazy-loaded in the TS reference (113KB module).
pub fn insights() -> Command {
    let b = base(
        "insights",
        "Generate a report analyzing your ThunderCode sessions",
    );
    Command::Prompt(PromptCommandData {
        base: b,
        progress_message: "analyzing your sessions".into(),
        content_length: 0, // Dynamic content
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
