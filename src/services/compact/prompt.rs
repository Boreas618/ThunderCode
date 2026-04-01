//! Compact prompts and summary formatting.
//!
//! Ported from ref/services/compact/prompt.ts`. Contains the system prompts
//! used to instruct the model to produce a structured conversation summary,
//! and helpers to format the raw model output into the final summary text.

/// No-tools preamble prepended to all compact prompts. Prevents the model
/// from attempting tool calls during compaction (which would be rejected and
/// waste the single turn).
const NO_TOOLS_PREAMBLE: &str = "\
CRITICAL: Respond with TEXT ONLY. Do NOT call any tools.

- Do NOT use Read, Bash, Grep, Glob, Edit, Write, or ANY other tool.
- You already have all the context you need in the conversation above.
- Tool calls will be REJECTED and will waste your only turn -- you will fail the task.
- Your entire response must be plain text: an <analysis> block followed by a <summary> block.

";

/// Detailed analysis instructions scoped to the full conversation.
const DETAILED_ANALYSIS_INSTRUCTION_BASE: &str = "\
Before providing your final summary, wrap your analysis in <analysis> tags to organize \
your thoughts and ensure you've covered all necessary points. In your analysis process:

1. Chronologically analyze each message and section of the conversation. For each section thoroughly identify:
   - The user's explicit requests and intents
   - Your approach to addressing the user's requests
   - Key decisions, technical concepts and code patterns
   - Specific details like:
     - file names
     - full code snippets
     - function signatures
     - file edits
   - Errors that you ran into and how you fixed them
   - Pay special attention to specific user feedback that you received, especially if the user told you to do something differently.
2. Double-check for technical accuracy and completeness, addressing each required element thoroughly.";

/// The base (full conversation) compact prompt.
const BASE_COMPACT_PROMPT: &str = "\
Your task is to create a detailed summary of the conversation so far, paying close attention \
to the user's explicit requests and your previous actions.
This summary should be thorough in capturing technical details, code patterns, and architectural \
decisions that would be essential for continuing development work without losing context.

{ANALYSIS}

Your summary should include the following sections:

1. Primary Request and Intent: Capture all of the user's explicit requests and intents in detail
2. Key Technical Concepts: List all important technical concepts, technologies, and frameworks discussed.
3. Files and Code Sections: Enumerate specific files and code sections examined, modified, or created. \
Pay special attention to the most recent messages and include full code snippets where applicable \
and include a summary of why this file read or edit is important.
4. Errors and fixes: List all errors that you ran into, and how you fixed them. \
Pay special attention to specific user feedback that you received, especially if the user told you \
to do something differently.
5. Problem Solving: Document problems solved and any ongoing troubleshooting efforts.
6. All user messages: List ALL user messages that are not tool results. These are critical for \
understanding the users' feedback and changing intent.
7. Pending Tasks: Outline any pending tasks that you have explicitly been asked to work on.
8. Current Work: Describe in detail precisely what was being worked on immediately before this \
summary request, paying special attention to the most recent messages from both user and assistant. \
Include file names and code snippets where applicable.
9. Optional Next Step: List the next step that you will take that is related to the most recent \
work you were doing. IMPORTANT: ensure that this step is DIRECTLY in line with the user's most \
recent explicit requests, and the task you were working on immediately before this summary request. \
If your last task was concluded, then only list next steps if they are explicitly in line with \
the users request. Do not start on tangential requests or really old requests that were already \
completed without confirming with the user first.
   If there is a next step, include direct quotes from the most recent conversation showing exactly \
what task you were working on and where you left off. This should be verbatim to ensure there's \
no drift in task interpretation.

Please provide your summary based on the conversation so far, following this structure and \
ensuring precision and thoroughness in your response.";

/// Trailing reminder appended to all compact prompts.
const NO_TOOLS_TRAILER: &str = "\n\nREMINDER: Do NOT call any tools. Respond with plain text only -- \
an <analysis> block followed by a <summary> block. \
Tool calls will be rejected and you will fail the task.";

