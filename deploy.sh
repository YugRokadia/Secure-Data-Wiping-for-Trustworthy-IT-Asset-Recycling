#!/bin/bash
# deploy.sh - Deploy crypto wipe tool to SystemRescue USB

set -e

echo "🔥 SystemRescue USB Crypto Wipe Deployment"
echo "=========================================="

# Configuration
USB_DEVICE=""
USB_MOUNT=""
TOOL_NAME="wipeshit"

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

# Function to print colored output
print_status() {
    echo -e "${GREEN}✅ $1${NC}"
}

print_warning() {
    echo -e "${YELLOW}⚠️  $1${NC}"
}

print_error() {
    echo -e "${RED}❌ $1${NC}"
}

print_info() {
    echo -e "${BLUE}ℹ️  $1${NC}"
}

# Check if running as root for device operations
check_permissions() {
    if [[ $EUID -eq 0 ]]; then
        print_warning "Running as root - be careful with device operations!"
    fi
}

# Build the tool if not already built
build_tool() {
    print_info "Checking if tool is built..."
    
    if [ ! -f "target/release/$TOOL_NAME" ]; then
        echo "📦 Building crypto wipe tool..."
        cargo build --release
        
        if [ $? -ne 0 ]; then
            print_error "Build failed!"
            exit 1
        fi
    fi
    
    print_status "Tool ready: $(du -h target/release/$TOOL_NAME | cut -f1)"
}

# Detect SystemRescue USB
detect_usb() {
    print_info "Detecting SystemRescue USB..."
    
    # Look for mounted RESCUE partitions
    RESCUE_MOUNTS=$(mount | grep -i rescue | head -1)
    
    if [ ! -z "$RESCUE_MOUNTS" ]; then
        USB_MOUNT=$(echo $RESCUE_MOUNTS | cut -d' ' -f3)
        USB_DEVICE=$(echo $RESCUE_MOUNTS | cut -d' ' -f1)
        print_status "Found SystemRescue USB at: $USB_DEVICE mounted at $USB_MOUNT"
        return 0
    fi
    
    # Show available USB devices
    echo ""
    echo "📱 Available USB devices:"
    lsblk -o NAME,SIZE,TYPE,MOUNTPOINT | grep -E "(disk|part)" | grep -v "loop\|sr"
    echo ""
    
    # Ask user to specify
    read -p "Enter USB device path (e.g., /dev/sdb1): " USB_DEVICE
    
    if [ ! -b "$USB_DEVICE" ]; then
        print_error "Device $USB_DEVICE not found!"
        exit 1
    fi
    
    # Try to mount if not mounted
    USB_MOUNT="/mnt/rescue_usb"
    sudo mkdir -p "$USB_MOUNT"
    
    if ! mount | grep -q "$USB_DEVICE"; then
        print_info "Mounting $USB_DEVICE to $USB_MOUNT..."
        sudo mount "$USB_DEVICE" "$USB_MOUNT"
    else
        USB_MOUNT=$(mount | grep "$USB_DEVICE" | cut -d' ' -f3)
    fi
}

