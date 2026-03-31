use crate::index::{DictionaryKeywordSource, DictionaryKeywords, IndexStore};
use crate::whisper::normalize_keywords;
use std::path::Path;

pub fn list_stt_dictionary_keywords(repo_root: &Path) -> Result<DictionaryKeywords, String> {
    IndexStore::new(repo_root).list_stt_dictionary_keywords()
}

pub fn add_stt_dictionary_keyword(
    repo_root: &Path,
    keyword: String,
) -> Result<DictionaryKeywords, String> {
    let keyword = normalize_single_keyword(keyword)?;
    let store = IndexStore::new(repo_root);
    store.upsert_stt_dictionary_keyword(&keyword, DictionaryKeywordSource::User)?;
    store.list_stt_dictionary_keywords()
}

pub fn delete_stt_dictionary_keyword(repo_root: &Path, keyword: String) -> Result<bool, String> {
    let keyword = normalize_single_keyword(keyword)?;
    IndexStore::new(repo_root)
        .delete_stt_dictionary_keyword(&keyword, DictionaryKeywordSource::User)
}

pub fn promote_auto_stt_dictionary_keyword(
    repo_root: &Path,
    keyword: String,
) -> Result<bool, String> {
    let keyword = normalize_single_keyword(keyword)?;
    IndexStore::new(repo_root).promote_stt_dictionary_keyword(&keyword)
}

pub fn delete_auto_stt_dictionary_keyword(
    repo_root: &Path,
    keyword: String,
) -> Result<bool, String> {
    let keyword = normalize_single_keyword(keyword)?;
    IndexStore::new(repo_root)
        .delete_stt_dictionary_keyword(&keyword, DictionaryKeywordSource::Auto)
}

pub fn delete_auto_stt_dictionary_keywords(repo_root: &Path) -> Result<DictionaryKeywords, String> {
    let store = IndexStore::new(repo_root);
    store.delete_stt_dictionary_keywords_by_source(DictionaryKeywordSource::Auto)?;
    store.list_stt_dictionary_keywords()
}

pub(crate) fn resolve_stt_keywords(
    index_store: &IndexStore,
    requested_keywords: &[String],
) -> Result<Vec<String>, String> {
    let mut merged = index_store.list_stt_dictionary_keywords()?.user_keywords;
    merged.extend(requested_keywords.iter().cloned());
    Ok(normalize_keywords(&merged))
}

fn normalize_single_keyword(keyword: String) -> Result<String, String> {
    normalize_keywords(&[keyword])
        .into_iter()
        .next()
        .ok_or_else(|| "keyword cannot be empty".to_string())
}
