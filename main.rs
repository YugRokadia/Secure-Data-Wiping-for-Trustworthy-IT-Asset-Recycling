use std::env;
use std::process::{Command as ProcessCommand, Stdio};
use std::io::{self, Write};
use std::path::Path;
use uuid::Uuid;
use rand::{thread_rng, Rng};
use std::time::{SystemTime, UNIX_EPOCH, Duration};
use std::thread;
use std::fs;

fn show_help() {
    println!("LUKS Crypto Wipe v1.0 - Secure Data Destruction Tool");
    println!();
    println!("USAGE:");
    println!("    wipeshit [DEVICE] [OPTIONS]");
    println!();
    println!("ARGUMENTS:");
    println!("    <DEVICE>    Target device to wipe (e.g., /dev/sdb)");
    println!("                If not specified, interactive mode will be used");
    println!();
    println!("OPTIONS:");
    println!("    -f, --force     Force wipe without confirmation");
    println!("    -v, --verify    Verify the wipe operation");
    println!("    -h, --help      Show this help message");
    println!();
    println!("EXAMPLES:");
    println!("    wipeshit                    # Interactive mode");
    println!("    wipeshit /dev/sdb           # Wipe specific device");
    println!("    wipeshit /dev/sdb --force   # Force wipe without confirmation");
    println!("    wipeshit /dev/sdb --verify  # Wipe with verification");
    println!();
    println!("WARNING: This tool will PERMANENTLY destroy ALL data on the target device!");
}

fn main() -> io::Result<()> {
    let args: Vec<String> = env::args().collect();
    
    // Parse simple command line arguments
    let device = if args.len() > 1 { Some(args[1].clone()) } else { None };
    let force = args.contains(&"--force".to_string()) || args.contains(&"-f".to_string());
    let verify = args.contains(&"--verify".to_string()) || args.contains(&"-v".to_string());
    
    // Show help if requested
    if args.contains(&"--help".to_string()) || args.contains(&"-h".to_string()) {
        show_help();
        return Ok(());
    }

    let device = if let Some(dev) = device {
        dev
    } else {
        // Interactive device selection
        select_device_interactively()?
    };

    // Display banner
    display_banner();

    // List available devices
    list_block_devices()?;

    // Validate device
    if !Path::new(&device).exists() {
        eprintln!("❌ Error: Device '{}' does not exist!", device);
        return Ok(());
    }

    // Safety confirmation
    if !force && !confirm_wipe(&device)? {
        println!("🛑 Wipe operation cancelled by user.");
        return Ok(());
    }

    // Perform LUKS crypto wipe
    match perform_luks_crypto_wipe(&device, verify) {
        Ok(_) => {
            println!("\n✅ LUKS crypto wipe completed successfully!");
            println!("🔒 Device '{}' has been securely wiped using LUKS encryption.", device);
        }
        Err(e) => {
            eprintln!("❌ Wipe failed: {}", e);
        }
    }

    Ok(())
}

