# Reox APT Repository

This directory contains the APT repository infrastructure for distributing Reox on Debian/Ubuntu systems.

## Structure

```
apt-repo/
├── dists/stable/main/binary-amd64/   # Package metadata
├── pool/main/                         # .deb packages
├── scripts/                           # Build scripts
├── reox.gpg                          # Public GPG key
└── reox.list                         # APT sources list
```

## User Installation

```bash
# Add repository
curl -fsSL https://apt.reox.dev/reox.gpg | sudo gpg --dearmor -o /usr/share/keyrings/reox.gpg
echo "deb [signed-by=/usr/share/keyrings/reox.gpg] https://apt.reox.dev stable main" | sudo tee /etc/apt/sources.list.d/reox.list

# Install
sudo apt update
sudo apt install reoxc
```

## Maintenance

Run `scripts/update-repo.sh` after adding new .deb packages.
