//! RecordRoute Vector Search Engine
//!
//! Text embedding and vector search functionality

mod engine;
mod similarity;
mod types;

pub use engine::VectorSearchEngine;
pub use similarity::{cosine_similarity, normalize};
pub use types::{SearchResult, VectorEntry, VectorIndex, VectorMetadata};
