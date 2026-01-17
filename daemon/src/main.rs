use anyhow::Result;
mod actuator;
mod config;
use config::NitroConfig;
use nitro_core::{DaemonCommand, PowerState, Profile};
use regex::Regex;
use std::fs;
use std::process::Command;
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::UnixListener;
use tokio::sync::watch;
use tokio::time;

async fn run_loop(
    tx: watch::Sender<PowerState>,
    shared_profile: Arc<Mutex<Profile>>,
    config: NitroConfig,
) -> Result<()> {
    let mut interval = time::interval(Duration::from_secs(2));
    let mut actuator = actuator::Actuator::new(config);

    loop {
        interval.tick().await;

        let battery_watts = read_watts();
        let cpu_watts = read_cpu_watts();
        let cpu_load = read_cpu_load();
        let battery_percent = read_battery_percent();
        let is_plugged_in = read_is_plugged_in();

        // Read the actual target profile from shared state
        let current_profile = {
            let lock = shared_profile.lock().unwrap();
            lock.clone()
        };

        // Apply Hardware Limits
        actuator.apply_profile(&current_profile, is_plugged_in);

        let state = PowerState {
            battery_watts,
            cpu_watts,
            battery_percent,
            cpu_load,
            profile: current_profile,
            wifi_on: true,      // Placeholder
            bluetooth_on: true, // Placeholder
            is_plugged_in,
        };

        // log::info!("{:?}", state); // Optional: keep logging or remove it
        let _ = tx.send(state);
    }
}

