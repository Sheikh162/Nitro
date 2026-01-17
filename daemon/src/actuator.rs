use nitro_core::Profile;
use std::process::Command;
use std::thread;
use std::time::Duration;

pub struct Actuator {
    last_profile: Option<Profile>,
    last_plugged_in: Option<bool>,
}

impl Actuator {
    pub fn new() -> Self {
        Self {
            last_profile: None,
            last_plugged_in: None,
        }
    }

    pub fn apply_profile(&mut self, profile: &Profile, is_plugged_in: bool) {
        // Safety First: If plugged in, do nothing (return early).
        // We never restrict performance on AC power.
        if is_plugged_in {
            // Reset state tracking so when we unplug, we re-apply everything
            self.last_plugged_in = Some(true);
            return;
        }

        // Check if state changed to avoid spamming commands
        // let profile_changed = self.last_profile.as_ref() != Some(profile);
        // let power_source_changed = self.last_plugged_in != Some(is_plugged_in);

        // if !profile_changed && !power_source_changed {
        //     return;
        // }

        // Apply Ryzen Limits
        // Force Apply: if unplugged, run twice
        println!("Enforcing limits...");
        self.apply_ryzen_limits(profile);
        if !is_plugged_in {
            thread::sleep(Duration::from_millis(100));
            self.apply_ryzen_limits(profile);
        }

        // Update state
        self.last_profile = Some(profile.clone());
        self.last_plugged_in = Some(is_plugged_in);
    }

    fn apply_ryzen_limits(&self, profile: &Profile) {
        let mut args = Vec::new();

        match profile {
            Profile::Monk => {
                args.push("--stapm-limit=5000".to_string());
                args.push("--fast-limit=8000".to_string());
                args.push("--slow-limit=5000".to_string());
            }
            Profile::Eco => {
                args.push("--stapm-limit=8000".to_string());
                args.push("--fast-limit=15000".to_string());
                args.push("--slow-limit=8000".to_string());
                args.push("--tctl-temp=85".to_string());
            }
            Profile::Pro => {
                args.push("--stapm-limit=25000".to_string());
                args.push("--fast-limit=35000".to_string());
                args.push("--slow-limit=25000".to_string());
            }
        };

        // Log what we are doing
        println!("Applying Ryzen Limits: {:?}", args);

        match Command::new("ryzenadj").args(&args).output() {
            Ok(output) => {
                if !output.status.success() {
                    eprintln!(
                        "ryzenadj failed: {}",
                        String::from_utf8_lossy(&output.stderr)
                    );
                }
            }
            Err(e) => {
                eprintln!("Failed to execute ryzenadj: {}", e);
            }
        }
    }
}
