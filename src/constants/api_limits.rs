//! API limits -- image, PDF, and media constants.
//!
//! Last verified: 2025-12-22

// ==========================================================================
// Image limits
// ==========================================================================

/// Maximum base64-encoded image size (API enforced).
/// The API rejects images where the base64 string length exceeds this value.
pub const API_IMAGE_MAX_BASE64_SIZE: usize = 5 * 1024 * 1024; // 5 MB

/// Target raw image size to stay under the base64 limit after encoding.
/// `raw_size * 4/3 = base64_size`, so `raw_size = base64_size * 3 / 4`.
pub const IMAGE_TARGET_RAW_SIZE: usize = API_IMAGE_MAX_BASE64_SIZE * 3 / 4; // 3.75 MB

/// Client-side maximum image width for resizing.
pub const IMAGE_MAX_WIDTH: u32 = 2000;

/// Client-side maximum image height for resizing.
pub const IMAGE_MAX_HEIGHT: u32 = 2000;

// ==========================================================================
// PDF limits
// ==========================================================================

/// Maximum raw PDF file size that fits within the API request limit after
/// encoding. 20 MB raw -> ~27 MB base64, leaving room for conversation context.
pub const PDF_TARGET_RAW_SIZE: usize = 20 * 1024 * 1024; // 20 MB

/// Maximum number of pages in a PDF accepted by the API.
pub const API_PDF_MAX_PAGES: u32 = 100;

/// Size threshold above which PDFs are extracted into page images instead of
/// being sent as base64 document blocks.
pub const PDF_EXTRACT_SIZE_THRESHOLD: usize = 3 * 1024 * 1024; // 3 MB

/// Maximum PDF file size for the page-extraction path.
pub const PDF_MAX_EXTRACT_SIZE: usize = 100 * 1024 * 1024; // 100 MB

/// Max pages the Read tool will extract in a single call.
pub const PDF_MAX_PAGES_PER_READ: u32 = 20;

/// PDFs with more pages than this get the reference treatment on @ mention
/// instead of being inlined into context.
pub const PDF_AT_MENTION_INLINE_THRESHOLD: u32 = 10;

// ==========================================================================
// Media limits
// ==========================================================================

/// Maximum number of media items (images + PDFs) allowed per API request.
pub const API_MAX_MEDIA_PER_REQUEST: u32 = 100;
