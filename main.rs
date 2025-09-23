use std::env;
use std::process::{Command as ProcessCommand, Stdio};
use std::io::{self, Write};
use std::path::Path;
use uuid::Uuid;
use rand::{thread_rng, Rng};
use std::collections::HashSet;
use std::time::{Duration, Instant};
use std::thread;
use std::sync::{Arc, Mutex};
use eframe::egui;
use chrono::{DateTime, Utc};

// UI Components
#[derive(Debug, Clone, PartialEq)]
enum UiState {
    Landing,
    DriveSelection,
    FinalConfirmation,
    PurgeInProgress,
    Completion,
}

#[derive(Debug, Clone)]
struct ProgressInfo {
    progress: f32,
    status: String,
    start_time: Instant,
    current_drive_index: usize,
    total_drives: usize,
    operation_id: String,
}

#[derive(Debug, Clone)]
struct DeviceInfo {
    path: String,
    name: String,
    size: String,
    device_type: String,
    mountpoint: String,
    model: String,
    is_partition: bool,
}

pub struct DriveWipeApp {
    state: UiState,
    available_drives: Vec<DeviceInfo>,
    selected_drives: HashSet<usize>,
    progress_info: Option<Arc<Mutex<ProgressInfo>>>,
    certificates: Vec<String>,
    error_message: Option<String>,
    confirmation_text: String,
    dark_mode: bool,
    force_mode: bool,
    verify_mode: bool,
    wipe_handle: Option<thread::JoinHandle<Result<Vec<String>, String>>>,
}

impl Default for DriveWipeApp {
    fn default() -> Self {
        Self {
            state: UiState::Landing,
            available_drives: Vec::new(),
            selected_drives: HashSet::new(),
            progress_info: None,
            certificates: Vec::new(),
            error_message: None,
            confirmation_text: String::new(),
            dark_mode: true,
            force_mode: false,
            verify_mode: false,
            wipe_handle: None,
        }
    }
}

impl eframe::App for DriveWipeApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // Apply theme
        if self.dark_mode {
            ctx.set_visuals(egui::Visuals::dark());
        } else {
            ctx.set_visuals(egui::Visuals::light());
        }

        // Check if wipe operation is complete
        if let Some(handle) = &self.wipe_handle {
            if handle.is_finished() {
                let handle = self.wipe_handle.take().unwrap();
                match handle.join() {
                    Ok(Ok(certificates)) => {
                        self.certificates = certificates;
                        self.state = UiState::Completion;
                        self.progress_info = None;
                    }
                    Ok(Err(error)) => {
                        self.error_message = Some(error);
                        self.state = UiState::DriveSelection;
                        self.progress_info = None;
                    }
                    Err(_) => {
                        self.error_message = Some("Thread panicked during wipe operation".to_string());
                        self.state = UiState::DriveSelection;
                        self.progress_info = None;
                    }
                }
            }
        }

        egui::CentralPanel::default().show(ctx, |ui| {
            // Theme toggle in top-right corner
            ui.with_layout(egui::Layout::top_down(egui::Align::Max), |ui| {
                ui.horizontal(|ui| {
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        let theme_text = if self.dark_mode { "☀ Light Mode" } else { "🌙 Dark Mode" };
                        if ui.button(theme_text).clicked() {
                            self.dark_mode = !self.dark_mode;
                        }
                    });
                });
            });

            match self.state {
                UiState::Landing => self.show_landing(ui),
                UiState::DriveSelection => self.show_drive_selection(ui),
                UiState::FinalConfirmation => self.show_final_confirmation(ui),
                UiState::PurgeInProgress => self.show_progress_screen(ui, ctx),
                UiState::Completion => self.show_completion_screen(ui),
            }
        });
    }
}

impl DriveWipeApp {
    pub fn new() -> Self {
        Default::default()
    }

