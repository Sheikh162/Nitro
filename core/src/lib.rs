use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum Profile {
    Monk, // Strict power saving
    Eco,  // Balanced
    Pro,  // Performance
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PowerState {
    pub battery_watts: f32,
    pub battery_percent: u8,
    pub cpu_load: f32,
    pub profile: Profile,
    pub wifi_on: bool,
    pub bluetooth_on: bool,
    pub is_plugged_in: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum DaemonCommand {
    SetProfile(Profile),
    ToggleWifi,
    ToggleBluetooth,
}
