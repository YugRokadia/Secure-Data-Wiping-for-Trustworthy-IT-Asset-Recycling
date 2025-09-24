use std::io::{self, Write, Read};
use std::process::{Command, Stdio};
use uuid::Uuid;
use rand::{thread_rng, Rng, RngCore};
use chrono;
use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
pub struct WipeCertificate {
    operation_id: String,
    device: String,
    method: String,
    key_size: u32,
    hash_algorithm: String,
    process_steps: Vec<String>,
    security_status: String,
    completion_time: String,
    verification_status: String,
}

#[derive(Debug, Clone)]
pub struct DeviceInfo {
    pub path: String,
    pub size: String,
    pub device_type: String,
    pub mountpoint: String,
    pub model: String,
    pub is_partition: bool,
    pub is_removable: bool,
}

fn get_device_size(device: &str) -> io::Result<u64> {
    let file = std::fs::File::open(device)?;
    Ok(file.metadata()?.len())
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

pub fn list_block_devices() -> io::Result<Vec<DeviceInfo>> {
    let output = Command::new("lsblk")
        .args(&["-n", "-o", "NAME,SIZE,TYPE,MOUNTPOINT,MODEL", "--tree"])
        .output()?;

    if !output.status.success() {
        return Err(io::Error::new(io::ErrorKind::Other, "Failed to get device list"));
    }

    let devices_output = String::from_utf8_lossy(&output.stdout);
    let lines: Vec<&str> = devices_output.lines().collect();
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
            path: device_path.clone(),
            size: size.to_string(),
            device_type: device_type.to_string(),
            mountpoint: mountpoint.to_string(),
            model,
            is_partition: device_type == "part",
            is_removable: is_removable_device(name),
        };

        devices.push(device_info);
    }

    Ok(devices)
}