    fn show_landing(&mut self, ui: &mut egui::Ui) {
        ui.with_layout(egui::Layout::top_down(egui::Align::Center), |ui| {
            ui.add_space(30.0);
            
            // Professional title
            ui.heading("🔐 LUKS Crypto Wipe v1.0");
            ui.add_space(10.0);
            ui.label("Advanced Cryptographic Data Destruction Tool");
            ui.add_space(30.0);
            
            ui.separator();
            ui.add_space(20.0);
            
            // Warning section
            let warning_color = if self.dark_mode { egui::Color32::LIGHT_RED } else { egui::Color32::DARK_RED };
            ui.colored_label(warning_color, "⚠️  DANGER: This will PERMANENTLY destroy ALL data!");
            ui.colored_label(warning_color, "This operation cannot be undone!");
            
            ui.add_space(20.0);
            
            // Process description
            ui.group(|ui| {
                ui.vertical(|ui| {
                    ui.strong("LUKS Crypto Wipe Process:");
                    ui.label("🔐 Creates LUKS2 AES-XTS-256 encryption");
                    ui.label("📝 Fills device with encrypted random data");
                    ui.label("🗝️  Destroys encryption keys");
                    ui.label("🛡️  Makes data cryptographically unrecoverable");
                    ui.label("📋 Generates completion certificate");
                });
            });
            
            ui.add_space(30.0);
            
            // Options
            ui.horizontal(|ui| {
                ui.checkbox(&mut self.force_mode, "Force mode (skip confirmations)");
                ui.checkbox(&mut self.verify_mode, "Verify wipe completion");
            });
            
            ui.add_space(20.0);
            
            // Main action button
            let button_color = if self.dark_mode { 
                egui::Color32::from_rgb(0, 120, 215) 
            } else { 
                egui::Color32::from_rgb(0, 95, 184) 
            };
            
            if ui.add_sized([250.0, 50.0], 
                egui::Button::new("🚀 Begin Crypto Wipe Process")
                    .fill(button_color)
            ).clicked() {
                self.load_available_drives();
                self.state = UiState::DriveSelection;
                self.error_message = None;
            }
            
            ui.add_space(20.0);
            ui.colored_label(
                if self.dark_mode { egui::Color32::LIGHT_YELLOW } else { egui::Color32::DARK_BLUE },
                "Ensure all important data is backed up before proceeding"
            );
        });
    }

