use serde::{Serialize, Deserialize};

#[derive(Debug, Clone, Eq, PartialEq)]
#[derive(Serialize, Deserialize)]
pub struct StaticConfig {
    pub tls: bool,
    pub cert_path: String,
    pub key_path: String,
}

pub fn read_static_config() -> StaticConfig {
    let config_file = std::fs::File::open("pokerrs.config").unwrap();
    serde_json::from_reader(&config_file).unwrap()
}
