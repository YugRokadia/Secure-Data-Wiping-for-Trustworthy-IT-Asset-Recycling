
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
        // Let's try a much simpler approach - just run the drive listing directly
        // and let the user see it in a separate elevated window
        println!("Opening elevated PowerShell window to show drive information...");
        println!("Please check the new PowerShell window that should appear.");
        
        let ps_command = r#"
Write-Host '=== OS Elevation Demo ===' -ForegroundColor Green
Write-Host 'Detected OS: windows' -ForegroundColor Yellow
Write-Host '✓ Running with Administrator privileges' -ForegroundColor Green
Write-Host ''

try {
    $drives = Get-PhysicalDisk | Select-Object DeviceID, MediaType, FriendlyName, Size
    $volumes = Get-Volume | Select-Object DriveLetter, FileSystemLabel, FileSystem, BitLockerProtectionStatus, Path
    $partitions = Get-Partition | Select-Object DiskNumber, PartitionNumber, DriveLetter, Size

    Write-Host '=== Windows Drives and Partitions ===' -ForegroundColor Cyan
    Write-Host ''
    
    $result = @()
    foreach ($drive in $drives) {
        $driveInfo = @{
            DeviceID = $drive.DeviceID
            MediaType = $drive.MediaType
            FriendlyName = $drive.FriendlyName
            Size = $drive.Size
            Volumes = @()
        }
        foreach ($vol in $volumes) {
            if ($vol.DriveLetter) {
                $partInfo = $partitions | Where-Object { $_.DriveLetter -eq $vol.DriveLetter }
                $driveInfo.Volumes += @{
                    DriveLetter = $vol.DriveLetter
                    Label = $vol.FileSystemLabel
                    FileSystem = $vol.FileSystem
                    BitLocker = if ($vol.BitLockerProtectionStatus -eq 1) { 'Enabled' } else { 'Disabled' }
                    PartitionNumber = $partInfo.PartitionNumber
                    PartitionSize = $partInfo.Size
                }
            }
        }
        $result += $driveInfo
    }

    foreach ($drive in $result) {
        $device_id = $drive.DeviceID
        $media_type = $drive.MediaType
        $friendly_name = $drive.FriendlyName
        $size = $drive.Size
        $sizeGB = [math]::Round($size / 1GB, 2)
        
        Write-Host ''
        Write-Host "Drive $device_id : $friendly_name ($media_type)" -ForegroundColor White
        Write-Host "  Total Size: $size bytes ($sizeGB GB)" -ForegroundColor Gray
        
        if ($drive.Volumes.Count -eq 0) {
            Write-Host '  No mounted volumes found.' -ForegroundColor Red
        } else {
            foreach ($vol in $drive.Volumes) {
                $letter = $vol.DriveLetter
                $fs = $vol.FileSystem
                $label = $vol.Label
                $bitlocker = $vol.BitLocker
                $part_num = $vol.PartitionNumber
                $part_size = $vol.PartitionSize
                $part_sizeGB = [math]::Round($part_size / 1GB, 2)
                
                Write-Host "  Partition $part_num : $letter : ($label)" -ForegroundColor Yellow
                Write-Host "    File System: $fs" -ForegroundColor Gray
                Write-Host "    Size: $part_size bytes ($part_sizeGB GB)" -ForegroundColor Gray
                $bitlockerColor = if($bitlocker -eq 'Enabled') { 'Red' } else { 'Green' }
                Write-Host "    BitLocker: $bitlocker" -ForegroundColor $bitlockerColor
            }
        }
    }
    Write-Host ''
    Write-Host 'Program completed successfully!' -ForegroundColor Green
} catch {
    Write-Host "Error occurred: $($_.Exception.Message)" -ForegroundColor Red
}