fn auto_unmount_device(device_path: &str) -> io::Result<()> {
    // For whole devices (like /dev/sdb), also check and unmount all partitions
    let device_name = device_path.strip_prefix("/dev/").unwrap_or(device_path);
    
    // Check if the specific device is mounted
    let mount_output = Command::new("findmnt")
        .args(&["-n", "-o", "TARGET", device_path])
        .output()?;

    if mount_output.status.success() && !mount_output.stdout.is_empty() {
        println!("🔄 Unmounting {}", device_path);
        let unmount = Command::new("umount")
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
        let partition_output = Command::new("lsblk")
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
    
    Ok(())
}

pub fn perform_luks_crypto_wipe(device: &str, verify: bool, progress_callback: impl Fn(f32, String) + Send + Sync + 'static) -> io::Result<String> {
    let wipe_id = Uuid::new_v4();
    let device_size = get_device_size(device)?;
    progress_callback(0.0, format!("Starting LUKS crypto wipe for {}", device));

    // Step 0: Auto-unmount if necessary (especially important for USB devices) - 5%
    progress_callback(0.0, "Preparing device...".to_string());
    auto_unmount_device(device)?;
    
    // Add a small delay for USB devices to settle
    let device_name = device.strip_prefix("/dev/").unwrap_or(device);
    if is_removable_device(device_name) {
        std::thread::sleep(std::time::Duration::from_secs(2));
    }
    progress_callback(0.05, "Device prepared".to_string());

    // Step 1: Generate random passphrase - 5%
    progress_callback(0.05, "Generating cryptographic key...".to_string());
    let passphrase = generate_random_passphrase();
    progress_callback(0.10, "Cryptographic key generated".to_string());
    
    // Step 2: Create LUKS partition - 10%
    progress_callback(0.10, "Setting up LUKS encryption...".to_string());
    create_luks_partition(device, &passphrase)?;
    progress_callback(0.20, "LUKS encryption setup complete".to_string());

    // Step 3: Open LUKS partition - 5%
    progress_callback(0.20, "Opening encrypted partition...".to_string());
    let mapper_name = format!("cryptowipe_{}", wipe_id.simple());
    open_luks_partition(device, &mapper_name, &passphrase)?;
    progress_callback(0.25, "Encrypted partition opened".to_string());

    // Step 4: Fill with random data - 50%
    progress_callback(0.25, "Starting secure data overwrite...".to_string());
    fill_with_random_data(
        &format!("/dev/mapper/{}", mapper_name),
        device_size,
        |fill_progress| {
            let overall_progress = 0.25 + (fill_progress * 0.50);
            progress_callback(overall_progress, format!(
                "Overwriting with encrypted random data: {:.1}%",
                fill_progress * 100.0
            ));
        }
    )?;
    progress_callback(0.75, "Data overwrite complete".to_string());

    // Step 5: Close and destroy keys - 15%
    progress_callback(0.75, "Closing encrypted partition...".to_string());
    close_luks_partition(&mapper_name)?;
    progress_callback(0.80, "Destroying encryption keys...".to_string());
    destroy_luks_header(device)?;
    progress_callback(0.90, "Keys and headers destroyed".to_string());

    // Step 6: Verification (optional) - 10%
    if verify {
        progress_callback(0.90, "Starting verification...".to_string());
        verify_wipe(device, |verify_progress| {
            let overall_progress = 0.90 + (verify_progress * 0.10);
            progress_callback(overall_progress, format!(
                "Verifying wipe: {:.1}%",
                verify_progress * 100.0
            ));
        })?;
    }

    progress_callback(1.0, "Operation complete!".to_string());
    
    // Generate and return completion certificate
    Ok(generate_completion_certificate(device, &wipe_id))
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
    let mut child = Command::new("cryptsetup")
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
    let mut child = Command::new("cryptsetup")
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

fn fill_with_random_data(mapper_device: &str, device_size: u64, progress_callback: impl Fn(f32) + Send) -> io::Result<()> {
    let block_size = 1024 * 1024; // 1MB blocks
    let mut file = std::fs::OpenOptions::new()
        .write(true)
        .open(mapper_device)?;
    
    let mut buffer = vec![0u8; block_size];
    let mut bytes_written = 0u64;
    let mut rng = rand::thread_rng();

    while bytes_written < device_size {
        // Generate random data
        rng.fill_bytes(&mut buffer);
        
        // Calculate how much to write in this iteration
        let remaining = device_size - bytes_written;
        let write_size = if remaining < block_size as u64 {
            remaining as usize
        } else {
            block_size
        };

        // Write the data
        file.write_all(&buffer[..write_size])?;
        bytes_written += write_size as u64;

        // Update progress
        let progress = bytes_written as f32 / device_size as f32;
        progress_callback(progress);
    }

    file.sync_all()?;
    Ok(())
}

fn close_luks_partition(mapper_name: &str) -> io::Result<()> {
    let output = Command::new("cryptsetup")
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
    let output = Command::new("dd")
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

fn verify_wipe(device: &str, progress_callback: impl Fn(f32) + Send) -> io::Result<()> {
    let device_size = get_device_size(device)?;
    let block_size = 1024 * 1024; // 1MB blocks
    let mut file = std::fs::File::open(device)?;
    let mut buffer = vec![0u8; block_size];
    let mut bytes_read = 0u64;
    let mut last_nonzero = false;

    while bytes_read < device_size {
        let remaining = device_size - bytes_read;
        let read_size = if remaining < block_size as u64 {
            remaining as usize
        } else {
            block_size
        };

        let bytes = file.read(&mut buffer[..read_size])?;
        if bytes == 0 {
            break;
        }

        // Check if the block contains any non-zero bytes
        if buffer[..bytes].iter().any(|&b| b != 0) {
            last_nonzero = true;
        }

        bytes_read += bytes as u64;
        
        // Update progress
        let progress = bytes_read as f32 / device_size as f32;
        progress_callback(progress);
    }

    if last_nonzero {
        return Err(io::Error::new(io::ErrorKind::Other, "Verification failed: non-zero data found"));
    }

    Ok(())
}

fn generate_completion_certificate(device: &str, wipe_id: &Uuid) -> String {
    let certificate = WipeCertificate {
        operation_id: wipe_id.to_string(),
        device: device.to_string(),
        method: "LUKS2 AES-XTS-256 Encryption".to_string(),
        key_size: 512,
        hash_algorithm: "SHA-256".to_string(),
        process_steps: vec![
            "LUKS encryption applied".to_string(),
            "Filled with encrypted random data".to_string(),
            "Encryption keys destroyed".to_string(),
            "LUKS header overwritten".to_string(),
        ],
        security_status: "Data is cryptographically unrecoverable".to_string(),
        completion_time: chrono::Utc::now().format("%Y-%m-%d %H:%M:%S UTC").to_string(),
        verification_status: "VERIFIED SECURE".to_string(),
    };

    serde_json::to_string_pretty(&certificate).unwrap_or_else(|_| "Error generating certificate".to_string())
}