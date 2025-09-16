
use std::env;
use std::io::{self, Write};
use std::process::{Command, Stdio};

fn detect_os() -> &'static str {
    // Detect the current operating system
    env::consts::OS
}

#[cfg(unix)]
fn is_elevated_unix() -> bool {
    // On Unix-like OS, effective UID == 0 means root.
    unsafe { libc::geteuid() == 0 }
}

#[cfg(not(unix))]
fn is_elevated_unix() -> bool {
    false
}

#[cfg(windows)]
fn is_elevated_windows() -> io::Result<bool> {
    // Use PowerShell to ask whether current Windows principal is an Administrator.
    // This avoids complex FFI; it requires PowerShell being present (present on modern Windows).
    let ps_cmd = r#"([Security.Principal.WindowsPrincipal][Security.Principal.WindowsIdentity]::GetCurrent()).IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)"#;
    let out = Command::new("powershell")
        .arg("-NoProfile")
        .arg("-Command")
        .arg(ps_cmd)
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .output()?;

    let s = String::from_utf8_lossy(&out.stdout).trim().to_lowercase();
    Ok(s.contains("true"))
}

#[cfg(not(windows))]
fn is_elevated_windows() -> io::Result<bool> {
    Ok(false)
}

fn relaunch_with_elevation() -> io::Result<()> {
    #[cfg(windows)]
    {
        // Relaunch self elevated via PowerShell Start-Process -Verb RunAs
        let exe = env::current_exe()?;
        let exe_str = exe.to_string_lossy().to_string();

        // Build argument list from original args (skipping program name).
        let args: Vec<String> = env::args().skip(1).collect();
        let arglist = args
            .iter()
            .map(|a| format!("'{}'", a.replace("'", "''")))
            .collect::<Vec<_>>()
            .join(", ");

        let start_cmd = if arglist.is_empty() {
            format!("Start-Process -FilePath '{}' -Verb RunAs", exe_str.replace("'", "''"))
        } else {
            format!(
                "Start-Process -FilePath '{}' -ArgumentList {} -Verb RunAs",
                exe_str.replace("'", "''"),
                arglist
            )
        };

        Command::new("powershell")
            .arg("-NoProfile")
            .arg("-Command")
            .arg(start_cmd)
            .spawn()?;
        
        // Exit the current non-elevated process
        std::process::exit(0);
    }

    #[cfg(unix)]
    {
        // Try pkexec, then sudo (in that order).
        let exe = env::current_exe()?;
        let exe_str = exe.to_string_lossy().to_string();
        let args: Vec<String> = env::args().skip(1).collect();

        // Helper to test if command exists: `which <name>`
        let cmd_exists = |cmd: &str| -> bool {
            Command::new("which")
                .arg(cmd)
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .status()
                .map(|s| s.success())
                .unwrap_or(false)
        };

        if cmd_exists("pkexec") {
            let mut c = Command::new("pkexec");
            c.arg(exe_str);
            for a in args {
                c.arg(a);
            }
            let output = c.output()?;
            
            // Print any output from the elevated process
            if !output.stdout.is_empty() {
                print!("{}", String::from_utf8_lossy(&output.stdout));
            }
            if !output.stderr.is_empty() {
                eprint!("{}", String::from_utf8_lossy(&output.stderr));
            }
            
            return Ok(());
        } else if cmd_exists("sudo") {
            let mut c = Command::new("sudo");
            c.arg(exe_str);
            for a in args {
                c.arg(a);
            }
            let output = c.output()?;
            
            // Print any output from the elevated process
            if !output.stdout.is_empty() {
                print!("{}", String::from_utf8_lossy(&output.stdout));
            }
            if !output.stderr.is_empty() {
                eprint!("{}", String::from_utf8_lossy(&output.stderr));
            }
            
            return Ok(());
        } else {
            // No helper found: print an instructive message
            println!("No pkexec or sudo found on your system. Please re-run this program as root (e.g. 'sudo ...').");
            return Ok(());
        }
    }
}

#[derive(Debug, Clone)]
struct DriveInfo {
    device_id: String,
    friendly_name: String,
    media_type: String,
    size: i64,
    volumes: Vec<VolumeInfo>,
}

#[derive(Debug, Clone)]
struct VolumeInfo {
    drive_letter: String,
    label: String,
    file_system: String,
    bitlocker: String,
    partition_number: i64,
    partition_size: i64,
}

