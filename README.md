# Nitro
> **A Kernel-level Battery Saver & TUI Dashboard for Linux Gaming Laptops.**

![Rust](https://img.shields.io/badge/Rust-1.75%2B-orange?logo=rust)
![Linux](https://img.shields.io/badge/Platform-Linux-blue?logo=linux)
![License](https://img.shields.io/badge/License-MIT-green)

**Nitro** is a distributed system written in Rust designed to extend battery life on Ryzen-based laptops (specifically the Acer Nitro 5, but applicable to others). It bypasses BIOS power limits to enforce strict TDP constraints using `ryzenadj`.

## Distributed Architecture

Nitro uses a split-process architecture to separate privileged hardware control from the user interface.

```ascii
+-----------------+       +-----------------+       +-----------------+
|                 |       |                 |       |                 |
|   nitro-daemon  | <---> | Unix Domain Sock| <---> |    nitro-gui    |
|   (Root/System) |       | /tmp/nitro.sock |       |   (User/TUI)    |
|                 |       |                 |       |                 |
+-----------------+       +-----------------+       +-----------------+
        |
        v
  [Hardware/RyzenAdj]
```

- **nitro-daemon**: Runs as root. Enforces hardware limits, monitors sensors, and fights back against BIOS power resets. Reads configuration from `/etc/nitro/config.toml`.
- **nitro-gui**: Runs as user. A Ratatui-based TUI for visualization and control.
- **IPC**: Uses Unix Domain Sockets for low-latency, bi-directional communication.

## Key Features

### Aggressive Power Management
- **Monk Mode**: Strict power saving. Default: 5W (Sustained) / 8W (Burst).
- **Eco Mode**: Balanced performance. Default: 8W / 15W.
- **Pro Mode**: High performance. Default: 25W+.

### Bios Fight-Back
The daemon aggressively reapplies power limits every 2 seconds (and immediately upon unplugging) to override BIOS watchdogs that attempt to reset TDP to default high values.

### Real-Time Dashboard
Visualizes:
- Power Draw: Displays both Total System Power and CPU Power separately.
- CPU Load
- Active Profile
- Battery Percentage (with color coding)

### Configuration
Fully configurable via `/etc/nitro/config.toml`. You can tweak the TDP limits for each profile to match your specific hardware capabilities.

## Requirements

- **OS**: Arch Linux (Recommended)
- **Hardware**: AMD Ryzen CPU (Rembrandt/Cezanne or similar)
- **Dependencies**:
  - `ryzenadj-git` (for TDP control)
  - `ryzen_smu-dkms-git` (Kernel Module for ryzenadj)
  - `cpupower` (optional, for frequency scaling)

## Installation Guide

You can use the provided `install.sh` script for an automated installation, or follow the manual steps below.

### Automated Installation
```bash
sudo ./install.sh
```

### Manual Installation

#### 1. Build
```bash
cargo build --release
```

#### 2. Install Binaries
```bash
sudo cp target/release/nitro-daemon /usr/local/bin/
sudo cp target/release/nitro-gui /usr/local/bin/
```

#### 3. Create Configuration
Create the directory and config file:
```bash
sudo mkdir -p /etc/nitro
sudo nano /etc/nitro/config.toml
```

Example configuration:
```toml
[monk]
stapm_limit = 5000
fast_limit = 8000
slow_limit = 5000

[eco]
stapm_limit = 8000
fast_limit = 15000
slow_limit = 8000
tctl_temp = 85

[pro]
stapm_limit = 25000
fast_limit = 35000
slow_limit = 25000
```

#### 4. System Service
Create a systemd service file at `/etc/systemd/system/nitro-daemon.service`:

```ini
[Unit]
Description=Nitro Battery Saver Daemon
After=network.target

[Service]
ExecStart=/usr/local/bin/nitro-daemon
Restart=always
User=root
Group=root
Environment=RUST_LOG=info

[Install]
WantedBy=multi-user.target
```

Enable and start the service:
```bash
sudo systemctl enable --now nitro-daemon
```

#### 5. Desktop Entry
Create a desktop entry for the GUI at `~/.local/share/applications/nitro.desktop`:

```ini
[Desktop Entry]
Type=Application
Name=Nitro Battery
Exec=/usr/local/bin/nitro-gui
Terminal=true
Categories=System;Monitor;
```

## Usage

Launch `nitro-gui` from your terminal or search for "Nitro Battery" in your application launcher.

### Keybindings

| Key | Action | Description |
| :---: | :--- | :--- |
| **m** | **Monk Mode** | Switch to Monk profile |
| **e** | **Eco Mode** | Switch to Eco profile |
| **p** | **Pro Mode** | Switch to Pro profile |
| **q** | **Quit** | Exit the GUI |

## Disclaimer

**USE AT YOUR OWN RISK.**
This software manipulates hardware power limits using `ryzenadj`. While generally safe on modern hardware with thermal protections, the authors are not responsible for any hardware damage, data loss, or instability caused by using this software.
