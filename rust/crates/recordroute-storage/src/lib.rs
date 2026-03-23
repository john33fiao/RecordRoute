use recordroute_core::config::AppConfig;

#[derive(Debug, Clone)]
pub struct StorageContext {
    pub config: AppConfig,
}

impl StorageContext {
    pub fn new(config: AppConfig) -> Self {
        Self { config }
    }
}