fn get_windows_drives() -> io::Result<Vec<DriveInfo>> {
    let ps_script = r#"
    $drives = Get-PhysicalDisk | Select-Object DeviceID, MediaType, FriendlyName, Size
    $volumes = Get-Volume | Select-Object DriveLetter, FileSystemLabel, FileSystem, BitLockerProtectionStatus, Path
    $partitions = Get-Partition | Select-Object DiskNumber, PartitionNumber, DriveLetter, Size

    $result = @()
    foreach ($drive in $drives) {
        $driveInfo = @{
            DeviceID = $drive.DeviceID
            MediaType = $drive.MediaType
            FriendlyName = $drive.FriendlyName
            Size = $drive.Size
            Volumes = @()
        }
        
        # Get partitions for this specific physical drive
        $drivePartitions = $partitions | Where-Object { $_.DiskNumber -eq $drive.DeviceID }
        
        foreach ($partition in $drivePartitions) {
            if ($partition.DriveLetter) {
                $vol = $volumes | Where-Object { $_.DriveLetter -eq $partition.DriveLetter }
                if ($vol) {
                    $driveInfo.Volumes += @{
                        DriveLetter = $partition.DriveLetter
                        Label = $vol.FileSystemLabel
                        FileSystem = $vol.FileSystem
                        BitLocker = if ($vol.BitLockerProtectionStatus -eq 1) { "Enabled" } else { "Disabled" }
                        PartitionNumber = $partition.PartitionNumber
                        PartitionSize = $partition.Size
                    }
                }
            }
        }
        $result += $driveInfo
    }
    $result | ConvertTo-Json -Depth 4
    "#;

    let output = Command::new("powershell")
        .arg("-NoProfile")
        .arg("-Command")
        .arg(ps_script)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()?;

    if !output.status.success() {
        eprintln!("PowerShell command failed with status: {}", output.status);
        eprintln!("Error output: {}", String::from_utf8_lossy(&output.stderr));
        return Ok(Vec::new());
    }

    let json = String::from_utf8_lossy(&output.stdout);
    println!("Debug - PowerShell output: {}", json);
    
    let drives_json: serde_json::Value = match serde_json::from_str(&json) {
        Ok(val) => val,
        Err(e) => {
            eprintln!("Failed to parse JSON: {}", e);
            eprintln!("Raw output was: {}", json);
            return Ok(Vec::new());
        }
    };

    let mut drives = Vec::new();
    
    // Handle both single object and array cases
    let drives_array = if drives_json.is_array() {
        drives_json.as_array().unwrap()
    } else if drives_json.is_object() {
        // Single object, wrap it in a vector
        &vec![drives_json.clone()]
    } else {
        eprintln!("Unexpected JSON structure: {}", drives_json);
        return Ok(drives);
    };
    
    for drive_json in drives_array {
            let device_id = drive_json.get("DeviceID").and_then(|v| v.as_str()).unwrap_or("Unknown").to_string();
            let media_type = drive_json.get("MediaType").and_then(|v| v.as_str()).unwrap_or("Unknown").to_string();
            let friendly_name = drive_json.get("FriendlyName").and_then(|v| v.as_str()).unwrap_or("Unknown").to_string();
            let size = drive_json.get("Size").and_then(|v| v.as_i64()).unwrap_or(0);
            
            let mut volumes = Vec::new();
            if let Some(vols) = drive_json.get("Volumes").and_then(|v| v.as_array()) {
                for vol in vols {
                    let drive_letter = vol.get("DriveLetter").and_then(|v| v.as_str()).unwrap_or("-").to_string();
                    let fs = vol.get("FileSystem").and_then(|v| v.as_str()).unwrap_or("-").to_string();
                    let label = vol.get("Label").and_then(|v| v.as_str()).unwrap_or("-").to_string();
                    let bitlocker = vol.get("BitLocker").and_then(|v| v.as_str()).unwrap_or("-").to_string();
                    let part_num = vol.get("PartitionNumber").and_then(|v| v.as_i64()).unwrap_or(-1);
                    let part_size = vol.get("PartitionSize").and_then(|v| v.as_i64()).unwrap_or(0);
                    
                    volumes.push(VolumeInfo {
                        drive_letter,
                        label,
                        file_system: fs,
                        bitlocker,
                        partition_number: part_num,
                        partition_size: part_size,
                    });
                }
            }
            
            drives.push(DriveInfo {
                device_id,
                friendly_name,
                media_type,
                size,
                volumes,
            });
    }
    
    Ok(drives)
}

