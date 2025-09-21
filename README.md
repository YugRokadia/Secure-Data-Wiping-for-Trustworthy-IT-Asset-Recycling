# 🔥 LUKS Crypto Wipe Tool

**Secure cryptographic data destruction tool using LUKS encryption.**

## 📋 Features
- Interactive device/partition selection
- LUKS2 AES-XTS-256 encryption
- Cryptographic key destruction
- Military-grade data destruction
- SystemRescue OS compatible

## 🛠️ Build Instructions

### Prerequisites
```bash
# Install Rust
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
source ~/.cargo/env

# Install dependencies (Ubuntu/Debian)
sudo apt update
sudo apt install cryptsetup build-essential
```

### Build the Tool
```bash
# Clone/extract the project
cd wipeshit/

# Build release version
cargo build --release

# Executable will be at: target/release/wipeshit
```

## 🚀 Usage

### Interactive Mode
```bash
sudo ./target/release/wipeshit
```

### Direct Mode
```bash
sudo ./target/release/wipeshit /dev/sdX
```

### With Options
```bash
sudo ./target/release/wipeshit /dev/sdX --force --verify
```

## 💽 SystemRescue USB Deployment

### Add to Existing SystemRescue USB
```bash
# Copy tool to USB
cp target/release/wipeshit /media/user/RESCUE1202/

# Boot SystemRescue, then run:
mount /dev/sdb1 /mnt && cp /mnt/wipeshit /tmp/ && chmod +x /tmp/wipeshit && /tmp/wipeshit
```

## ⚠️ WARNING
This tool will PERMANENTLY DESTROY all data on selected devices. Use with extreme caution!

## 🔒 Security
- LUKS2 encryption with AES-XTS-256
- 512-bit encryption keys
- SHA-256 hashing
- Cryptographic key destruction
- Forensically unrecoverable results

## 📞 Support
Built for SIH 2025 - Secure Data Destruction Project
