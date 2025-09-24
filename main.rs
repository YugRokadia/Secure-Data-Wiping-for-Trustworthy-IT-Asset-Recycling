
mod UI;
mod core;

use std::env;
use std::process::{Command as ProcessCommand};
use std::io::{self, Write};
use std::path::Path;
use std::time::Duration;
use std::thread;

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

    // When no arguments are provided, run in GUI mode
    if device.is_none() {
        return UI::run_ui().map_err(|e| io::Error::new(io::ErrorKind::Other, e.to_string()));
    }

    // CLI mode with specified device
    let device = device.unwrap();

    // Display banner
    display_banner();

    // List available devices
    list_block_devices()?;

    // Validate device
    if !Path::new(&device).exists() {
        eprintln!("Error: Device {} not found", device);
        return Ok(());
    }

    // Safety confirmation
    if !force && !confirm_wipe(&device)? {
        println!("Operation cancelled.");
        return Ok(());
    }

    // Perform LUKS crypto wipe
    match core::perform_luks_crypto_wipe(&device, verify, |progress, status| {
        print!("\r{}: {:.1}%", status, progress * 100.0);
        std::io::stdout().flush().unwrap();
    }) {
        Ok(certificate) => {
            println!("\n\nOperation completed successfully!");
            println!("\nCompletion Certificate:\n{}", certificate);
        }
        Err(e) => {
            eprintln!("\n\nError: {}", e);
            return Err(e);
        }
    }

    Ok(())
}

fn display_banner() {
    println!("\x1b[31m");  // Red color
    println!("
╔══════════════════════════════════════════════════════════════╗
║                    🔐 LUKS CRYPTO WIPE 🔐                   ║
║              Advanced Cryptographic Data Destruction         ║
║                                                              ║
║  ⚠️  WARNING: This will PERMANENTLY destroy ALL data!        ║
║      This operation cannot be undone!                        ║
╚══════════════════════════════════════════════════════════════╝
    ");
    println!("\x1b[0m");   // Reset color
}

fn is_removable_device(device_name: &str) -> bool {
    // Extract base device name (remove partition numbers)
    let base = if device_name.chars().any(|c| c.is_ascii_digit()) {
        device_name.trim_end_matches(|c: char| c.is_ascii_digit())
    } else {
        device_name
    };

    // Check if device is removable via sysfs
    let removable_path = format!("/sys/block/{}/removable", base);
    if let Ok(content) = std::fs::read_to_string(&removable_path) {
        return content.trim() == "1";
    }

    // Fallback: check device type patterns common for USB devices
    base.starts_with("sd") && !base.starts_with("sda")
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
        println!("🔄 Unmounting {}", device_path);
        let unmount = ProcessCommand::new("umount")
            .arg(device_path)
            .output()?;
        
        if !unmount.status.success() {
            return Err(io::Error::new(
                io::ErrorKind::Other,
                format!("Failed to unmount {}", device_path)
            ));
        }
    }
    
    // For whole devices, check and unmount all partitions
    if !device_name.chars().any(|c| c.is_ascii_digit()) {
        // List all partitions
        let partition_output = ProcessCommand::new("lsblk")
            .args(&["-n", "-o", "NAME", "-l", device_path])
            .output()?;
        
        if partition_output.status.success() {
            let partitions = String::from_utf8_lossy(&partition_output.stdout);
            for partition in partitions.lines().skip(1) {
                let partition_path = format!("/dev/{}", partition.trim());
                let _ = auto_unmount_device(&partition_path);
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
        .args(&["-o", "NAME,SIZE,TYPE,MOUNTPOINT,MODEL", "--tree"])
        .output()?;

    if output.status.success() {
        print!("{}", String::from_utf8_lossy(&output.stdout));
    } else {
        eprintln!("Failed to list block devices");
    }
    
    println!();
    Ok(())
}

fn select_device_interactively() -> io::Result<String> {
    println!("\n🎯 STORAGE DEVICE & PARTITION SELECTION");
    println!("═══════════════════════════════════════");

    // Get comprehensive list of all block devices and partitions
    let output = ProcessCommand::new("lsblk")
        .args(&["-o", "NAME,SIZE,TYPE,MOUNTPOINT,MODEL", "--tree"])
        .output()?;

    if !output.status.success() {
        return Err(io::Error::new(io::ErrorKind::Other, "Failed to list devices"));
    }

    let devices_output = String::from_utf8_lossy(&output.stdout);
    let lines: Vec<&str> = devices_output.lines().collect();

    if lines.is_empty() {
        return Err(io::Error::new(io::ErrorKind::Other, "No devices found"));
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
        return Err(io::Error::new(io::ErrorKind::Other, "No valid devices found"));
    }

    // Display categorized list
    println!("📀 Available Storage Devices and Partitions:");
    println!();
    
    // First show removable devices
    println!("📱 Removable Devices:");
    let mut has_removable = false;
    for (i, device) in devices.iter().enumerate() {
        if device.is_removable {
            has_removable = true;
            println!("  {}. {} ({}) - {} {}{}",
                i + 1,
                device.path,
                device.size,
                device.device_type,
                if !device.model.is_empty() && device.model != "Unknown" {
                    format!("- {}", device.model)
                } else {
                    String::new()
                },
                if !device.mountpoint.is_empty() && device.mountpoint != "-" {
                    format!(" [Mounted: {}]", device.mountpoint)
                } else {
                    String::new()
                }
            );
        }
    }
    if !has_removable {
        println!("  No removable devices found");
    }
    println!();

    // Then show fixed devices
    println!("💾 Fixed Storage Devices:");
    for (i, device) in devices.iter().enumerate() {
        if !device.is_removable {
            println!("  {}. {} ({}) - {} {}{}",
                i + 1,
                device.path,
                device.size,
                device.device_type,
                if !device.model.is_empty() && device.model != "Unknown" {
                    format!("- {}", device.model)
                } else {
                    String::new()
                },
                if !device.mountpoint.is_empty() && device.mountpoint != "-" {
                    format!(" [Mounted: {}]", device.mountpoint)
                } else {
                    String::new()
                }
            );
        }
    }

    println!("\n💡 Tip: You can wipe entire drives or individual partitions");
    println!("⚠️  WARNING: Selected device/partition will be COMPLETELY DESTROYED!");
    print!("\nSelect device/partition number (1-{}): ", devices.len());
    io::stdout().flush()?;

    let mut input = String::new();
    io::stdin().read_line(&mut input)?;

    let choice: usize = input.trim().parse()
        .map_err(|_| io::Error::new(io::ErrorKind::InvalidInput, "Invalid selection"))?;

    if choice == 0 || choice > devices.len() {
        return Err(io::Error::new(io::ErrorKind::InvalidInput, "Invalid selection"));
    }

    let selected = &devices[choice - 1];
    
    // Additional warning for mounted devices
    if !selected.mountpoint.is_empty() && selected.mountpoint != "-" {
        println!("\n⚠️  WARNING: Selected device is currently mounted at {}", selected.mountpoint);
        println!("It will be automatically unmounted before wiping.");
        print!("Continue? [y/N]: ");
        io::stdout().flush()?;
        
        let mut confirm = String::new();
        io::stdin().read_line(&mut confirm)?;
        
        if !confirm.trim().eq_ignore_ascii_case("y") {
            return Err(io::Error::new(io::ErrorKind::Other, "Operation cancelled"));
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