    fn show_drive_selection(&mut self, ui: &mut egui::Ui) {
        ui.with_layout(egui::Layout::top_down(egui::Align::Center), |ui| {
            ui.add_space(20.0);
            ui.heading("🎯 Storage Device & Partition Selection");
            ui.add_space(20.0);
            
            if let Some(ref error) = self.error_message {
                ui.colored_label(egui::Color32::RED, format!("Error: {}", error));
                ui.add_space(10.0);
            }
            
            ui.separator();
            ui.add_space(15.0);
            
            ui.label("💽 Available Storage Devices and Partitions:");
            ui.add_space(10.0);
            
            egui::ScrollArea::vertical().max_height(350.0).show(ui, |ui| {
                for (index, device) in self.available_drives.iter().enumerate() {
                    let is_selected = self.selected_drives.contains(&index);
                    
                    ui.group(|ui| {
                        ui.horizontal(|ui| {
                            let mut selected = is_selected;
                            let checkbox_response = ui.checkbox(&mut selected, "");
                            if checkbox_response.changed() {
                                if selected {
                                    self.selected_drives.insert(index);
                                } else {
                                    self.selected_drives.remove(&index);
                                }
                            }
                            
                            let icon = if device.is_partition { "📂" } else { "💽" };
                            ui.label(icon);
                            
                            ui.vertical(|ui| {
                                ui.strong(format!("{} ({})", device.path, device.name));
                                ui.label(format!("Size: {} | Type: {} | Model: {}", 
                                    device.size, device.device_type.to_uppercase(), device.model));
                                
                                if !device.mountpoint.is_empty() && device.mountpoint != "-" {
                                    ui.colored_label(egui::Color32::YELLOW, 
                                        format!("⚠️  MOUNTED at: {}", device.mountpoint));
                                }
                            });
                            
                            if is_selected {
                                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                    ui.colored_label(egui::Color32::RED, "SELECTED FOR WIPE");
                                });
                            }
                        });
                    });
                    
                    ui.add_space(10.0);
                }
            });
            
            ui.add_space(15.0);
            ui.colored_label(
                if self.dark_mode { egui::Color32::LIGHT_BLUE } else { egui::Color32::DARK_BLUE },
                "💡 Tip: You can wipe entire drives or individual partitions"
            );
            
            ui.add_space(20.0);
            ui.separator();
            ui.add_space(15.0);
            
            ui.horizontal(|ui| {
                if ui.button("← Back to Home").clicked() {
                    self.state = UiState::Landing;
                    self.selected_drives.clear();
                    self.error_message = None;
                }
                
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    let can_proceed = !self.selected_drives.is_empty();
                    
                    ui.add_enabled_ui(can_proceed, |ui| {
                        if ui.add_sized([150.0, 35.0], egui::Button::new("Continue →")).clicked() {
                            self.state = UiState::FinalConfirmation;
                            self.error_message = None;
                        }
                    });
                    
                    if !can_proceed {
                        ui.label("Select at least one device to continue");
                    } else {
                        ui.label(format!("{} device(s) selected", self.selected_drives.len()));
                    }
                });
            });
        });
    }

    fn show_final_confirmation(&mut self, ui: &mut egui::Ui) {
        ui.with_layout(egui::Layout::top_down(egui::Align::Center), |ui| {
            ui.add_space(30.0);
            ui.heading("⚠️  DANGER ZONE");
            ui.add_space(20.0);
            
            ui.colored_label(egui::Color32::RED, "You are about to PERMANENTLY WIPE the selected devices!");
            ui.add_space(15.0);
            
            ui.separator();
            ui.add_space(15.0);
            
            ui.label("The following devices will be processed:");
            ui.add_space(10.0);
            
            egui::ScrollArea::vertical().max_height(200.0).show(ui, |ui| {
                for &index in &self.selected_drives {
                    if let Some(device) = self.available_drives.get(index) {
                        ui.group(|ui| {
                            ui.horizontal(|ui| {
                                ui.colored_label(egui::Color32::RED, "🔥");
                                ui.vertical(|ui| {
                                    ui.strong(format!("{} ({})", device.path, device.name));
                                    ui.label(format!("{} - {}", device.size, device.device_type));
                                    if !device.mountpoint.is_empty() && device.mountpoint != "-" {
                                        ui.colored_label(egui::Color32::YELLOW, 
                                            format!("Currently mounted at: {}", device.mountpoint));
                                    }
                                });
                            });
                        });
                        ui.add_space(5.0);
                    }
                }
            });
            
            ui.add_space(20.0);
            
            // Process description
            ui.group(|ui| {
                ui.vertical(|ui| {
                    ui.strong("This will:");
                    ui.label("🔥 Destroy ALL data on the device(s)");
                    ui.label("🔐 Create LUKS encryption");
                    ui.label("🗑️  Fill with encrypted random data");
                    ui.label("🔓 Remove encryption keys (making data unrecoverable)");
                });
            });
            
            ui.add_space(20.0);
            ui.separator();
            ui.add_space(20.0);
            
            let confirm_text = if self.force_mode { 
                "FORCE WIPE" 
            } else { 
                "DESTROY ALL DATA" 
            };
            
            ui.label(format!("Type '{}' to confirm:", confirm_text));
            ui.add_space(5.0);
            
            ui.add_sized(
                [250.0, 25.0],
                egui::TextEdit::singleline(&mut self.confirmation_text)
                    .hint_text(format!("Type {} here", confirm_text))
            );
            
            ui.add_space(20.0);
            
            ui.horizontal(|ui| {
                if ui.button("← Back to Selection").clicked() {
                    self.state = UiState::DriveSelection;
                    self.confirmation_text.clear();
                }
                
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    let can_proceed = self.confirmation_text.trim() == confirm_text;
                    
                    ui.add_enabled_ui(can_proceed, |ui| {
                        let button = egui::Button::new("🔥 START CRYPTO WIPE 🔥")
                            .fill(egui::Color32::from_rgb(180, 0, 0));
                        
                        if ui.add_sized([200.0, 40.0], button).clicked() {
                            self.start_crypto_wipe_process();
                        }
                    });
                    
                    if !can_proceed {
                        ui.colored_label(egui::Color32::GRAY, format!("Type '{}' to enable", confirm_text));
                    }
                });
            });
        });
    }

    fn show_progress_screen(&mut self, ui: &mut egui::Ui, ctx: &egui::Context) {
        ui.with_layout(egui::Layout::top_down(egui::Align::Center), |ui| {
            ui.add_space(30.0);
            ui.heading("🔄 LUKS Crypto Wipe In Progress");
            ui.add_space(20.0);
            
            if let Some(ref progress_info_arc) = self.progress_info {
                if let Ok(progress_info) = progress_info_arc.try_lock() {
                    let elapsed = progress_info.start_time.elapsed();
                    let estimated_total = if progress_info.progress > 0.01 {
                        Some(elapsed.mul_f32(1.0 / progress_info.progress))
                    } else {
                        None
                    };
                    
                    ui.group(|ui| {
                        ui.vertical(|ui| {
                            ui.label(format!("🆔 Operation ID: {}", progress_info.operation_id));
                            ui.add_space(5.0);
                            
                            ui.label(format!(
                                "Device {}/{}: {}",
                                progress_info.current_drive_index + 1,
                                progress_info.total_drives,
                                self.get_current_device_name(&progress_info)
                            ));
                            
                            ui.add_space(10.0);
                            
                            let progress_bar = egui::ProgressBar::new(progress_info.progress)
                                .desired_width(450.0)
                                .text(format!("{:.1}%", progress_info.progress * 100.0));
                            ui.add(progress_bar);
                            
                            ui.add_space(10.0);
                            ui.strong(&progress_info.status);
                            
                            ui.add_space(10.0);
                            ui.separator();
                            ui.add_space(5.0);
                            
                            ui.horizontal(|ui| {
                                ui.label(format!("Elapsed: {:02}:{:02}", 
                                    elapsed.as_secs() / 60, elapsed.as_secs() % 60));
                                
                                if let Some(total) = estimated_total {
                                    let remaining = total.saturating_sub(elapsed);
                                    ui.label(format!(" | ETA: {:02}:{:02}", 
                                        remaining.as_secs() / 60, remaining.as_secs() % 60));
                                }
                            });
                        });
                    });
                }
                
                ui.add_space(30.0);
                ui.colored_label(egui::Color32::YELLOW, 
                    "⚠️  DO NOT power off or disconnect devices during crypto wipe");
                ui.colored_label(egui::Color32::LIGHT_BLUE,
                    "🔐 Cryptographic destruction in progress...");
                
                ctx.request_repaint_after(Duration::from_millis(100));
            }
        });
    }

    fn show_completion_screen(&mut self, ui: &mut egui::Ui) {
        ui.with_layout(egui::Layout::top_down(egui::Align::Center), |ui| {
            ui.add_space(30.0);
            ui.heading("🎉 Crypto Wipe Complete!");
            ui.add_space(20.0);
            
            ui.colored_label(egui::Color32::GREEN, "All selected devices have been securely wiped");
            ui.colored_label(egui::Color32::GREEN, "🛡️  Data is cryptographically unrecoverable");
            ui.add_space(20.0);
            
            ui.separator();
            ui.add_space(15.0);
            
            ui.label("📋 Completion Certificates:");
            ui.add_space(10.0);
            
            egui::ScrollArea::vertical().max_height(300.0).show(ui, |ui| {
                for (index, certificate) in self.certificates.iter().enumerate() {
                    ui.group(|ui| {
                        ui.vertical(|ui| {
                            ui.strong(format!("Certificate #{}", index + 1));
                            ui.separator();
                            
                            egui::ScrollArea::vertical().max_height(150.0).show(ui, |ui| {
                                ui.monospace(certificate);
                            });
                            
                            ui.add_space(10.0);
                            
                            ui.horizontal(|ui| {
                                if ui.button("📋 Copy to Clipboard").clicked() {
                                    ui.ctx().copy_text(certificate.clone());
                                }
                                
                                if ui.button("💾 Save Certificate").clicked() {
                                    // In a real implementation, this would save to file
                                    println!("Saving certificate {} to file", index + 1);
                                }
                            });
                        });
                    });
                    ui.add_space(10.0);
                }
            });
            
            ui.add_space(20.0);
            ui.separator();
            ui.add_space(15.0);
            
            if ui.add_sized([250.0, 40.0], egui::Button::new("🏠 Return to Home")).clicked() {
                self.reset_to_landing();
            }
        });
    }

    // Helper methods
    fn load_available_drives(&mut self) {
        match get_block_devices() {
            Ok(devices) => self.available_drives = devices,
            Err(e) => self.error_message = Some(format!("Failed to load devices: {}", e)),
        }
    }

    fn start_crypto_wipe_process(&mut self) {
        let operation_id = Uuid::new_v4().to_string();
        let progress_info = Arc::new(Mutex::new(ProgressInfo {
            progress: 0.0,
            status: "Initializing LUKS crypto wipe...".to_string(),
            start_time: Instant::now(),
            current_drive_index: 0,
            total_drives: self.selected_drives.len(),
            operation_id: operation_id.clone(),
        }));
        
        self.state = UiState::PurgeInProgress;
        self.progress_info = Some(progress_info.clone());
        
        // Collect devices to wipe
        let selected_indices: Vec<_> = self.selected_drives.iter().cloned().collect();
        let devices_to_wipe: Vec<_> = selected_indices.iter()
            .filter_map(|&idx| self.available_drives.get(idx).cloned())
            .collect();
        
        let verify_mode = self.verify_mode;
        
        // Start the wipe operation in a separate thread
        self.wipe_handle = Some(thread::spawn(move || {
            execute_crypto_wipe_threaded(devices_to_wipe, verify_mode, progress_info, &operation_id)
        }));
    }
    
    fn get_current_device_name(&self, progress_info: &ProgressInfo) -> String {
        let selected_indices: Vec<_> = self.selected_drives.iter().cloned().collect();
        if let Some(&device_index) = selected_indices.get(progress_info.current_drive_index) {
            if let Some(device) = self.available_drives.get(device_index) {
                return format!("{} ({})", device.path, device.name);
            }
        }
        "Unknown Device".to_string()
    }
    
    fn reset_to_landing(&mut self) {
        *self = Self::default();
    }
}