# Copy tool and create scripts
deploy_files() {
    print_info "Deploying files to USB..."
    
    # Copy main executable
    if [ -w "$USB_MOUNT" ]; then
        cp target/release/$TOOL_NAME "$USB_MOUNT/"
    else
        sudo cp target/release/$TOOL_NAME "$USB_MOUNT/"
    fi
    print_status "Copied $TOOL_NAME to USB"
    
    # Create launcher script
    LAUNCHER_SCRIPT="$USB_MOUNT/start-crypto-wipe.sh"
    if [ -w "$USB_MOUNT" ]; then
        cat > "$LAUNCHER_SCRIPT" << 'EOF'
#!/bin/bash
clear
echo "╔══════════════════════════════════════════════════════════════╗"
echo "║                🔥 CRYPTO WIPE LAUNCHER 🔥                    ║"
echo "║              SystemRescue USB Data Destruction              ║"
echo "╚══════════════════════════════════════════════════════════════╝"
echo ""
echo "🚀 Starting crypto wipe tool..."
echo ""

# Find USB mount point
USB_MOUNT="/mnt"
if [ ! -f "$USB_MOUNT/wipeshit" ]; then
    # Try to auto-detect USB
    USB_MOUNT=$(mount | grep -E "(sdb1|sdc1)" | cut -d' ' -f3 | head -1)
    if [ -z "$USB_MOUNT" ]; then
        USB_MOUNT="/run/archiso/bootmnt"
    fi
fi

# Copy tool to RAM and execute
if [ -f "$USB_MOUNT/wipeshit" ]; then
    cp "$USB_MOUNT/wipeshit" /tmp/
    chmod +x /tmp/wipeshit
    echo "✅ Tool loaded into RAM"
    echo ""
    exec /tmp/wipeshit
else
    echo "❌ Error: wipeshit not found on USB"
    echo "💡 Manual command: mount /dev/sdb1 /mnt && cp /mnt/wipeshit /tmp/ && chmod +x /tmp/wipeshit && /tmp/wipeshit"
    exit 1
fi
EOF
    else
        sudo tee "$LAUNCHER_SCRIPT" > /dev/null << 'EOF'
#!/bin/bash
clear
echo "╔══════════════════════════════════════════════════════════════╗"
echo "║                🔥 CRYPTO WIPE LAUNCHER 🔥                    ║"
echo "║              SystemRescue USB Data Destruction              ║"
echo "╚══════════════════════════════════════════════════════════════╝"
echo ""
echo "🚀 Starting crypto wipe tool..."
echo ""

# Find USB mount point
USB_MOUNT="/mnt"
if [ ! -f "$USB_MOUNT/wipeshit" ]; then
    # Try to auto-detect USB
    USB_MOUNT=$(mount | grep -E "(sdb1|sdc1)" | cut -d' ' -f3 | head -1)
    if [ -z "$USB_MOUNT" ]; then
        USB_MOUNT="/run/archiso/bootmnt"
    fi
fi

# Copy tool to RAM and execute
if [ -f "$USB_MOUNT/wipeshit" ]; then
    cp "$USB_MOUNT/wipeshit" /tmp/
    chmod +x /tmp/wipeshit
    echo "✅ Tool loaded into RAM"
    echo ""
    exec /tmp/wipeshit
else
    echo "❌ Error: wipeshit not found on USB"
    echo "💡 Manual command: mount /dev/sdb1 /mnt && cp /mnt/wipeshit /tmp/ && chmod +x /tmp/wipeshit && /tmp/wipeshit"
    exit 1
fi
EOF
    fi
    print_status "Created launcher script"
    
    # Create quick command reference
    QUICK_CMD="$USB_MOUNT/QUICK-START.txt"
    if [ -w "$USB_MOUNT" ]; then
        cat > "$QUICK_CMD" << EOF
🔥 CRYPTO WIPE QUICK START COMMANDS
==================================

Boot SystemRescue USB, then run ONE of these commands:

METHOD 1 (Recommended):
mount /dev/sdb1 /mnt && cp /mnt/wipeshit /tmp/ && chmod +x /tmp/wipeshit && /tmp/wipeshit

METHOD 2 (Using launcher):
mount /dev/sdb1 /mnt && bash /mnt/start-crypto-wipe.sh

METHOD 3 (Manual):
mount /dev/sdb1 /mnt
cp /mnt/wipeshit /tmp/
chmod +x /tmp/wipeshit
/tmp/wipeshit

⚠️  WARNING: This tool will PERMANENTLY destroy ALL data on selected devices!
EOF
    else
        sudo tee "$QUICK_CMD" > /dev/null << EOF
🔥 CRYPTO WIPE QUICK START COMMANDS
==================================

Boot SystemRescue USB, then run ONE of these commands:

METHOD 1 (Recommended):
mount /dev/sdb1 /mnt && cp /mnt/wipeshit /tmp/ && chmod +x /tmp/wipeshit && /tmp/wipeshit

METHOD 2 (Using launcher):
mount /dev/sdb1 /mnt && bash /mnt/start-crypto-wipe.sh

METHOD 3 (Manual):
mount /dev/sdb1 /mnt
cp /mnt/wipeshit /tmp/
chmod +x /tmp/wipeshit
/tmp/wipeshit

⚠️  WARNING: This tool will PERMANENTLY destroy ALL data on selected devices!
EOF
    fi
    print_status "Created quick start guide"
}

# Verify deployment
verify_deployment() {
    print_info "Verifying deployment..."
    
    if [ -f "$USB_MOUNT/$TOOL_NAME" ]; then
        SIZE=$(du -h "$USB_MOUNT/$TOOL_NAME" | cut -f1)
        print_status "Tool deployed: $SIZE"
    else
        print_error "Tool not found on USB!"
        exit 1
    fi
    
    if [ -f "$USB_MOUNT/start-crypto-wipe.sh" ]; then
        print_status "Launcher script deployed"
    fi
    
    if [ -f "$USB_MOUNT/QUICK-START.txt" ]; then
        print_status "Quick start guide deployed"
    fi
}

# Show usage instructions
show_instructions() {
    echo ""
    echo "🎉 DEPLOYMENT SUCCESSFUL! 🎉"
    echo "============================="
    echo ""
    echo "Your SystemRescue USB now contains:"
    echo "├── wipeshit                 # Main crypto wipe tool"
    echo "├── start-crypto-wipe.sh     # Launcher script"
    echo "└── QUICK-START.txt          # Command reference"
    echo ""
    echo "🚀 TO USE:"
    echo "1. Boot from your SystemRescue USB"
    echo "2. Wait for SystemRescue to load"
    echo "3. Run this command:"
    echo ""
    echo -e "${GREEN}mount /dev/sdb1 /mnt && cp /mnt/wipeshit /tmp/ && chmod +x /tmp/wipeshit && /tmp/wipeshit${NC}"
    echo ""
    echo "⚠️  Remember: This tool will PERMANENTLY destroy data!"
    echo "🛡️  Only use on devices you want to securely wipe!"
}

# Main deployment process
main() {
    echo ""
    check_permissions
    build_tool
    detect_usb
    deploy_files
    verify_deployment
    show_instructions
}

# Run main function
main "$@"
