#!/bin/bash
# auto-crypto-wipe-usb.sh - Creates auto-starting crypto wipe USB

set -e

echo "🔥 Creating Auto-Start Crypto Wipe USB"
echo "======================================="

# Build the tool
echo "📦 Building crypto wipe tool..."
cd wipeshit
cargo build --release
cd ..

# Check if SystemRescue ISO exists
if [ ! -f "systemrescue-10.02-amd64.iso" ]; then
    echo "📥 Downloading SystemRescue ISO..."
    wget https://www.system-rescue.org/releases/systemrescue-10.02-amd64.iso
fi

# Extract ISO
echo "📀 Extracting SystemRescue ISO..."
sudo mkdir -p /mnt/sysrescue
sudo mount -o loop systemrescue-10.02-amd64.iso /mnt/sysrescue
mkdir -p sysrescue-custom
sudo cp -r /mnt/sysrescue/* sysrescue-custom/
sudo umount /mnt/sysrescue

# Extract filesystem
echo "📂 Extracting root filesystem..."
cd sysrescue-custom/sysresccd/
sudo unsquashfs airootfs.sfs
sudo mv squashfs-root airootfs

# Add crypto wipe tool
echo "🛠️ Installing crypto wipe tool..."
sudo cp ../../wipeshit/target/release/wipeshit airootfs/usr/local/bin/
sudo chmod +x airootfs/usr/local/bin/wipeshit

# Create auto-start script
echo "🚀 Creating auto-start mechanism..."
sudo tee airootfs/usr/local/bin/crypto-wipe-autostart.sh << 'EOF'
#!/bin/bash
clear
echo "╔══════════════════════════════════════════════════════════════╗"
echo "║                🔥 CRYPTO WIPE AUTO-START 🔥                  ║"
echo "║              Bootable USB Data Destruction Tool             ║"
echo "╚══════════════════════════════════════════════════════════════╝"
echo ""
echo "🚀 Auto-launching crypto wipe tool in 5 seconds..."
echo "⚠️  Press Ctrl+C to cancel and get normal shell"
echo ""

for i in {5..1}; do
    echo -n "⏰ $i... "
    sleep 1
done
echo ""
echo ""

# Launch the tool
exec /usr/local/bin/wipeshit
EOF

sudo chmod +x airootfs/usr/local/bin/crypto-wipe-autostart.sh

# Modify .bashrc for auto-start
sudo tee -a airootfs/root/.bashrc << 'EOF'

# Auto-launch crypto wipe tool on first login
if [ "$PS1" ] && [ -z "$CRYPTO_WIPE_LAUNCHED" ] && [ "$(tty)" = "/dev/tty1" ]; then
    export CRYPTO_WIPE_LAUNCHED=1
    exec /usr/local/bin/crypto-wipe-autostart.sh
fi
EOF

# Repack filesystem
echo "📦 Repacking filesystem..."
sudo mksquashfs airootfs airootfs.sfs -comp xz
sudo rm -rf airootfs

# Create custom ISO
echo "💿 Creating custom ISO..."
cd ../..
sudo genisoimage -o systemrescue-cryptowipe-autostart.iso \
    -b isolinux/isolinux.bin \
    -c isolinux/boot.cat \
    -no-emul-boot \
    -boot-load-size 4 \
    -boot-info-table \
    -J -R -V "CryptoWipe-AutoStart" \
    sysrescue-custom/

echo ""
echo "✅ SUCCESS! Created: systemrescue-cryptowipe-autostart.iso"
echo ""
echo "📀 To create bootable USB:"
echo "   sudo dd if=systemrescue-cryptowipe-autostart.iso of=/dev/sdX bs=4M status=progress"
echo ""
echo "🚀 When USB boots, crypto wipe tool will auto-start!"
echo "🛑 Press Ctrl+C during countdown to get normal shell"