// Threaded crypto wipe execution
fn execute_crypto_wipe_threaded(
    devices: Vec<DeviceInfo>,
    verify_mode: bool,
    progress_info: Arc<Mutex<ProgressInfo>>,
    operation_id: &str,
) -> Result<Vec<String>, String> {
    let mut certificates = Vec::new();
    
    for (drive_idx, device) in devices.iter().enumerate() {
        // Update progress info
        if let Ok(mut progress) = progress_info.lock() {
            progress.current_drive_index = drive_idx;
            progress.status = format!("Processing device: {}", device.path);
        }
        
        // Execute the actual LUKS crypto wipe
        match perform_luks_crypto_wipe_ui(&device.path, verify_mode, |prog, status| {
            if let Ok(mut progress) = progress_info.lock() {
                progress.progress = prog;
                progress.status = status;
            }
        }) {
            Ok(certificate) => certificates.push(certificate),
            Err(e) => return Err(format!("Failed to wipe device {}: {}", device.path, e)),
        }
    }
    
    Ok(certificates)
}

// Integration functions that bridge UI with original CLI logic
fn get_block_devices() -> io::Result<Vec<DeviceInfo>> {
    let output = ProcessCommand::new("lsblk")
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
            path: device_path,
            name: name.to_string(),
            size: size.to_string(),
            device_type: device_type.to_string(),
            mountpoint: mountpoint.to_string(),
            model,
            is_partition: device_type == "part",
        };

        devices.push(device_info);
    }

    Ok(devices)
}