/// Build the full compaction system prompt, optionally including custom
/// instructions (e.g. user-provided compact instructions from RULES.md).
pub fn get_compact_prompt(custom_instructions: Option<&str>) -> String {
    let analysis = DETAILED_ANALYSIS_INSTRUCTION_BASE;
    let body = BASE_COMPACT_PROMPT.replace("{ANALYSIS}", analysis);

    let mut prompt = format!("{NO_TOOLS_PREAMBLE}{body}");

    if let Some(instructions) = custom_instructions {
        let trimmed = instructions.trim();
        if !trimmed.is_empty() {
            prompt.push_str(&format!("\n\nAdditional Instructions:\n{trimmed}"));
        }
    }

    prompt.push_str(NO_TOOLS_TRAILER);
    prompt
}

/// Format the raw model output by stripping the `<analysis>` scratchpad and
/// extracting the `<summary>` section.
///
/// The analysis block improves summary quality but has no informational value
/// once the summary is written. The `<summary>` tags are replaced with a
/// readable "Summary:" header.
pub fn format_compact_summary(raw: &str) -> String {
    // Strip analysis section.
    let re_analysis = regex::Regex::new(r"(?s)<analysis>.*?</analysis>").unwrap();
    let without_analysis = re_analysis.replace(raw, "");

    // Extract and format summary section.
    let re_summary = regex::Regex::new(r"(?s)<summary>(.*?)</summary>").unwrap();
    let formatted = if let Some(caps) = re_summary.captures(&without_analysis) {
        let content = caps.get(1).map_or("", |m| m.as_str()).trim();
        re_summary
            .replace(&without_analysis, format!("Summary:\n{content}"))
            .into_owned()
    } else {
        without_analysis.into_owned()
    };

    // Clean up extra whitespace between sections.
    let re_blank = regex::Regex::new(r"\n{3,}").unwrap();
    re_blank.replace_all(formatted.trim(), "\n\n").into_owned()
}

/// Build the user-visible message that wraps the formatted summary.
///
/// This message is injected at the start of the post-compact conversation so
/// the model has full context for continuing work.
pub fn get_compact_user_summary_message(
    summary: &str,
    suppress_follow_up_questions: bool,
    transcript_path: Option<&str>,
    recent_messages_preserved: bool,
) -> String {
    let formatted = format_compact_summary(summary);

    let mut msg = format!(
        "This session is being continued from a previous conversation that ran out of context. \
         The summary below covers the earlier portion of the conversation.\n\n{formatted}"
    );

    if let Some(path) = transcript_path {
        msg.push_str(&format!(
            "\n\nIf you need specific details from before compaction (like exact code snippets, \
             error messages, or content you generated), read the full transcript at: {path}"
        ));
    }

    if recent_messages_preserved {
        msg.push_str("\n\nRecent messages are preserved verbatim.");
    }

    if suppress_follow_up_questions {
        msg.push_str(
            "\nContinue the conversation from where it left off without asking the user any \
             further questions. Resume directly -- do not acknowledge the summary, do not recap \
             what was happening, do not preface with \"I'll continue\" or similar. Pick up the \
             last task as if the break never happened.",
        );
    }

    msg
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn format_strips_analysis() {
        let raw = "<analysis>thinking...</analysis>\n\n<summary>the result</summary>";
        let formatted = format_compact_summary(raw);
        assert!(!formatted.contains("<analysis>"));
        assert!(formatted.contains("Summary:"));
        assert!(formatted.contains("the result"));
    }

    #[test]
    fn format_handles_no_tags() {
        let raw = "plain text summary without XML tags";
        let formatted = format_compact_summary(raw);
        assert_eq!(formatted, raw);
    }

    #[test]
    fn compact_prompt_includes_trailer() {
        let prompt = get_compact_prompt(None);
        assert!(prompt.contains("REMINDER: Do NOT call any tools"));
    }

    #[test]
    fn compact_prompt_includes_custom_instructions() {
        let prompt = get_compact_prompt(Some("Focus on Rust code changes."));
        assert!(prompt.contains("Focus on Rust code changes."));
    }

    #[test]
    fn summary_message_with_transcript() {
        let msg = get_compact_user_summary_message(
            "<summary>did stuff</summary>",
            false,
            Some("/tmp/transcript.jsonl"),
            false,
        );
        assert!(msg.contains("Summary:"));
        assert!(msg.contains("/tmp/transcript.jsonl"));
    }
}
