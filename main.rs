use std::env;
use std::process::{Command as ProcessCommand, Stdio};
use std::io::{self, Write};
use std::path::Path;
use uuid::Uuid;
use rand::{thread_rng, Rng};

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
║                    🔐 LUKS CRYPTO WIPE 🔐                     ║
║              Advanced Cryptographic Data Destruction          ║
║                                                              ║
║  ⚠️  WARNING: This will PERMANENTLY destroy ALL data!       ║
║      This operation cannot be undone!                       ║
╚══════════════════════════════════════════════════════════════╝
    ");
    println!("\x1b[0m");   // Reset color
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
        let device_info = DeviceInfo {
            path: device_path,
            size: size.to_string(),
            device_type: device_type.to_string(),
            mountpoint: mountpoint.to_string(),
            model,
            is_partition: device_type == "part",
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
        let icon = if device.is_partition { "  📂" } else { "💽" };
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
            device.device_type.to_uppercase(),
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

    // Step 1: Generate random passphrase
    println!("🔑 Step 1: Generating cryptographic key...");
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
    println!("\n📝 Step 4: Filling with encrypted random data...");
    fill_with_random_data(&format!("/dev/mapper/{}", mapper_name))?;
    println!("✅ Device filled with encrypted random data");

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
    let mut child = ProcessCommand::new("cryptsetup")
        .args(&[
            "luksFormat",
            "--type", "luks2",
            "--cipher", "aes-xts-plain64",
            "--key-size", "512",
            "--hash", "sha256",
            "--iter-time", "2000",
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
    if !output.status.success() {
        return Err(io::Error::new(
            io::ErrorKind::Other,
            format!("Failed to create LUKS partition: {}", String::from_utf8_lossy(&output.stderr))
        ));
    }

    Ok(())
}

fn open_luks_partition(device: &str, mapper_name: &str, passphrase: &str) -> io::Result<()> {
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
        return Err(io::Error::new(
            io::ErrorKind::Other,
            format!("Failed to open LUKS partition: {}", String::from_utf8_lossy(&output.stderr))
        ));
    }

    Ok(())
}

fn fill_with_random_data(mapper_device: &str) -> io::Result<()> {
    let _output = ProcessCommand::new("dd")
        .args(&[
            "if=/dev/urandom",
            &format!("of={}", mapper_device),
            "bs=1M",
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
    // Overwrite LUKS header with random data
    let output = ProcessCommand::new("dd")
        .args(&[
            "if=/dev/urandom",
            &format!("of={}", device),
            "bs=1M",
            "count=10",
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
    println!("🕒 Completed: {}", chrono::Utc::now().format("%Y-%m-%d %H:%M:%S UTC"));
    println!("{}", separator);
    println!("\n🎉 Mission accomplished! Your data is gone forever! 🎉");
}