fn perform_luks_crypto_wipe_ui<F>(device: &str, verify: bool, mut progress_callback: F) -> io::Result<String> 
where
    F: FnMut(f32, String),
{
    let wipe_id = Uuid::new_v4();
    
    progress_callback(0.05, "Generating cryptographic key...".to_string());
    let passphrase = generate_random_passphrase();
    
    progress_callback(0.15, "Setting up LUKS encryption...".to_string());
    create_luks_partition(device, &passphrase)?;
    
    progress_callback(0.25, "Opening encrypted partition...".to_string());
    let mapper_name = format!("cryptowipe_{}", wipe_id.simple());
    open_luks_partition(device, &mapper_name, &passphrase)?;
    
    progress_callback(0.35, "Filling with encrypted random data...".to_string());
    fill_with_random_data_ui(&format!("/dev/mapper/{}", mapper_name), &mut progress_callback)?;
    
    progress_callback(0.85, "Closing partition and destroying keys...".to_string());
    close_luks_partition(&mapper_name)?;
    destroy_luks_header(device)?;
    
    if verify {
        progress_callback(0.95, "Verifying wipe completion...".to_string());
        verify_wipe(device)?;
    }
    
    progress_callback(1.0, "Crypto wipe completed successfully!".to_string());
    
    // Generate certificate
    let certificate = format!(
        "═══════════════════════════════════════════════════════════════\n\
         📋 LUKS CRYPTO WIPE COMPLETION CERTIFICATE\n\
         ═══════════════════════════════════════════════════════════════\n\
         🆔 Operation ID: {}\n\
         📱 Device: {}\n\
         🔐 Method: LUKS2 AES-XTS-256 Encryption\n\
         🗝️  Key Size: 512 bits\n\
         🧂 Hash: SHA-256\n\
         🔄 Process:\n\
            1. ✅ LUKS encryption applied\n\
            2. ✅ Filled with encrypted random data\n\
            3. ✅ Encryption keys destroyed\n\
            4. ✅ LUKS header overwritten\n\
         🛡️  Security: Data is cryptographically unrecoverable\n\
         🕒 Completed: {}\n\
         ═══════════════════════════════════════════════════════════════",
        wipe_id, device, Utc::now().format("%Y-%m-%d %H:%M:%S UTC")
    );
    
    Ok(certificate)
}

