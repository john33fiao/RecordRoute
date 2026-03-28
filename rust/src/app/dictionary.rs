use crate::index::IndexStore;
use crate::whisper::normalize_keywords;
use std::path::Path;

pub fn list_stt_dictionary_keywords(repo_root: &Path) -> Result<Vec<String>, String> {
    IndexStore::new(repo_root).list_stt_dictionary_keywords()
}

pub fn add_stt_dictionary_keyword(
    repo_root: &Path,
    keyword: String,
) -> Result<Vec<String>, String> {
    let keyword = normalize_single_keyword(keyword)?;
    let store = IndexStore::new(repo_root);
    store.upsert_stt_dictionary_keyword(&keyword)?;
    store.list_stt_dictionary_keywords()
}

pub fn delete_stt_dictionary_keyword(repo_root: &Path, keyword: String) -> Result<bool, String> {
    let keyword = normalize_single_keyword(keyword)?;
    IndexStore::new(repo_root).delete_stt_dictionary_keyword(&keyword)
}

pub(crate) fn resolve_stt_keywords(
    index_store: &IndexStore,
    requested_keywords: &[String],
) -> Result<Vec<String>, String> {
    let mut merged = index_store.list_stt_dictionary_keywords()?;
    merged.extend(requested_keywords.iter().cloned());
    Ok(normalize_keywords(&merged))
}

fn normalize_single_keyword(keyword: String) -> Result<String, String> {
    normalize_keywords(&[keyword])
        .into_iter()
        .next()
        .ok_or_else(|| "keyword cannot be empty".to_string())
}