async fn start_ipc_server(
    rx: watch::Receiver<PowerState>,
    shared_profile: Arc<Mutex<Profile>>,
) -> Result<()> {
    let socket_path = "/tmp/nitro.sock";
    if fs::metadata(socket_path).is_ok() {
        fs::remove_file(socket_path)?;
    }

    let listener = UnixListener::bind(socket_path)?;

    use std::os::unix::fs::PermissionsExt;
    let mut perms = std::fs::metadata(socket_path).unwrap().permissions();
    perms.set_mode(0o777); // Allow everyone to read/write
    std::fs::set_permissions(socket_path, perms).unwrap();

    log::info!("IPC Server listening on {}", socket_path);

    loop {
        let (socket, _) = listener.accept().await?;
        let mut rx = rx.clone();
        let shared_profile = shared_profile.clone();

        tokio::spawn(async move {
            let (reader, mut writer) = socket.into_split();

            // Task 1: Writer (Send PowerState)
            let writer_task = tokio::spawn(async move {
                // Send the current value immediately
                {
                    let state = rx.borrow().clone();
                    if let Ok(json) = serde_json::to_string(&state) {
                        if writer
                            .write_all(format!("{}\n", json).as_bytes())
                            .await
                            .is_err()
                        {
                            return;
                        }
                    }
                }

                // Watch for changes
                while rx.changed().await.is_ok() {
                    let state = rx.borrow().clone();
                    if let Ok(json) = serde_json::to_string(&state) {
                        if writer
                            .write_all(format!("{}\n", json).as_bytes())
                            .await
                            .is_err()
                        {
                            break;
                        }
                    }
                }
            });

            // Task 2: Reader (Receive DaemonCommand)
            let reader_task = tokio::spawn(async move {
                let mut lines = BufReader::new(reader).lines();
                while let Ok(Some(line)) = lines.next_line().await {
                    if let Ok(cmd) = serde_json::from_str::<DaemonCommand>(&line) {
                        log::info!("Received Command: {:?}", cmd);
                        match cmd {
                            DaemonCommand::SetProfile(p) => {
                                let mut lock = shared_profile.lock().unwrap();
                                *lock = p;
                            }
                            DaemonCommand::ToggleWifi => {
                                log::info!("Toggle Wifi (Not Implemented)");
                            }
                            DaemonCommand::ToggleBluetooth => {
                                log::info!("Toggle Bluetooth (Not Implemented)");
                            }
                        }
                    }
                }
            });

            // Wait for either to finish (likely connection closed)
            let _ = tokio::select! {
                _ = writer_task => {},
                _ = reader_task => {},
            };
        });
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    env_logger::init();
    log::info!("Starting Nitro Daemon...");

    // Load Config
    let config = NitroConfig::load().unwrap_or_else(|e| {
        log::warn!("Failed to load config, using defaults: {}", e);
        // We need a way to get default config if load fails, but load() returns Result.
        // For now, let's just panic or handle it better.
        // Actually, load() handles defaults if file missing, but if file exists and is bad, it errors.
        // Let's just assume defaults if it fails for now, or re-call load with no file?
        // Simpler: Just panic if config is bad, or use a default struct.
        // Since we don't have a Default impl for NitroConfig easily accessible here without deriving it,
        // let's just unwrap for now as per instructions "If not found, returns a Default configuration".
        // My load() implementation does that. So unwrap is fine if it returns defaults on missing file.
        // If it errors on bad file, we probably want to crash to alert user.
        NitroConfig::load().expect("Failed to load configuration")
    });

    // Graceful Exit Handler
    let config_clone = config.clone();
    ctrlc::set_handler(move || {
        log::info!("Exiting... Resetting to Pro Mode.");
        let mut actuator = actuator::Actuator::new(config_clone.clone());
        actuator.apply_profile(&Profile::Pro, false); // Force apply Pro mode (unplugged logic to ensure it runs)
        std::process::exit(0);
    })?;

    // Shared State
    let shared_profile = Arc::new(Mutex::new(Profile::Eco));

    // Initial state
    let initial_state = PowerState {
        battery_watts: 0.0,
        cpu_watts: 0.0,
        battery_percent: 0,
        cpu_load: 0.0,
        profile: Profile::Eco,
        wifi_on: false,
        bluetooth_on: false,
        is_plugged_in: false,
    };

    let (tx, rx) = watch::channel(initial_state);

    // Spawn IPC Server
    let profile_for_server = shared_profile.clone();
    tokio::spawn(async move {
        if let Err(e) = start_ipc_server(rx, profile_for_server).await {
            log::error!("IPC Server Error: {}", e);
        }
    });

    // Run Sensor Loop
    run_loop(tx, shared_profile, config).await
}

fn read_watts() -> f32 {
    let bat1_power = "/sys/class/power_supply/BAT1/power_now";
    let bat0_power = "/sys/class/power_supply/BAT0/power_now";

    if let Ok(watts) = read_power_file(bat1_power) {
        return watts;
    }

    if let Ok(watts) = read_power_file(bat0_power) {
        return watts;
    }

    // Fallback: voltage_now * current_now
    // Try BAT1 then BAT0 for these as well
    if let Some(watts) = calculate_watts_from_voltage_current("BAT1") {
        return watts;
    }
    if let Some(watts) = calculate_watts_from_voltage_current("BAT0") {
        return watts;
    }

    0.0
}

fn read_power_file(path: &str) -> Result<f32> {
    let content = fs::read_to_string(path)?;
    let micro_watts: f32 = content.trim().parse()?;
    Ok(micro_watts / 1_000_000.0)
}

fn calculate_watts_from_voltage_current(bat: &str) -> Option<f32> {
    let voltage_path = format!("/sys/class/power_supply/{}/voltage_now", bat);
    let current_path = format!("/sys/class/power_supply/{}/current_now", bat);

    let voltage_str = fs::read_to_string(voltage_path).ok()?;
    let current_str = fs::read_to_string(current_path).ok()?;

    let voltage: f32 = voltage_str.trim().parse().ok()?;
    let current: f32 = current_str.trim().parse().ok()?;

    // voltage (uV) * current (uA) = pW (picowatts)
    // pW / 10^12 = W
    Some((voltage * current) / 1_000_000_000_000.0)
}

fn read_cpu_load() -> f32 {
    match sys_info::loadavg() {
        Ok(load) => load.one as f32, // Using 1-minute load average as a proxy for "current" load
        Err(_) => 0.0,
    }
}

fn read_cpu_watts() -> f32 {
    // Run ryzenadj -i
    if let Ok(output) = Command::new("ryzenadj").arg("-i").output() {
        let stdout = String::from_utf8_lossy(&output.stdout);
        // Regex to find "PPT LIMIT FAST" or similar? No, user said "PPT VALUE FAST"
        // Actually usually it's "PPT VALUE FAST" or just "PPT VALUE".
        // Let's look for the table output.
        // Example output:
        // | PPT LIMIT FAST | 35.000 |
        // | PPT VALUE FAST | 12.345 |
        // We want the value.
        let re = Regex::new(r"PPT VALUE FAST\s*\|\s*([\d\.]+)").unwrap();
        if let Some(caps) = re.captures(&stdout) {
            if let Some(val_str) = caps.get(1) {
                if let Ok(val) = val_str.as_str().parse::<f32>() {
                    return val;
                }
            }
        }
    }
    0.0
}

// Helpers for other fields to make the struct more realistic
fn read_battery_percent() -> u8 {
    let paths = [
        "/sys/class/power_supply/BAT1/capacity",
        "/sys/class/power_supply/BAT0/capacity",
    ];
    for path in paths {
        if let Ok(content) = fs::read_to_string(path) {
            if let Ok(val) = content.trim().parse() {
                return val;
            }
        }
    }
    0
}

fn read_is_plugged_in() -> bool {
    let paths = [
        "/sys/class/power_supply/AC/online",
        "/sys/class/power_supply/ACAD/online",
        "/sys/class/power_supply/ADP0/online",
        "/sys/class/power_supply/ADP1/online",
    ];
    for path in paths {
        if let Ok(content) = fs::read_to_string(path) {
            if content.trim() == "1" {
                return true;
            }
        }
    }
    false
}
