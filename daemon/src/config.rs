use anyhow::Result;
use config::{Config, File};
use serde::Deserialize;

#[derive(Debug, Deserialize, Clone)]
pub struct ProfileConfig {
    pub stapm_limit: u32,
    pub fast_limit: u32,
    pub slow_limit: u32,
    pub tctl_temp: Option<u32>,
}

#[derive(Debug, Deserialize, Clone)]
pub struct NitroConfig {
    pub monk: ProfileConfig,
    pub eco: ProfileConfig,
    pub pro: ProfileConfig,
}

impl NitroConfig {
    pub fn load() -> Result<Self> {
        let config_path = "/etc/nitro/config.toml";

        let builder = Config::builder()
            .set_default("monk.stapm_limit", 5000)?
            .set_default("monk.fast_limit", 8000)?
            .set_default("monk.slow_limit", 5000)?
            .set_default("eco.stapm_limit", 8000)?
            .set_default("eco.fast_limit", 15000)?
            .set_default("eco.slow_limit", 8000)?
            .set_default("eco.tctl_temp", 85)?
            .set_default("pro.stapm_limit", 25000)?
            .set_default("pro.fast_limit", 35000)?
            .set_default("pro.slow_limit", 25000)?
            .add_source(File::with_name(config_path).required(false));

        let config = builder.build()?;
        Ok(config.try_deserialize()?)
    }
}
