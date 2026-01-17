#!/bin/bash
set -e # Exit on any error

# Colors for pretty output
GREEN='\033[0;32m'
RED='\033[0;31m'
NC='\033[0m' # No Color

# 1. Privilege Check
if [ "$EUID" -ne 0 ]; then
  echo -e "${RED}Please run as root (use sudo).${NC}"
  exit 1
fi

# Get the original user (who called sudo)
REAL_USER=${SUDO_USER:-$USER}
USER_HOME=$(getent passwd $REAL_USER | cut -d: -f6)

echo -e "${GREEN}=== Nitro Installer ===${NC}"
echo "Installing for user: $REAL_USER"

# 2. Dependency Check
echo -e "\n${GREEN}[1/6] Checking dependencies...${NC}"
if ! command -v ryzenadj &> /dev/null; then
    echo -e "${RED}Error: ryzenadj is not installed.${NC}"
    echo "Please run: yay -S ryzenadj-git"
    exit 1
fi
echo "Dependencies OK."

# 3. Build (Fixed to find Cargo)
echo -e "\n${GREEN}[2/6] Building project (Release Mode)...${NC}"

# Find the user's cargo binary explicitly
CARGO_PATH="/home/$REAL_USER/.cargo/bin/cargo"

if [ ! -f "$CARGO_PATH" ]; then
    echo -e "${RED}Error: Could not find cargo at $CARGO_PATH${NC}"
    echo "Please ensure Rust is installed for user '$REAL_USER'"
    exit 1
fi

echo "Using cargo at: $CARGO_PATH"
# Run build as the real user using the absolute path
sudo -u $REAL_USER $CARGO_PATH build --release

# 4. Install Binaries
echo -e "\n${GREEN}[3/6] Installing binaries...${NC}"
cp target/release/nitro-daemon /usr/local/bin/
cp target/release/nitro-gui /usr/local/bin/
chmod +x /usr/local/bin/nitro-daemon
chmod +x /usr/local/bin/nitro-gui
echo "Binaries installed to /usr/local/bin/"

# 5. Config Setup
echo -e "\n${GREEN}[4/6] Creating configuration...${NC}"
mkdir -p /etc/nitro
CONFIG_FILE="/etc/nitro/config.toml"

if [ ! -f "$CONFIG_FILE" ]; then
    cat > "$CONFIG_FILE" <<EOF
# Nitro Power Configuration
# Limits are in mW (milliwatts). 1000 = 1 Watt.

[monk]
slow_limit = 5000    # Slow Average (5W)
fast_limit = 8000    # Burst Load (8W)
stapm_limit = 5000   # Sustained Load (5W)

[eco]
slow_limit = 8000    # Slow Average (8W)
fast_limit = 15000   # Burst Load (15W)
stapm_limit = 8000   # Sustained Load (8W)

[pro]
slow_limit = 50000   # Slow Average (50W)
fast_limit = 50000   # Burst Load (50W)
stapm_limit = 50000  # Sustained Load (50W)

EOF
    echo "Created default config at $CONFIG_FILE"
else
    echo "Config file already exists. Skipping overwrite."
fi

# 6. Systemd Service
echo -e "\n${GREEN}[5/6] Setting up Systemd Service...${NC}"
SERVICE_FILE="/etc/systemd/system/nitro-daemon.service"

cat > "$SERVICE_FILE" <<EOF
[Unit]
Description=Nitro Battery Saver Daemon
After=network.target

[Service]
ExecStart=/usr/local/bin/nitro-daemon
Restart=on-failure
RestartSec=5s
User=root
Group=root
Environment=RUST_LOG=info
Environment=RUST_BACKTRACE=1

[Install]
WantedBy=multi-user.target
EOF

systemctl daemon-reload
systemctl enable nitro-daemon
systemctl restart nitro-daemon
echo "Service enabled and started."

# 7. Desktop Entry
echo -e "\n${GREEN}[6/6] Creating App Launcher shortcut...${NC}"
DESKTOP_FILE="$USER_HOME/.local/share/applications/nitro.desktop"
mkdir -p "$USER_HOME/.local/share/applications"

cat > "$DESKTOP_FILE" <<EOF
[Desktop Entry]
Type=Application
Name=Nitro 
Comment=Power Control Manager
Exec=/usr/bin/foot -e /usr/local/bin/nitro-gui
Icon=battery
Terminal=false
Categories=System;Monitor;Utility;
EOF

chown $REAL_USER:$REAL_USER "$DESKTOP_FILE"
echo "Desktop entry created."

echo -e "\n${GREEN}=== Installation Complete! ===${NC}"
echo "1. The Daemon is running in the background."
echo "2. You can launch the dashboard by typing 'nitro-gui' or finding 'Nitro' in your app menu."
echo "3. Edit /etc/nitro/config.toml to tweak power limits."