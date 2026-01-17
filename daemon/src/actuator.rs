use crate::config::NitroConfig;
use nitro_core::Profile;
use std::process::Command;
use std::thread;
use std::time::Duration;

pub struct Actuator {
    last_profile: Option<Profile>,
    last_plugged_in: Option<bool>,
    config: NitroConfig,
}

impl Actuator {
    pub fn new(config: NitroConfig) -> Self {
        Self {
            last_profile: None,
            last_plugged_in: None,
            config,
        }
    }

    pub fn apply_profile(&mut self, profile: &Profile, is_plugged_in: bool) {
        // If plugged in, IGNORE the dashboard profile and FORCE "Pro" limits.
        // This ensures that plugging in always uncaps the performance,
        // even if the dashboard was left on "Monk".
        let target_profile = if is_plugged_in {
            &Profile::Pro
        } else {
            profile
        };

        // Log the action
        log::info!("Enforcing limits for {:?}", target_profile);

        // 1. Apply the limits IMMEDIATELY (Every single loop)
        // This is what fights the BIOS watchdog.
        self.apply_ryzen_limits(target_profile);

        // 2. Double-Tap on Unplug:
        // If we just unplugged (AC -> Battery), wait a tiny bit and apply AGAIN.
        // This ensures the transition sticks if the hardware was busy switching states.
        if !is_plugged_in && self.last_plugged_in == Some(true) {
            thread::sleep(Duration::from_millis(100));
            self.apply_ryzen_limits(target_profile);
        }

        // Update state tracking
        self.last_profile = Some(profile.clone());
        self.last_plugged_in = Some(is_plugged_in);
    }

    fn apply_ryzen_limits(&self, profile: &Profile) {
        let mut args = Vec::new();

        match profile {
            Profile::Monk => {
                args.push(format!("--slow-limit={}", self.config.monk.slow_limit));
                args.push(format!("--fast-limit={}", self.config.monk.fast_limit));
                args.push(format!("--stapm-limit={}", self.config.monk.stapm_limit));
            }
            Profile::Eco => {
                args.push(format!("--slow-limit={}", self.config.eco.slow_limit));
                args.push(format!("--fast-limit={}", self.config.eco.fast_limit));
                args.push(format!("--stapm-limit={}", self.config.eco.stapm_limit));
                if let Some(temp) = self.config.eco.tctl_temp {
                    args.push(format!("--tctl-temp={}", temp));
                }
            }
            Profile::Pro => {
                args.push(format!("--slow-limit={}", self.config.pro.slow_limit));
                args.push(format!("--fast-limit={}", self.config.pro.fast_limit));
                args.push(format!("--stapm-limit={}", self.config.pro.stapm_limit));
            }
        };

        // Log what we are doing
        log::info!("Applying Ryzen Limits: {:?}", args);

        match Command::new("ryzenadj").args(&args).output() {
            Ok(output) => {
                if !output.status.success() {
                    log::error!(
                        "ryzenadj failed: {}",
                        String::from_utf8_lossy(&output.stderr)
                    );
                }
            }
            Err(e) => {
                log::error!("Failed to execute ryzenadj: {}", e);
            }
        }
    }
}