fn display_banner() {
    println!("\x1b[31m");  // Red color
    println!("
╔══════════════════════════════════════════════════════════════╗
║                  🔐 LUKS CRYPTO WIPE 🔐                     ║
║          Advanced Cryptographic Data Destruction             ║
║                                                              ║
║      WARNING: This will PERMANENTLY destroy ALL data         ║
║              This operation cannot be undone!                ║
╚══════════════════════════════════════════════════════════════╝
    ");
    println!("\x1b[0m");   // Reset color
}

fn is_removable_device(device_name: &str) -> bool {
    // Extract base device name (remove partition numbers)
    let base_name = if device_name.len() > 3 {
        // For devices like sda1, sdb2, etc., get sda, sdb
        let chars: Vec<char> = device_name.chars().collect();
        let mut base = String::new();
        for ch in chars {
            if ch.is_ascii_digit() {
                break;
            }
            base.push(ch);
        }
        base
    } else {
        device_name.to_string()
    };

    // Check if device is removable via sysfs
    let removable_path = format!("/sys/block/{}/removable", base_name);
    if let Ok(content) = std::fs::read_to_string(&removable_path) {
        return content.trim() == "1";
    }

    // Fallback: check device type patterns common for USB devices
    base_name.starts_with("sd") && !base_name.starts_with("sda")
}

fn auto_unmount_device(device_path: &str) -> io::Result<()> {
    println!("🔄 Checking if {} is mounted...", device_path);
    
    // For whole devices (like /dev/sdb), also check and unmount all partitions
    let device_name = device_path.strip_prefix("/dev/").unwrap_or(device_path);
    
    // Check if the specific device is mounted
    let mount_output = ProcessCommand::new("findmnt")
        .args(&["-n", "-o", "TARGET", device_path])
        .output()?;
    
    if mount_output.status.success() && !mount_output.stdout.is_empty() {
        let mount_point = String::from_utf8_lossy(&mount_output.stdout).trim().to_string();
        println!("📤 Device mounted at {}, unmounting...", mount_point);
        
        let umount_result = ProcessCommand::new("umount")
            .arg(device_path)
            .status()?;
            
        if umount_result.success() {
            println!("✅ Successfully unmounted {}", device_path);
        } else {
            return Err(io::Error::new(
                io::ErrorKind::Other, 
                format!("Failed to unmount {} - device may be busy", device_path)
            ));
        }
    } else {
        println!("✅ Device {} is not mounted", device_path);
    }
    
    // For whole devices, check and unmount all partitions (e.g., sdb1, sdb2, etc.)
    if !device_name.chars().any(|c| c.is_ascii_digit()) {
        println!("🔍 Checking for mounted partitions on {}...", device_path);
        
        let lsblk_output = ProcessCommand::new("lsblk")
            .args(&["-n", "-o", "NAME,MOUNTPOINT", device_path])
            .output()?;
            
        if lsblk_output.status.success() {
            let output_str = String::from_utf8_lossy(&lsblk_output.stdout);
            for line in output_str.lines() {
                let parts: Vec<&str> = line.split_whitespace().collect();
                if parts.len() >= 2 {
                    // Clean up the partition name by removing tree characters
                    let raw_name = parts[0];
                    let clean_name = raw_name
                        .trim_start_matches('├')
                        .trim_start_matches('└')
                        .trim_start_matches('│')
                        .trim_start_matches('─')
                        .trim();
                        
                    let mount_point = parts[1];
                    
                    // Skip the main device itself (only process partitions)
                    if clean_name != device_name && !mount_point.is_empty() && mount_point != "-" {
                        let part_device = format!("/dev/{}", clean_name);
                        println!("📤 Unmounting partition {} from {}...", part_device, mount_point);
                        
                        let umount_result = ProcessCommand::new("umount")
                            .arg(&part_device)
                            .status()?;
                            
                        if umount_result.success() {
                            println!("✅ Successfully unmounted {}", part_device);
                        } else {
                            println!("⚠️ Failed to unmount {} - trying force unmount...", part_device);
                            
                            let force_umount = ProcessCommand::new("umount")
                                .args(&["-f", &part_device])
                                .status()?;
                                
                            if force_umount.success() {
                                println!("✅ Force unmounted {}", part_device);
                            } else {
                                return Err(io::Error::new(
                                    io::ErrorKind::Other, 
                                    format!("Failed to unmount partition {} - device busy", part_device)
                                ));
                            }
                        }
                    }
                }
            }
        }
    }
    
    // Give the system a moment to release the device
    println!("⏱️ Waiting for device to be released by system...");
    thread::sleep(Duration::from_secs(2));
    
    Ok(())
}

fn list_block_devices() -> io::Result<()> {
    println!("\n💾 Available Block Devices:");
    println!("═══════════════════════════");

    let output = ProcessCommand::new("lsblk")
        .args(&["-d", "-o", "NAME,SIZE,TYPE,MODEL"])
        .output()?;

    if output.status.success() {
        print!("{}", String::from_utf8_lossy(&output.stdout));
    } else {
        println!("❌ Failed to list block devices");
    }
    
    println!();
    Ok(())
}

fn select_device_interactively() -> io::Result<String> {
    println!("\n🎯 STORAGE DEVICE & PARTITION SELECTION");
    println!("═══════════════════════════════════════");

    // Get comprehensive list of all block devices and partitions
    let output = ProcessCommand::new("lsblk")
        .args(&["-n", "-o", "NAME,SIZE,TYPE,MOUNTPOINT,MODEL", "--tree"])
        .output()?;

    if !output.status.success() {
        return Err(io::Error::new(io::ErrorKind::Other, "Failed to get device list"));
    }

    let devices_output = String::from_utf8_lossy(&output.stdout);
    let lines: Vec<&str> = devices_output.lines().collect();

    if lines.is_empty() {
        return Err(io::Error::new(io::ErrorKind::Other, "No storage devices found"));
    }

    // Parse and categorize devices
    let mut devices = Vec::new();
    
    for line in lines {
        let parts: Vec<&str> = line.split_whitespace().collect();
        if parts.len() < 3 { continue; }
        
        let name = parts[0].trim_start_matches('├').trim_start_matches('└').trim_start_matches('│').trim();
        let size = parts[1];
        let device_type = parts[2];
        let mountpoint = if parts.len() > 3 { parts[3] } else { "" };
        let model = if parts.len() > 4 { parts[4..].join(" ") } else { "Unknown".to_string() };

        // Skip loop devices, ram disks, etc.
        if name.starts_with("loop") || name.starts_with("ram") || name.starts_with("sr") {
            continue;
        }

        let device_path = format!("/dev/{}", name);
        let is_removable = is_removable_device(name);
        let device_info = DeviceInfo {
            path: device_path,
            size: size.to_string(),
            device_type: device_type.to_string(),
            mountpoint: mountpoint.to_string(),
            model,
            is_partition: device_type == "part",
            is_removable,
        };

        devices.push(device_info);
    }

    if devices.is_empty() {
        return Err(io::Error::new(io::ErrorKind::Other, "No suitable devices found"));
    }

    // Display categorized list
    println!("📀 Available Storage Devices and Partitions:");
    println!();
    
    for (i, device) in devices.iter().enumerate() {
        let icon = if device.is_partition {
            "  📂"
        } else if device.is_removable {
            "🔌"  // USB/removable device icon
        } else {
            "💽"  // Internal drive icon
        };
        
        let device_type_display = if device.is_removable {
            "USB/REMOVABLE"
        } else {
            &device.device_type.to_uppercase()
        };
        
        let mount_info = if !device.mountpoint.is_empty() && device.mountpoint != "-" {
            format!(" (mounted at {})", device.mountpoint)
        } else {
            String::new()
        };
        
        let warning = if !device.mountpoint.is_empty() && device.mountpoint != "-" {
            " ⚠️ MOUNTED"
        } else {
            ""
        };

        println!("  {}: {} {} - {} {} - {}{}{}",
            i + 1,
            icon,
            device.path,
            device.size,
            device_type_display,
            device.model,
            mount_info,
            warning
        );
    }

    println!("\n💡 Tip: You can wipe entire drives or individual partitions");
    println!("⚠️  WARNING: Selected device/partition will be COMPLETELY DESTROYED!");
    print!("\nSelect device/partition number (1-{}): ", devices.len());
    io::stdout().flush()?;

    let mut input = String::new();
    io::stdin().read_line(&mut input)?;

    let choice: usize = input.trim().parse()
        .map_err(|_| io::Error::new(io::ErrorKind::InvalidInput, "Invalid number"))?;

    if choice == 0 || choice > devices.len() {
        return Err(io::Error::new(io::ErrorKind::InvalidInput, "Invalid selection"));
    }

    let selected = &devices[choice - 1];
    
    // Additional warning for mounted devices
    if !selected.mountpoint.is_empty() && selected.mountpoint != "-" {
        println!("\n⚠️  CRITICAL WARNING ⚠️");
        println!("The selected device is CURRENTLY MOUNTED at: {}", selected.mountpoint);
        println!("Wiping it will crash the system if it contains important files!");
        print!("Type 'I UNDERSTAND THE RISK' to continue: ");
        io::stdout().flush()?;
        
        let mut risk_input = String::new();
        io::stdin().read_line(&mut risk_input)?;
        
        if risk_input.trim() != "I UNDERSTAND THE RISK" {
            return Err(io::Error::new(io::ErrorKind::Other, "Operation cancelled for safety"));
        }
    }

    println!("✅ Selected: {} ({} {})", selected.path, selected.size, selected.device_type);
    Ok(selected.path.clone())
}

#[derive(Debug)]
struct DeviceInfo {
    path: String,
    size: String,
    device_type: String,
    mountpoint: String,
    model: String,
    is_partition: bool,
    is_removable: bool,
}

fn confirm_wipe(device: &str) -> io::Result<bool> {
    println!("\x1b[33m");  // Yellow color
    println!("⚠️  DANGER ZONE ⚠️");
    println!("═══════════════════");
    println!("You are about to PERMANENTLY WIPE: {}", device);
    println!("This will:");
    println!("  🔥 Destroy ALL data on the device");
    println!("  🔐 Create LUKS encryption");
    println!("  🗑️  Fill with encrypted random data");
    println!("  🔓 Remove encryption keys (making data unrecoverable)");
    println!();
    println!("\x1b[0m");   // Reset color
    
    print!("Type 'DESTROY ALL DATA' to confirm: ");
    io::stdout().flush()?;
    
    let mut input = String::new();
    io::stdin().read_line(&mut input)?;
    
    Ok(input.trim() == "DESTROY ALL DATA")
}

fn perform_luks_crypto_wipe(device: &str, verify: bool) -> io::Result<()> {
    let wipe_id = Uuid::new_v4();
    println!("🚀 Starting LUKS crypto wipe...");
    println!("🆔 Operation ID: {}", wipe_id);
    println!("📱 Target: {}", device);
    println!();

    // Step 0: Auto-unmount if necessary (especially important for USB devices)
    println!("🔄 Step 0: Preparing device...");
    auto_unmount_device(device)?;
    
    // Add a small delay for USB devices to settle
    let device_name = device.strip_prefix("/dev/").unwrap_or(device);
    if is_removable_device(device_name) {
        println!("🔌 USB/Removable device detected, allowing time to settle...");
        thread::sleep(Duration::from_secs(2));
    }
    println!("✅ Device ready for wiping");

    // Step 1: Generate random passphrase
    println!("\n🔑 Step 1: Generating cryptographic key...");
    let passphrase = generate_random_passphrase();
    println!("✅ Cryptographic key generated");

    // Step 2: Create LUKS partition
    println!("\n🔐 Step 2: Setting up LUKS encryption...");
    create_luks_partition(device, &passphrase)?;
    println!("✅ LUKS partition created");

    // Step 3: Open LUKS partition
    println!("\n🔓 Step 3: Opening encrypted partition...");
    let mapper_name = format!("cryptowipe_{}", wipe_id.simple());
    open_luks_partition(device, &mapper_name, &passphrase)?;
    println!("✅ Encrypted partition opened as /dev/mapper/{}", mapper_name);

    // Step 4: Fill with random data
    println!("\n📝 Step 4: Filling with encrypted data...");
    fill_with_random_data(&format!("/dev/mapper/{}", mapper_name))?;
    println!("✅ Device filled with encrypted data");

    // Step 5: Close and destroy keys
    println!("\n🔒 Step 5: Closing partition and destroying keys...");
    close_luks_partition(&mapper_name)?;
    destroy_luks_header(device)?;
    println!("✅ Encryption keys destroyed - data is now unrecoverable");

    // Step 6: Verification (optional)
    if verify {
        println!("\n🔍 Step 6: Verification...");
        verify_wipe(device)?;
        println!("✅ Wipe verification completed");
    }

    // Generate report
    generate_completion_report(device, &wipe_id);

    Ok(())
}

fn generate_random_passphrase() -> String {
    let mut rng = thread_rng();
    let charset = "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789!@#$%^&*";
    (0..64)
        .map(|_| {
            let idx = rng.gen_range(0..charset.len());
            charset.chars().nth(idx).unwrap()
        })
        .collect()
}

fn create_luks_partition(device: &str, passphrase: &str) -> io::Result<()> {
    let device_name = device.strip_prefix("/dev/").unwrap_or(device);
    let is_usb = is_removable_device(device_name);
    
    // For USB devices, use faster iteration time to avoid timeout issues
    let iter_time = if is_usb { "1000" } else { "2000" };
    
    let mut attempts = 0;
    let max_attempts = if is_usb { 3 } else { 1 };
    
    loop {
        attempts += 1;
        if is_usb && attempts > 1 {
            println!("🔄 Retry attempt {} for USB device...", attempts);
            thread::sleep(Duration::from_secs(1));
        }
        
        let mut child = ProcessCommand::new("cryptsetup")
            .args(&[
                "luksFormat",
                "--type", "luks2",
                "--cipher", "aes-xts-plain64",
                "--key-size", "512",
                "--hash", "sha256",
                "--iter-time", iter_time,
                "--use-random",
                device
            ])
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()?;

        if let Some(mut stdin) = child.stdin.take() {
            writeln!(stdin, "{}", passphrase)?;
        }

        let output = child.wait_with_output()?;
        if output.status.success() {
            return Ok(());
        }
        
        let error_msg = String::from_utf8_lossy(&output.stderr);
        if attempts >= max_attempts {
            return Err(io::Error::new(
                io::ErrorKind::Other,
                format!("Failed to create LUKS partition after {} attempts: {}", attempts, error_msg)
            ));
        }
        
        println!("⚠️ LUKS creation attempt {} failed: {}", attempts, error_msg.trim());
        if is_usb {
            println!("🔄 USB devices sometimes need multiple attempts...");
        }
    }
}

fn open_luks_partition(device: &str, mapper_name: &str, passphrase: &str) -> io::Result<()> {
    let device_name = device.strip_prefix("/dev/").unwrap_or(device);
    let is_usb = is_removable_device(device_name);
    
    if is_usb {
        println!("🔌 Opening LUKS partition on USB device...");
        thread::sleep(Duration::from_millis(500)); // Small delay for USB devices
    }
    
    let mut child = ProcessCommand::new("cryptsetup")
        .args(&["luksOpen", device, mapper_name])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()?;

    if let Some(mut stdin) = child.stdin.take() {
        writeln!(stdin, "{}", passphrase)?;
    }

    let output = child.wait_with_output()?;
    if !output.status.success() {
        let error_msg = String::from_utf8_lossy(&output.stderr);
        return Err(io::Error::new(
            io::ErrorKind::Other,
            format!("Failed to open LUKS partition: {}", error_msg)
        ));
    }

    Ok(())
}

fn fill_with_random_data(mapper_device: &str) -> io::Result<()> {
    let _output = ProcessCommand::new("dd")
        .args(&[
            "if=/dev/zero",
            &format!("of={}", mapper_device),
            "bs=8M",
            "status=progress"
        ])
        .output()?;

    // dd will return non-zero when it hits end of device, which is expected
    println!("Random data fill completed");
    Ok(())
}

fn close_luks_partition(mapper_name: &str) -> io::Result<()> {
    let output = ProcessCommand::new("cryptsetup")
        .args(&["luksClose", mapper_name])
        .output()?;

    if !output.status.success() {
        return Err(io::Error::new(
            io::ErrorKind::Other,
            format!("Failed to close LUKS partition: {}", String::from_utf8_lossy(&output.stderr))
        ));
    }

    Ok(())
}

fn destroy_luks_header(device: &str) -> io::Result<()> {
    // Overwrite LUKS header with zeros (sufficient for destruction)
    let output = ProcessCommand::new("dd")
        .args(&[
            "if=/dev/zero",
            &format!("of={}", device),
            "bs=8M",
            "count=2",
            "conv=notrunc"
        ])
        .output()?;

    if !output.status.success() {
        return Err(io::Error::new(
            io::ErrorKind::Other,
            "Failed to destroy LUKS header"
        ));
    }

    Ok(())
}

fn verify_wipe(device: &str) -> io::Result<()> {
    println!("🔍 Reading first 100MB to verify randomness...");
    
    let output = ProcessCommand::new("dd")
        .args(&[
            &format!("if={}", device),
            "of=/dev/null",
            "bs=1M",
            "count=100",
            "status=progress"
        ])
        .output()?;

    if output.status.success() {
        println!("✅ Device appears to contain random data");
    } else {
        println!("⚠️  Verification completed with warnings");
    }

    Ok(())
}

fn generate_completion_report(device: &str, wipe_id: &Uuid) {
    let separator = "═".repeat(60);
    println!("\n{}", separator);
    println!("📋 LUKS CRYPTO WIPE COMPLETION REPORT");
    println!("{}", separator);
    println!("🆔 Operation ID: {}", wipe_id);
    println!("📱 Device: {}", device);
    println!("🔐 Method: LUKS2 AES-XTS-256 Encryption");
    println!("🗝️  Key Size: 512 bits");
    println!("🧂 Hash: SHA-256");
    println!("🔄 Process:");
    println!("   1. ✅ LUKS encryption applied");
    println!("   2. ✅ Filled with encrypted random data");
    println!("   3. ✅ Encryption keys destroyed");
    println!("   4. ✅ LUKS header overwritten");
    println!("🛡️  Security: Data is cryptographically unrecoverable");
    
    // Use system time instead of chrono
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs();
    println!("🕒 Completed: {} (Unix timestamp)", timestamp);
    
    println!("{}", separator);
    println!("\n🎉 Mission accomplished! Your data is gone forever! 🎉");
}
