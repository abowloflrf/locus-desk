mod extract;
mod fetch;

pub use extract::{ExtractedDocument, extract_document};
pub use fetch::{ContentError, PageFetcher, SecurePageFetcher};

pub const MAX_SOURCE_BYTES: usize = 2 * 1024 * 1024;
pub const MAX_READER_BYTES: usize = 8 * 1024 * 1024;
