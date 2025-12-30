//! RecordRoute HTTP/WebSocket Server
//!
//! Actix-web 기반 REST API 및 WebSocket 서버
//! Phase 5에서 구현 예정

use recordroute_common::Result;

/// 서버 설정 (추후 구현)
pub struct ServerConfig;

impl ServerConfig {
    /// 새 서버 설정 생성 (스텁)
    pub fn new() -> Result<Self> {
        Ok(Self)
    }
}

impl Default for ServerConfig {
    fn default() -> Self {
        Self
    }
}
