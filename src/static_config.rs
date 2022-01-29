use serde::{Serialize, Deserialize};

#[derive(Debug, Clone, Eq, PartialEq)]
#[derive(Serialize, Deserialize)]
pub struct StaticConfig {
    pub serve_address: String,
    pub serve_port: u32,
    pub tls: bool,
    pub cert_path: String,
    pub key_path: String,
    pub ms_between_rounds: u64,
}

pub fn read_static_config() -> StaticConfig {
    let config_file = std::fs::File::open("pokerrs.config").unwrap();
    serde_json::from_reader(&config_file).unwrap()
}