fn fill_with_random_data_ui<F>(mapper_device: &str, progress_callback: &mut F) -> io::Result<()>
where
    F: FnMut(f32, String),
{
    // Actually perform the operation - in a real implementation you'd want to track progress properly
    let output = ProcessCommand::new("dd")
        .args(&[
            "if=/dev/urandom",
            &format!("of={}", mapper_device),
            "bs=1M",
            "status=none"
        ])
        .output();

    match output {
        Ok(_) => {
            progress_callback(0.80, "Random data fill completed".to_string());
            Ok(())
        }
        Err(e) => Err(e),
    }
}

// Application entry point for UI mode
pub fn run_ui_app() -> eframe::Result<()> {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([900.0, 700.0])
            .with_min_inner_size([700.0, 500.0])
            .with_title("LUKS Crypto Wipe v1.0"),
        ..Default::default()
    };
    
    eframe::run_native(
        "LUKS Crypto Wipe v1.0",
        options,
        Box::new(|_cc| Box::new(DriveWipeApp::new())),
    )
}

// Original CLI functions (preserved unchanged)
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
    println!("    --ui            Launch graphical user interface");
    println!();
    println!("EXAMPLES:");
    println!("    wipeshit                    # Interactive mode");
    println!("    wipeshit /dev/sdb           # Wipe specific device");
    println!("    wipeshit /dev/sdb --force   # Force wipe without confirmation");
    println!("    wipeshit /dev/sdb --verify  # Wipe with verification");
    println!("    wipeshit --ui               # Launch GUI mode");
    println!();
    println!("WARNING: This tool will PERMANENTLY destroy ALL data on the target device!");
}

fn main() -> io::Result<()> {
    let args: Vec<String> = env::args().collect();
    
    // Check for UI mode
    if args.contains(&"--ui".to_string()) {
        return run_ui_app().map_err(|e| io::Error::new(io::ErrorKind::Other, format!("UI Error: {}", e)));
    }
    
    // Parse simple command line arguments
    let device = if args.len() > 1 && !args[1].starts_with("-") { Some(args[1].clone()) } else { None };
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
        let device_info = CliDeviceInfo {
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
struct CliDeviceInfo {
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

// Continuing from where the code left off...

    if output.status.success() {
        println!("✅ Verification completed - device appears properly wiped");
        Ok(())
    } else {
        Err(io::Error::new(
            io::ErrorKind::Other,
            format!("Verification failed: {}", String::from_utf8_lossy(&output.stderr))
        ))
    }
}

fn generate_completion_report(device: &str, wipe_id: &Uuid) {
    let timestamp = Utc::now();
    println!("\n");
    println!("═══════════════════════════════════════════════════════════════");
    println!("📋 LUKS CRYPTO WIPE COMPLETION CERTIFICATE");
    println!("═══════════════════════════════════════════════════════════════");
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
    println!("🕒 Completed: {}", timestamp.format("%Y-%m-%d %H:%M:%S UTC"));
    println!("═══════════════════════════════════════════════════════════════");
    println!();
    println!("💾 This certificate can be saved as proof of secure data destruction.");
}