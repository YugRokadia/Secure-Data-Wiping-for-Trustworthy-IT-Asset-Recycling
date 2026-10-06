<div align="center">

<img src="https://capsule-render.vercel.app/api?type=waving&color=0:1e2327,100:4a148c&height=180&section=header&text=Secure%20Data%20Wipe&fontSize=46&fontColor=ffffff&animation=fadeIn&fontAlignY=38&desc=Cryptographic,%20Irreversible%20Disk%20Sanitization&descAlignY=58&descSize=16" width="100%"/>

<img src="https://readme-typing-svg.demolab.com/?font=Fira+Code&weight=600&size=22&duration=3000&pause=800&color=A78BFA&center=true&vCenter=true&width=700&lines=LUKS2+%2B+AES-XTS-256+Cryptographic+Overwrite;Bootable+ISO+%E2%80%A2+No+OS+Required;Forensically+Unrecoverable+Results;Built+for+Trustworthy+IT+Asset+Recycling" alt="Typing SVG" />

<br/>

![Rust](https://img.shields.io/badge/Rust-000000?style=for-the-badge&logo=rust&logoColor=white)
![LUKS2](https://img.shields.io/badge/LUKS2-AES--XTS--256-4a148c?style=for-the-badge&logo=keybase&logoColor=white)
![License](https://img.shields.io/badge/License-MIT-00c853?style=for-the-badge)
![Status](https://img.shields.io/badge/Status-Active-blue?style=for-the-badge)

<img src="https://img.shields.io/github/stars/YugRokadia/Secure-Data-Wiping-for-Trustworthy-IT-Asset-Recycling?style=social" />
<img src="https://img.shields.io/github/forks/YugRokadia/Secure-Data-Wiping-for-Trustworthy-IT-Asset-Recycling?style=social" />

</div>

<br/>

> **Permanently and irreversibly destroys data on any storage device using military-grade cryptographic overwrite — no installed OS required.**

A standalone, bootable tool for secure IT asset recycling and data retention compliance. Instead of slow multi-pass physical overwriting, it encrypts the entire device with a throwaway LUKS2 key and then destroys the key — rendering every bit on the disk forensically unrecoverable in seconds.

<br/>

<div align="center">
<img src="https://capsule-render.vercel.app/api?type=rect&color=0:2d1b4e,100:1e2327&height=2&width=800"/>
</div>

## ⚡ How It Works

<div align="center">

```mermaid
flowchart LR
    A[💽 Target Device] --> B[🔐 LUKS2 Format<br/>AES-XTS-256]
    B --> C[🗝️ Random 512-bit Key]
    C --> D[🧨 Key Destruction]
    D --> E[☠️ Data Forensically<br/>Unrecoverable]

    style A fill:#2d1b4e,stroke:#A78BFA,color:#fff
    style B fill:#2d1b4e,stroke:#A78BFA,color:#fff
    style C fill:#2d1b4e,stroke:#A78BFA,color:#fff
    style D fill:#4a148c,stroke:#ff5252,color:#fff
    style E fill:#1e2327,stroke:#00c853,color:#fff
```

</div>

Once the encryption key is destroyed, the ciphertext on disk is mathematically indistinguishable from random noise — there is no key left anywhere to ever decrypt it again.

<br/>

## ✨ Features

<table>
<tr>
<td width="50%">

**🔐 Cryptographic Destruction**
LUKS2 + AES-XTS-256 full-disk encryption, followed by irreversible key destruction — no multi-pass overwrite needed.

**💽 Interactive Device Selection**
Guided prompts for picking the exact device/partition, reducing the risk of wiping the wrong drive.

</td>
<td width="50%">

**🧰 SystemRescue Compatible**
Drop the binary onto any SystemRescue USB and run it directly from a bootable environment, no host OS required.

**🪖 Military-Grade Standard**
512-bit key material, SHA-256 hashing, and forensically unrecoverable output.

</td>
</tr>
</table>

<br/>

<div align="center">
<img src="https://capsule-render.vercel.app/api?type=rect&color=0:2d1b4e,100:1e2327&height=2&width=800"/>
</div>

## 🛠️ Build

```bash
# Install Rust
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
source ~/.cargo/env

# Install dependencies (Ubuntu/Debian)
sudo apt update
sudo apt install cryptsetup build-essential

# Clone and build
git clone https://github.com/YugRokadia/Secure-Data-Wiping-for-Trustworthy-IT-Asset-Recycling.git
cd Secure-Data-Wiping-for-Trustworthy-IT-Asset-Recycling
cargo build --release

# Binary will be at: target/release/secure-wipe
```

## 🚀 Usage

```bash
# Interactive mode — guided device selection
sudo ./target/release/secure-wipe

# Direct mode — target a specific device
sudo ./target/release/secure-wipe /dev/sdX

# With options
sudo ./target/release/secure-wipe /dev/sdX --force --verify
```

## 💾 SystemRescue USB Deployment

```bash
# Copy the tool onto an existing SystemRescue USB
cp target/release/secure-wipe /media/user/RESCUE1202/

# Boot SystemRescue, then:
mount /dev/sdb1 /mnt && cp /mnt/secure-wipe /tmp/ && chmod +x /tmp/secure-wipe && /tmp/secure-wipe
```

<br/>

<div align="center">

## ⚠️ Warning

</div>

> **This tool permanently and irreversibly destroys all data on the selected device.** There is no undo. Always double-check the target device path before confirming.

<br/>

## 🔒 Security Details

| Component | Spec |
|---|---|
| Encryption | LUKS2, AES-XTS-256 |
| Key size | 512-bit |
| Hashing | SHA-256 |
| Key handling | Generated, used once, then destroyed |
| Recoverability | Forensically unrecoverable post key-destruction |

<br/>

<div align="center">

Built for **Smart India Hackathon** — Secure Data Destruction for Trustworthy IT Asset Recycling

<img src="https://capsule-render.vercel.app/api?type=waving&color=0:4a148c,100:1e2327&height=100&section=footer" width="100%"/>

</div>
