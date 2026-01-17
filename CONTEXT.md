# PROJECT SPECIFICATION: Nitro-Sip

## 1. Project Goal
Build a Linux System Daemon in Rust to optimize battery life on a Gaming Laptop (Acer Nitro 5) by enforcing strict power profiles ("Race-to-Idle" strategy) and a TUI Dashboard to visualize/control it.

## 2. Tech Stack & Architecture
- **Language**: Rust (Edition 2021)
- **Architecture**: Cargo Workspace with 3 members:
  1. `core`: Shared library (Data structures)
  2. `daemon`: Root-level background service (Tokio Runtime)
  3. `gui`: User-level TUI dashboard (Ratatui + Crossterm)
- **Target OS**: Arch Linux (Zen Kernel)

## 3. Workspace Structure
```
nitro_sip/
├── Cargo.toml (Workspace Root)
├── core/
│   ├── Cargo.toml
│   └── src/lib.rs (Shared types: PowerState, Profile)
├── daemon/
│   ├── Cargo.toml
│   └── src/main.rs (Logic loop, Hardware control)
└── gui/
    ├── Cargo.toml
    └── src/main.rs (Ratatui Dashboard, IPC Client)
```

## 4. Hardware Constraints (The Truth Table)
- **Battery Floor**: 5W (Target), 8W (Max Sustained)
- **Burst Limit**: 15W (Allowed for <5s)
- **GPU**: Nvidia must be in D3Cold (Off)
- **Screen**: 60Hz on battery, 144Hz on AC
- **CPU**: Disable Turbo Boost on battery

## 5. Implementation Details

### A. Shared Core (`core/lib.rs`)
Define a struct `PowerState` that both Daemon and GUI use:
- `battery_watts`: f32
- `battery_percent`: u8
- `cpu_load`: f32
- `profile`: Profile Enum (Monk, Eco, Pro)
- `wifi_on`: bool
- `bluetooth_on`: bool
- `is_plugged_in`: bool

### B. The Daemon (`daemon/src/main.rs`)
- **Loop**: Run a tokio loop every 2 seconds.
- **Sensor**: Read `/sys/class/power_supply/BAT1/power_now` (or `current_now * voltage_now` if missing).
- **Logic (The Governor)**:
  - **IF unplugged**:
    - Enforce 60Hz: `niri msg output eDP-1 mode 1920x1080@60.000`
    - Kill Nvidia: Ensure supergfxctl is "Integrated" or "Hybrid" (sleeping).
    - Pause Docker: `docker pause $(docker ps -q)` if CPU load < 5% for 2 mins.
    - Ryzen TDP: `ryzenadj --stapm-limit=8000 --fast-limit=15000`
- **IPC**: Expose a simple Unix Socket or HTTP server (localhost:9898) to send `JSON(PowerState)` to the GUI.

### C. The GUI (`gui/src/main.rs`)
- **Lib**: ratatui
- **Layout**:
  - Top: Gauge showing Battery %. Green > 50, Yellow > 20, Red < 20.
  - Middle: Big ASCII Text "8.5 W" (Current Draw).
  - Bottom: List of Toggles (Wifi, Bluetooth, Profile).
- **Input**: Press 'm' for Monk Mode, 'e' for Eco Mode. Sends command to Daemon.

## 6. Critical Rules
1. **Safety**: Never block the main thread. Use `tokio::spawn` for running shell commands.
2. **Error Handling**: If reading `/sys/...` fails (e.g., file not found), log error but do NOT crash the daemon. Return 0.0 watts.
3. **Paths**:
   - Battery: Try BAT1 first, fallback to BAT0.
   - Backlight: `/sys/class/backlight/amdgpu_bl0/brightness`.
