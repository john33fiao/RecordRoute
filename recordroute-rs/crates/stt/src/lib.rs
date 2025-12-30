//! RecordRoute STT (Speech-to-Text) Engine
//!
//! Whisper.cpp 기반 음성 인식 모듈
//! Phase 2에서 구현 예정

use recordroute_common::Result;

/// STT 엔진 (추후 구현)
pub struct WhisperEngine;

impl WhisperEngine {
    /// 새 Whisper 엔진 생성 (스텁)
    pub fn new() -> Result<Self> {
        Ok(Self)
    }
}

impl Default for WhisperEngine {
    fn default() -> Self {
        Self
    }
}
