//! Memory relevance scoring.
//!
//! Ported from ref/memdir/findRelevantMemories.ts`.
//!
//! In the TypeScript reference, relevance selection uses a Sonnet side-query
//! to pick the most relevant memories. This Rust port provides a local
//! keyword-based scoring fallback that works without an API call, plus
//! the data structures needed to integrate with an LLM-based selector.

use crate::memory::types::MemoryFile;

// ---------------------------------------------------------------------------
// Local keyword scoring
// ---------------------------------------------------------------------------

/// Find memories relevant to a query using local keyword matching.
///
/// Returns a list of `(score, &MemoryFile)` tuples sorted by descending
/// relevance score. Only memories with a positive score are returned.
///
/// Scoring strategy:
/// - Each word in the query is checked against the memory's name,
///   description, and content.
/// - Exact word matches in the description are weighted higher than
///   matches in the content body.
/// - Results are capped at 5 (matching the TypeScript reference's limit).
pub fn find_relevant_memories<'a>(
    query: &str,
    memories: &'a [MemoryFile],
) -> Vec<(f64, &'a MemoryFile)> {
    if query.trim().is_empty() || memories.is_empty() {
        return Vec::new();
    }

    let query_words = tokenize(query);
    if query_words.is_empty() {
        return Vec::new();
    }

    let mut scored: Vec<(f64, &MemoryFile)> = memories
        .iter()
        .map(|m| {
            let score = score_memory(&query_words, m);
            (score, m)
        })
        .filter(|(score, _)| *score > 0.0)
        .collect();

    // Sort by descending score
    scored.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap_or(std::cmp::Ordering::Equal));

    // Cap at 5
    scored.truncate(5);
    scored
}

/// Score a single memory against query words.
fn score_memory(query_words: &[String], memory: &MemoryFile) -> f64 {
    let name_lower = memory.name.to_lowercase();
    let desc_lower = memory.description.to_lowercase();
    let content_lower = memory.content.to_lowercase();

    let mut score = 0.0;

    for word in query_words {
        // Name matches are highest value
        if name_lower.contains(word.as_str()) {
            score += 3.0;
        }
        // Description matches are high value (used for relevance decisions)
        if desc_lower.contains(word.as_str()) {
            score += 2.0;
        }
        // Content matches contribute less
        if content_lower.contains(word.as_str()) {
            score += 1.0;
        }
    }

    // Normalize by number of query words so longer queries don't
    // automatically score higher than shorter ones.
    score / query_words.len() as f64
}

/// Tokenize a query into lowercase words, filtering stop words.
fn tokenize(text: &str) -> Vec<String> {
    let stop_words: std::collections::HashSet<&str> = [
        "a", "an", "the", "is", "are", "was", "were", "be", "been", "being",
        "have", "has", "had", "do", "does", "did", "will", "would", "could",
        "should", "may", "might", "shall", "can", "to", "of", "in", "for",
        "on", "with", "at", "by", "from", "as", "into", "through", "during",
        "before", "after", "above", "below", "between", "and", "but", "or",
        "not", "no", "so", "if", "then", "else", "when", "while", "where",
        "how", "what", "which", "who", "whom", "this", "that", "these",
        "those", "i", "me", "my", "we", "our", "you", "your", "it", "its",
        "they", "them", "their",
    ]
    .iter()
    .copied()
    .collect();

    text.split_whitespace()
        .map(|w| w.to_lowercase())
        .map(|w| w.trim_matches(|c: char| !c.is_alphanumeric()).to_string())
        .filter(|w| w.len() >= 2 && !stop_words.contains(w.as_str()))
        .collect()
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn make_memory(name: &str, desc: &str, content: &str) -> MemoryFile {
        MemoryFile {
            path: PathBuf::from(format!("/test/{name}.md")),
            name: name.to_string(),
            description: desc.to_string(),
            memory_type: None,
            content: content.to_string(),
        }
    }

    #[test]
    fn empty_query() {
        let memories = vec![make_memory("test", "test", "test")];
        assert!(find_relevant_memories("", &memories).is_empty());
    }

    #[test]
    fn empty_memories() {
        assert!(find_relevant_memories("hello", &[]).is_empty());
    }

    #[test]
    fn keyword_match() {
        let memories = vec![
            make_memory("database", "database connection settings", "Use PostgreSQL."),
            make_memory("testing", "testing preferences", "Always use real DB in tests."),
            make_memory("user_role", "user is a senior engineer", "10 years experience."),
        ];

        let results = find_relevant_memories("database settings", &memories);
        assert!(!results.is_empty());
        // "database" should be the top result
        assert_eq!(results[0].1.name, "database");
    }

    #[test]
    fn max_five_results() {
        let memories: Vec<MemoryFile> = (0..10)
            .map(|i| make_memory(&format!("mem{i}"), &format!("keyword {i}"), "keyword content"))
            .collect();

        let results = find_relevant_memories("keyword", &memories);
        assert!(results.len() <= 5);
    }

    #[test]
    fn name_matches_score_highest() {
        let memories = vec![
            make_memory("rust", "unrelated description", "unrelated content"),
            make_memory("unrelated", "mentions rust lang", "unrelated content"),
            make_memory("unrelated2", "unrelated desc", "some rust content here"),
        ];

        let results = find_relevant_memories("rust", &memories);
        assert!(!results.is_empty());
        // Name match should score highest
        assert_eq!(results[0].1.name, "rust");
    }

    #[test]
    fn tokenize_filters_stop_words() {
        let words = tokenize("the quick brown fox is a test");
        assert!(words.contains(&"quick".to_string()));
        assert!(words.contains(&"brown".to_string()));
        assert!(words.contains(&"fox".to_string()));
        assert!(words.contains(&"test".to_string()));
        assert!(!words.contains(&"the".to_string()));
        assert!(!words.contains(&"is".to_string()));
        assert!(!words.contains(&"a".to_string()));
    }
}