Write-Host ''
Write-Host 'Press any key to close this window...' -ForegroundColor Yellow
$null = $Host.UI.RawUI.ReadKey('NoEcho,IncludeKeyDown')
"#;

        // Create a PowerShell script file
        let temp_ps1 = std::env::temp_dir().join("show_drives.ps1");
        std::fs::write(&temp_ps1, ps_command)?;
        
        // Create a batch file that will run the PowerShell script elevated
        let temp_bat = std::env::temp_dir().join("run_elevated.bat");
        let batch_content = format!(
            "@echo off\n\
            powershell -NoProfile -ExecutionPolicy Bypass -File \"{}\"\n\
            pause",
            temp_ps1.to_string_lossy()
        );
        std::fs::write(&temp_bat, batch_content)?;
        
        // Run the batch file elevated
        let _output = Command::new("powershell")
            .arg("-NoProfile")
            .arg("-Command")
            .arg(format!("Start-Process -FilePath '{}' -Verb RunAs -Wait", temp_bat.to_string_lossy()))
            .output()?;
            
        // Clean up batch file
        let _ = std::fs::remove_file(&temp_bat);
            
        // Clean up temp script file
        let _ = std::fs::remove_file(&temp_ps1);
        
        Ok(())
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
        .output()?;

    let json = String::from_utf8_lossy(&output.stdout);
    let drives_json: serde_json::Value = serde_json::from_str(&json).unwrap_or(serde_json::Value::Null);

    let mut drives = Vec::new();
    
    if let Some(arr) = drives_json.as_array() {
        for drive_json in arr {
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
    
    let mut available_letters = Vec::new();
    for drive in drives {
        for vol in &drive.volumes {
            if !vol.drive_letter.is_empty() && vol.drive_letter != "-" {
                available_letters.push(vol.drive_letter.clone());
            }
        }
    }
    
    // Remove duplicates and sort
    available_letters.sort();
    available_letters.dedup();
    
    for (i, letter) in available_letters.iter().enumerate() {
        println!("{}. Drive {}", i + 1, letter);
    }
    
    println!("\nEnter the drive letter you want to select (e.g., C, D, E):");
    print!("> ");
    io::stdout().flush()?;
    
    let mut input = String::new();
    io::stdin().read_line(&mut input)?;
    let selected = input.trim().to_uppercase();
    
    if available_letters.contains(&selected) {
        Ok(Some(selected))
    } else {
        println!("Invalid drive letter: {}", selected);
        Ok(None)
    }
}

fn main() -> io::Result<()> {
    println!("=== OS Elevation Demo ===");
    
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
                    
                    // Let user select a drive
                    loop {
                        match select_drive(&drives)? {
                            Some(selected_drive) => {
                                println!("\n✓ You selected drive: {}", selected_drive);
                                
                                // Find the selected drive details
                                for drive in &drives {
                                    for vol in &drive.volumes {
                                        if vol.drive_letter == selected_drive {
                                            println!("\nSelected Drive Details:");
                                            println!("  Physical Drive: {} ({})", drive.friendly_name, drive.media_type);
                                            println!("  Drive Letter: {}", vol.drive_letter);
                                            println!("  Label: {}", vol.label);
                                            println!("  File System: {}", vol.file_system);
                                            println!("  Size: {} bytes ({:.2} GB)", vol.partition_size, vol.partition_size as f64 / 1_000_000_000.0);
                                            println!("  BitLocker: {}", vol.bitlocker);
                                            break;
                                        }
                                    }
                                }
                                
                                println!("\nThis drive is ready for cryptographic erasure.");
                                println!("(Cryptographic erase functionality will be implemented later)");
                                break;
                            }
                            None => {
                                println!("Please try again with a valid drive letter.");
                            }
                        }
                    }
                }
                Ok(false) => {
                    println!("⚠ Not running with Administrator privileges");
                    println!("Attempting to relaunch with elevated permissions...");
                    relaunch_with_elevation()?;
                    println!("Elevation completed. Drive information should have been displayed above.");
                    
                    // After elevation, get drive information and allow selection
                    println!("\nNow getting drive information for selection...");
                    let drives = get_windows_drives()?;
                    display_drives(&drives);
                    
                    // Let user select a drive
                    loop {
                        match select_drive(&drives)? {
                            Some(selected_drive) => {
                                println!("\n✓ You selected drive: {}", selected_drive);
                                
                                // Find the selected drive details
                                for drive in &drives {
                                    for vol in &drive.volumes {
                                        if vol.drive_letter == selected_drive {
                                            println!("\nSelected Drive Details:");
                                            println!("  Physical Drive: {} ({})", drive.friendly_name, drive.media_type);
                                            println!("  Drive Letter: {}", vol.drive_letter);
                                            println!("  Label: {}", vol.label);
                                            println!("  File System: {}", vol.file_system);
                                            println!("  Size: {} bytes ({:.2} GB)", vol.partition_size, vol.partition_size as f64 / 1_000_000_000.0);
                                            println!("  BitLocker: {}", vol.bitlocker);
                                            break;
                                        }
                                    }
                                }
                                
                                println!("\nThis drive is ready for cryptographic erasure.");
                                println!("(Cryptographic erase functionality will be implemented later)");
                                break;
                            }
                            None => {
                                println!("Please try again with a valid drive letter.");
                            }
                        }
                    }
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
                println!("Linux system detected - no drive listing implemented for this demo");
            } else {
                println!("⚠ Not running with root privileges");
                println!("Attempting to request elevation (pkexec/sudo)...");
                relaunch_with_elevation()?;
                println!("Elevation completed.");
            }
        }
        _ => {
            println!("Unsupported OS: {}", os);
            println!("This program supports Windows and Linux only.");
        }
    }
    
    println!("\nProgram completed.");
    Ok(())
}