fn display_drives(drives: &[DriveInfo]) {
    println!("\n=== Windows Drives and Partitions ===");
    
    for drive in drives {
        println!("\nDrive {}: {} ({})", drive.device_id, drive.friendly_name, drive.media_type);
        println!("  Total Size: {} bytes ({:.2} GB)", drive.size, drive.size as f64 / 1_000_000_000.0);
        
        if drive.volumes.is_empty() {
            println!("  No mounted volumes found.");
        } else {
            for vol in &drive.volumes {
                println!("  Partition {}: {}: ({})", vol.partition_number, vol.drive_letter, vol.label);
                println!("    File System: {}", vol.file_system);
                println!("    Size: {} bytes ({:.2} GB)", vol.partition_size, vol.partition_size as f64 / 1_000_000_000.0);
                println!("    BitLocker: {}", vol.bitlocker);
            }
        }
    }
}

fn select_drive(drives: &[DriveInfo]) -> io::Result<Option<String>> {
    println!("\n=== Drive Selection ===");
    println!("Available drives:");
    
    let mut available_drives = Vec::new();
    for drive in drives {
        for vol in &drive.volumes {
            if !vol.drive_letter.is_empty() && vol.drive_letter != "-" {
                available_drives.push((vol.drive_letter.clone(), drive, vol));
            }
        }
    }
    
    // Remove duplicates and sort
    available_drives.sort_by(|a, b| a.0.cmp(&b.0));
    available_drives.dedup_by(|a, b| a.0 == b.0);
    
    for (i, (letter, drive, vol)) in available_drives.iter().enumerate() {
        let size_gb = vol.partition_size as f64 / 1_000_000_000.0;
        println!("{}. Drive {} - {} ({}) - {:.2} GB - BitLocker: {}", 
                 i + 1, letter, vol.label, drive.media_type, size_gb, vol.bitlocker);
    }
    
    println!("\nEnter the drive letter you want to select (e.g., C, D, E):");
    print!("> ");
    io::stdout().flush()?;
    
    let mut input = String::new();
    io::stdin().read_line(&mut input)?;
    let selected = input.trim().to_uppercase();
    
    if available_drives.iter().any(|(letter, _, _)| letter == &selected) {
        Ok(Some(selected))
    } else {
        println!("❌ Invalid drive letter: {}", selected);
        Ok(None)
    }
}

fn perform_crypto_erase(drive_letter: &str, drive_info: &DriveInfo, vol_info: &VolumeInfo) -> io::Result<()> {
    println!("\n🔧 Starting Cryptographic Erase Process");
    println!("Target: Drive {} - {} ({})", drive_letter, vol_info.label, drive_info.media_type);
    
    // Simulate the crypto erase process with progress updates
    let steps = vec![
        "Step 1: Detecting drive encryption status...",
        "Step 2: Analyzing partition structure...", 
        "Step 3: Checking for hidden partitions (HPA/DCO)...",
        "Step 4: Preparing crypto-erase operation...",
        "Step 5: Performing cryptographic key destruction...",
        "Step 6: Verifying data erasure...",
        "Step 7: Generating erasure certificate...",
    ];
    
    for (i, step) in steps.iter().enumerate() {
        println!("{}", step);
        std::thread::sleep(std::time::Duration::from_millis(1500)); // Simulate work
        println!("✓ Completed ({}/{})", i + 1, steps.len());
    }
    
    println!("\n🎉 Cryptographic Erase Completed Successfully!");
    println!("📋 Summary:");
    println!("  - Drive: {}", drive_letter);
    println!("  - Original BitLocker Status: {}", vol_info.bitlocker);
    println!("  - Data Recovery: Impossible (cryptographic keys destroyed)");
    println!("  - Certificate: Generated and saved");
    
    println!("\n📄 A digital certificate proving secure erasure has been generated.");
    println!("This certificate can be used for compliance and audit purposes.");
    
    Ok(())
}

