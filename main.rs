use std::env;
use std::io::{self, Write};
use std::process::{Command, Stdio};
use std::thread;

fn detect_os() -> &'static str {
    // std::env::consts::OS is compile-time constant, but fine for reporting runtime target
    // For runtime detection of the current OS this is sufficient (it reflects the build target).
    // If you need to detect within a multi-arch binary, you'd use runtime checks (uname, etc).
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
            c.spawn()?;
            return Ok(());
        } else if cmd_exists("sudo") {
            let mut c = Command::new("sudo");
            c.arg(exe_str);
            for a in args {
                c.arg(a);
            }
            c.spawn()?;
            return Ok(());
        } else {
            // No helper found: print an instructive message
            println!("No pkexec or sudo found on your system. Please re-run this program as root (e.g. 'sudo ...').");
            return Ok(());
        }
    }
}

fn main() -> io::Result<()> {
    println!("Starting program on OS: {}", detect_os());
    
    // --- Add this block right here ---
    if detect_os() == "windows" {
        let ps_script = r#"
        $drives = Get-PhysicalDisk | Select-Object DeviceID, MediaType, FriendlyName
        $volumes = Get-Volume | Select-Object DriveLetter, FileSystemLabel, FileSystem, BitLockerProtectionStatus, Path
        $partitions = Get-Partition | Select-Object DiskNumber, PartitionNumber, DriveLetter, Size

        $result = @()
        foreach ($drive in $drives) {
            $driveInfo = @{
                DeviceID = $drive.DeviceID
                MediaType = $drive.MediaType
                FriendlyName = $drive.FriendlyName
                Volumes = @()
            }
            foreach ($vol in $volumes) {
                if ($vol.DriveLetter) {
                    $partInfo = $partitions | Where-Object { $_.DriveLetter -eq $vol.DriveLetter }
                    $driveInfo.Volumes += @{
                        DriveLetter = $vol.DriveLetter
                        Label = $vol.FileSystemLabel
                        FileSystem = $vol.FileSystem
                        BitLocker = if ($vol.BitLockerProtectionStatus -eq 1) { "Enabled" } else { "Disabled" }
                        PartitionNumber = $partInfo.PartitionNumber
                        PartitionSize = $partInfo.Size
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
        let drives: serde_json::Value = serde_json::from_str(&json).unwrap_or(serde_json::Value::Null);

        println!("Drives info:");
        if let Some(arr) = drives.as_array() {
            for drive in arr {
                let dtype = drive.get("MediaType").and_then(|v| v.as_str()).unwrap_or("Unknown");
                let name = drive.get("FriendlyName").and_then(|v| v.as_str()).unwrap_or("Unknown");
                println!("Drive: {} ({})", name, dtype);
                if let Some(vols) = drive.get("Volumes").and_then(|v| v.as_array()) {
                    for vol in vols {
                        let letter = vol.get("DriveLetter").and_then(|v| v.as_str()).unwrap_or("-");
                        let fs = vol.get("FileSystem").and_then(|v| v.as_str()).unwrap_or("-");
                        let label = vol.get("Label").and_then(|v| v.as_str()).unwrap_or("-");
                        let bitlocker = vol.get("BitLocker").and_then(|v| v.as_str()).unwrap_or("-");
                        let part_num = vol.get("PartitionNumber").and_then(|v| v.as_i64()).unwrap_or(-1);
                        let part_size = vol.get("PartitionSize").and_then(|v| v.as_i64()).unwrap_or(0);
                        println!(
                            "  Partition: {} | Label: {} | FS: {} | BitLocker: {} | Partition #: {} | Size: {} bytes",
                            letter, label, fs, bitlocker, part_num, part_size
                        );
                    }
                }
            }
        } else {
            println!("No drive info found.");
        }
    }
    // --- End of block ---
    

    // Spawn a thread for detection and elevation request so main stays responsive.
    let handle = thread::spawn(|| {
        // 1) Determine and print OS
        let os = detect_os();
        println!("Detected OS: {}", os);

        // 2) Check elevated status and try to request elevated permission if not already elevated.
        #[cfg(unix)]
        {
            let elevated = is_elevated_unix();
            println!("Elevated (root) status: {}", elevated);
            if !elevated {
                println!("Not elevated — attempting to request elevation (pkexec/sudo)...");
                if let Err(e) = relaunch_with_elevation() {
                    eprintln!("Failed to request elevation: {}", e);
                } else {
                    println!("Elevation process started. Current instance will continue (or you can exit).");
                }
            } else {
                println!("Already running as root.");
            }
        }

        #[cfg(windows)]
        {
            match is_elevated_windows() {
                Ok(true) => {
                    println!("Already running elevated (Administrator).");
                }
                Ok(false) => {
                    println!("Not elevated — attempting to relaunch elevated via PowerShell...");
                    if let Err(e) = relaunch_with_elevation() {
                        eprintln!("Failed to request elevation: {}", e);
                    } else {
                        println!("Elevation prompt should appear. Current instance will continue (or you can exit).");
                    }
                }
                Err(e) => {
                    eprintln!("Could not reliably determine elevation status: {}", e);
                }
            }
        }

        // Thread work done.
    });

    // Main thread can do other work here.
    println!("Main thread continues to run. Waiting for the detection thread to finish...");
    // Optionally wait:
    if let Err(e) = handle.join() {
        eprintln!("Thread join error: {:?}", e);
    }

    println!("Main exiting.");
    Ok(())
}