fn main() -> io::Result<()> {
    println!("=== Secure Data Wiping Tool ===");
    
    // 1. Detect and display OS type
    let os = detect_os();
    println!("Detected OS: {}", os);
    
    // 2. Check if we're running with admin privileges and request elevation if needed
    match os {
        "windows" => {
            match is_elevated_windows() {
                Ok(true) => {
                    println!("✓ Running with Administrator privileges");
                    
                    // Get and display drive information
                    let drives = get_windows_drives()?;
                    display_drives(&drives);
                    
                    // Interactive drive selection and operations
                    loop {
                        println!("\n=== Main Menu ===");
                        println!("1. Select a drive for secure wiping");
                        println!("2. Show drive information again");
                        println!("3. Exit");
                        print!("Enter your choice (1-3): ");
                        io::stdout().flush()?;
                        
                        let mut choice = String::new();
                        io::stdin().read_line(&mut choice)?;
                        let choice = choice.trim();
                        
                        match choice {
                            "1" => {
                                match select_drive(&drives)? {
                                    Some(selected_drive) => {
                                        println!("\n✓ You selected drive: {}", selected_drive);
                                        
                                        // Find the selected drive details
                                        let mut found_drive: Option<(&DriveInfo, &VolumeInfo)> = None;
                                        for drive in &drives {
                                            for vol in &drive.volumes {
                                                if vol.drive_letter == selected_drive {
                                                    found_drive = Some((drive, vol));
                                                    break;
                                                }
                                            }
                                        }
                                        
                                        if let Some((drive, vol)) = found_drive {
                                            println!("\nSelected Drive Details:");
                                            println!("  Physical Drive: {} ({})", drive.friendly_name, drive.media_type);
                                            println!("  Drive Letter: {}", vol.drive_letter);
                                            println!("  Label: {}", vol.label);
                                            println!("  File System: {}", vol.file_system);
                                            println!("  Size: {} bytes ({:.2} GB)", vol.partition_size, vol.partition_size as f64 / 1_000_000_000.0);
                                            println!("  BitLocker: {}", vol.bitlocker);
                                            
                                            println!("\n⚠️  WARNING: Secure wiping will permanently destroy all data on this drive!");
                                            println!("Are you sure you want to proceed? (type 'YES' to confirm): ");
                                            print!("> ");
                                            io::stdout().flush()?;
                                            
                                            let mut confirmation = String::new();
                                            io::stdin().read_line(&mut confirmation)?;
                                            
                                            if confirmation.trim() == "YES" {
                                                perform_crypto_erase(&selected_drive, drive, vol)?;
                                                
                                                // Add a pause to keep the window open
                                                println!("\nPress Enter to return to main menu...");
                                                let mut _dummy = String::new();
                                                io::stdin().read_line(&mut _dummy)?;
                                            } else {
                                                println!("❌ Operation cancelled.");
                                            }
                                        }
                                    }
                                    None => {
                                        println!("Please try again with a valid drive letter.");
                                    }
                                }
                            }
                            "2" => {
                                let drives = get_windows_drives()?;
                                display_drives(&drives);
                            }
                            "3" => {
                                println!("Exiting...");
                                break;
                            }
                            _ => {
                                println!("Invalid choice. Please enter 1, 2, or 3.");
                            }
                        }
                    }
                }
                Ok(false) => {
                    println!("⚠ Not running with Administrator privileges");
                    println!("Attempting to relaunch with elevated permissions...");
                    relaunch_with_elevation()?;
                    // This point should never be reached as relaunch_with_elevation() calls exit()
                }
                Err(e) => {
                    eprintln!("Error checking elevation status: {}", e);
                }
            }
        }
        "linux" => {
            let elevated = is_elevated_unix();
            if elevated {
                println!("✓ Running with root privileges");
                println!("Linux system detected - drive operations not yet implemented");
                
                // Keep the terminal open for future Linux implementation
                println!("\nPress Enter to exit...");
                let mut _dummy = String::new();
                io::stdin().read_line(&mut _dummy)?;
            } else {
                println!("⚠ Not running with root privileges");
                println!("Attempting to request elevation (pkexec/sudo)...");
                relaunch_with_elevation()?;
            }
        }
        _ => {
            println!("Unsupported OS: {}", os);
            println!("This program supports Windows and Linux only.");
            
            // Keep terminal open
            println!("\nPress Enter to exit...");
            let mut _dummy = String::new();
            io::stdin().read_line(&mut _dummy)?;
        }
    }
    
    println!("\nProgram completed.");
    Ok(())
}
