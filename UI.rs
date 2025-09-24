use eframe::egui;
use std::collections::HashSet;
use std::time::{Duration, Instant};

// Mock purge module for standalone demo
mod purge {
    use std::path::PathBuf;
    use std::time::Duration;
    
    #[derive(Clone, Debug)]
    pub struct DriveInfo {
        pub path: PathBuf,
        pub name: String,
        pub size_gb: u64,
        pub mount_point: String,
    }
    
    pub fn get_available_drives() -> Vec<DriveInfo> {
        // Mock implementation with realistic test data
        vec![
            DriveInfo {
                path: "/dev/sda".into(),
                name: "Samsung SSD 970 EVO Plus".to_string(),
                size_gb: 500,
                mount_point: "/".to_string(),
            },
            DriveInfo {
                path: "/dev/sdb".into(),
                name: "SanDisk Ultra USB 3.0".to_string(),
                size_gb: 64,
                mount_point: "/mnt/usb".to_string(),
            },
            DriveInfo {
                path: "/dev/sdc".into(),
                name: "WD Blue HDD".to_string(),
                size_gb: 1000,
                mount_point: "-".to_string(),
            },
            DriveInfo {
                path: "/dev/nvme0n1".into(),
                name: "Intel SSD 660p Series".to_string(),
                size_gb: 256,
                mount_point: "-".to_string(),
            },
        ]
    }
    
    pub fn wipe_drive(drive: &DriveInfo, mut progress_callback: impl FnMut(f32, String)) -> Result<String, String> {
        // Mock implementation with realistic progression
        use std::thread;
        
        for i in 0..=100 {
            thread::sleep(Duration::from_millis(50));
            let progress = i as f32 / 100.0;
            let status = match i {
                0..=10 => "Generating cryptographic key...".to_string(),
                11..=25 => "Setting up LUKS encryption...".to_string(),
                26..=35 => "Opening encrypted partition...".to_string(),
                36..=85 => "Filling with encrypted random data...".to_string(),
                86..=95 => "Closing partition and destroying keys...".to_string(),
                _ => "Finalizing crypto wipe...".to_string(),
            };
            progress_callback(progress, status);
        }
        
        // Return realistic certificate content
        let timestamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs();
        let operation_id = format!("LUKS-{:08X}", timestamp as u32);
        Ok(format!(
            "LUKS CRYPTO WIPE COMPLETION CERTIFICATE\n\
             Operation ID: {}\n\
             Device: {} ({})\n\
             Size: {} GB\n\
             Method: LUKS2 AES-XTS-256 Encryption\n\
             Key Size: 512 bits\n\
             Hash: SHA-256\n\
             Process:\n\
               1. LUKS encryption applied\n\
               2. Filled with encrypted random data\n\
               3. Encryption keys destroyed\n\
               4. LUKS header overwritten\n\
             Security: Data is cryptographically unrecoverable\n\
             Completion Time: {}\n\
             Status: VERIFIED SECURE",
            operation_id, drive.path.display(), drive.name, drive.size_gb,
            chrono::Utc::now().format("%Y-%m-%d %H:%M:%S UTC")
        ))
    }
}

#[derive(Debug, Clone, PartialEq)]
enum UiState {
    Landing,
    DriveSelection,
    FinalConfirmation,
    InitializingWipe,
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

pub struct DriveWipeApp {
    state: UiState,
    available_drives: Vec<purge::DriveInfo>,
    selected_drives: HashSet<usize>,
    progress_info: Option<ProgressInfo>,
    certificates: Vec<String>,
    error_message: Option<String>,
    confirmation_text: String,
    dark_mode: bool,
    force_mode: bool,
    verify_mode: bool,
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

        egui::CentralPanel::default().show(ctx, |ui| {
            // Simple theme toggle button at top
            ui.with_layout(egui::Layout::top_down(egui::Align::Max), |ui| {
                ui.horizontal(|ui| {
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui.button(if self.dark_mode { "🌞" } else { "🌙" }).clicked() {
                            self.dark_mode = !self.dark_mode;
                        }
                    });
                });
            });

            match self.state {
                UiState::Landing => self.show_landing(ui),
                UiState::DriveSelection => self.show_drive_selection(ui),
                UiState::FinalConfirmation => self.show_final_confirmation(ui),
                UiState::InitializingWipe => self.show_initializing_screen(ui, ctx),
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
            ui.heading("LUKS Crypto Wipe v1.0");
            ui.add_space(10.0);
            ui.label("Advanced Cryptographic Data Destruction Tool");
            ui.add_space(30.0);
            
            ui.separator();
            ui.add_space(20.0);
            
            // Warning section
            let warning_color = if self.dark_mode { egui::Color32::LIGHT_RED } else { egui::Color32::from_rgb(180, 0, 0) };
            ui.colored_label(warning_color, "WARNING: This will PERMANENTLY destroy ALL data!");
            ui.colored_label(warning_color, "This operation cannot be undone!");
            
            ui.add_space(20.0);
            
            // Process description
            ui.group(|ui| {
                ui.vertical(|ui| {
                    ui.strong("LUKS Crypto Wipe Process:");
                    ui.label("Creates LUKS2 AES-XTS-256 encryption");
                    ui.label("Fills device with encrypted random data");
                    ui.label("Destroys encryption keys");
                    ui.label("Makes data cryptographically unrecoverable");
                    ui.label("Generates completion certificate");
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
                egui::Button::new("Begin Crypto Wipe Process")
                    .fill(button_color)
            ).clicked() {
                self.available_drives = purge::get_available_drives();
                self.state = UiState::DriveSelection;
                self.error_message = None;
            }
            
            ui.add_space(20.0);
            ui.colored_label(
                if self.dark_mode { egui::Color32::LIGHT_YELLOW } else { egui::Color32::from_rgb(0, 60, 120) },
                "Ensure all important data is backed up before proceeding"
            );
        });
    }

    fn show_drive_selection(&mut self, ui: &mut egui::Ui) {
        ui.with_layout(egui::Layout::top_down(egui::Align::Center), |ui| {
            ui.add_space(20.0);
            ui.heading("Storage Device & Partition Selection");
            ui.add_space(20.0);
            
            if let Some(ref error) = self.error_message {
                ui.colored_label(egui::Color32::RED, format!("Error: {}", error));
                ui.add_space(10.0);
            }
            
            ui.separator();
            ui.add_space(15.0);
            
            ui.label("Available Storage Devices:");
            ui.add_space(10.0);
            
            egui::ScrollArea::vertical().max_height(350.0).show(ui, |ui| {
                for (index, drive) in self.available_drives.iter().enumerate() {
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
                            
                            ui.vertical(|ui| {
                                ui.strong(format!("{} ({})", drive.path.display(), drive.name));
                                ui.label(format!("Size: {} GB", drive.size_gb));
                                
                                if drive.mount_point != "-" {
                                    ui.colored_label(
                                        if self.dark_mode { egui::Color32::YELLOW } else { egui::Color32::from_rgb(180, 120, 0) }, 
                                        format!("MOUNTED at: {}", drive.mount_point)
                                    );
                                } else {
                                    ui.colored_label(
                                        if self.dark_mode { egui::Color32::LIGHT_GREEN } else { egui::Color32::from_rgb(0, 120, 0) },
                                        "Not mounted"
                                    );
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
                if self.dark_mode { egui::Color32::LIGHT_BLUE } else { egui::Color32::from_rgb(0, 60, 120) },
                "Tip: You can select multiple devices for batch processing"
            );
            
            ui.add_space(20.0);
            ui.separator();
            ui.add_space(15.0);
            
            ui.horizontal(|ui| {
                if ui.button("Back to Home").clicked() {
                    self.state = UiState::Landing;
                    self.selected_drives.clear();
                    self.error_message = None;
                }
                
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    let can_proceed = !self.selected_drives.is_empty();
                    
                    ui.add_enabled_ui(can_proceed, |ui| {
                        if ui.add_sized([150.0, 35.0], egui::Button::new("Continue")).clicked() {
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
            ui.heading("DANGER ZONE");
            ui.add_space(20.0);
            
            ui.colored_label(egui::Color32::RED, "You are about to PERMANENTLY WIPE the selected devices!");
            ui.add_space(15.0);
            
            ui.separator();
            ui.add_space(15.0);
            
            ui.label("The following devices will be processed:");
            ui.add_space(10.0);
            
            egui::ScrollArea::vertical().max_height(200.0).show(ui, |ui| {
                for &index in &self.selected_drives {
                    if let Some(drive) = self.available_drives.get(index) {
                        ui.group(|ui| {
                            ui.horizontal(|ui| {
                                ui.vertical(|ui| {
                                    ui.strong(format!("{} ({})", drive.path.display(), drive.name));
                                    ui.label(format!("{} GB", drive.size_gb));
                                    if drive.mount_point != "-" {
                                        ui.colored_label(
                                            if self.dark_mode { egui::Color32::YELLOW } else { egui::Color32::from_rgb(180, 120, 0) },
                                            format!("Currently mounted at: {}", drive.mount_point)
                                        );
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
                    ui.label("Destroy ALL data on the device(s)");
                    ui.label("Create LUKS encryption");
                    ui.label("Fill with encrypted random data");
                    ui.label("Remove encryption keys (making data unrecoverable)");
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
                if ui.button("Back to Selection").clicked() {
                    self.state = UiState::DriveSelection;
                    self.confirmation_text.clear();
                }
                
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    let can_proceed = self.confirmation_text.trim() == confirm_text;
                    
                    ui.add_enabled_ui(can_proceed, |ui| {
                        let button = egui::Button::new("START CRYPTO WIPE")
                            .fill(egui::Color32::from_rgb(180, 0, 0));
                        
                        if ui.add_sized([200.0, 40.0], button).clicked() {
                            self.state = UiState::InitializingWipe;
                            self.confirmation_text.clear();
                        }
                    });
                    
                    if !can_proceed {
                        ui.colored_label(egui::Color32::GRAY, format!("Type '{}' to enable", confirm_text));
                    }
                });
            });
        });
    }

    fn show_initializing_screen(&mut self, ui: &mut egui::Ui, ctx: &egui::Context) {
        ui.with_layout(egui::Layout::top_down(egui::Align::Center), |ui| {
            ui.add_space(50.0);
            
            ui.heading("Initializing Crypto Wipe...");
            ui.add_space(30.0);
            
            // Simple progress spinner effect
            let time = ctx.input(|i| i.time) as f32;
            let progress = (time.sin() + 1.0) / 2.0;
            
            ui.add(egui::ProgressBar::new(progress)
                .desired_width(300.0)
                .text("Preparing secure wipe operation..."));
            
            ui.add_space(30.0);
            ui.label("Setting up cryptographic environment");
            ui.label("Validating selected devices");
            ui.label("Generating security parameters");
            
            ui.add_space(20.0);
            ui.colored_label(egui::Color32::LIGHT_BLUE, "Please wait...");
            
            // Auto-advance after a longer delay so user can see this screen
            ctx.request_repaint_after(Duration::from_millis(100));
            
            // Simulate initialization time (3 seconds instead of 2)
            if time > 3.0 {
                self.start_crypto_wipe_process();
            }
        });
    }

    fn show_progress_screen(&mut self, ui: &mut egui::Ui, ctx: &egui::Context) {
        ui.with_layout(egui::Layout::top_down(egui::Align::Center), |ui| {
            ui.add_space(30.0);
            ui.heading("LUKS CRYPTO WIPE IN PROGRESS");
            ui.add_space(30.0);
            
            // First gather all the data we need
            let (should_show_progress, progress_data) = if let Some(ref mut progress_info) = self.progress_info {
                // Simulate progress over time (this makes the progress bar actually work!)
                let elapsed = progress_info.start_time.elapsed().as_secs_f32();
                let total_duration = 20.0; // 20 seconds total for demo
                progress_info.progress = (elapsed / total_duration).min(1.0);
                
                // Update status based on progress
                progress_info.status = match (progress_info.progress * 100.0) as i32 {
                    0..=10 => "Generating cryptographic keys...".to_string(),
                    11..=25 => "Setting up LUKS encryption...".to_string(),
                    26..=35 => "Opening encrypted partition...".to_string(),
                    36..=85 => format!("Filling with encrypted random data... {:.0}%", progress_info.progress * 100.0),
                    86..=95 => "Closing partition and destroying keys...".to_string(),
                    _ => "Finalizing crypto wipe...".to_string(),
                };
                
                let estimated_total = if progress_info.progress > 0.01 {
                    Some(std::time::Duration::from_secs_f32(total_duration))
                } else {
                    None
                };
                
                // Clone or copy all the data we need for the UI
                let data = (
                    progress_info.operation_id.clone(),
                    progress_info.current_drive_index,
                    progress_info.total_drives,
                    progress_info.progress,
                    progress_info.status.clone(),
                    progress_info.start_time,
                    estimated_total,
                );
                
                (true, Some(data))
            } else {
                (false, None)
            };

            if should_show_progress {
                let (operation_id, current_idx, total_drives, progress, status, start_time, estimated_total) = 
                    progress_data.unwrap();
                
                // Get the device name before the UI closure
                let device_name = self.get_current_device_name();
                
                // Large prominent progress group
                ui.group(|ui| {
                    ui.vertical(|ui| {
                        ui.add_space(15.0);
                        
                        ui.label(format!("Operation ID: {}", operation_id));
                        ui.add_space(10.0);
                        
                        ui.strong(format!(
                            "Device {}/{}: {}",
                            current_idx + 1,
                            total_drives,
                            device_name
                        ));
                        
                        ui.add_space(20.0);
                        
                        // LARGE progress bar
                        let progress_bar = egui::ProgressBar::new(progress)
                            .desired_width(500.0)
                            .desired_height(30.0)
                            .text(format!("{:.1}% COMPLETE", progress * 100.0));
                        ui.add(progress_bar);
                        
                        ui.add_space(15.0);
                        ui.heading(&status);
                        
                        ui.add_space(15.0);
                        ui.separator();
                        ui.add_space(10.0);
                        
                        let elapsed_duration = start_time.elapsed();
                        ui.horizontal(|ui| {
                            ui.label(format!("Elapsed: {:02}:{:02}", 
                                elapsed_duration.as_secs() / 60, elapsed_duration.as_secs() % 60));
                            
                            if let Some(total) = estimated_total {
                                let remaining = total.saturating_sub(elapsed_duration);
                                ui.label(format!(" | ETA: {:02}:{:02}", 
                                    remaining.as_secs() / 60, remaining.as_secs() % 60));
                            }
                        });
                        
                        ui.add_space(15.0);
                    });
                });
                
                ui.add_space(30.0);
                ui.colored_label(
                    if self.dark_mode { egui::Color32::YELLOW } else { egui::Color32::from_rgb(180, 120, 0) }, 
                    "DO NOT power off or disconnect devices during crypto wipe"
                );
                ui.colored_label(
                    if self.dark_mode { egui::Color32::LIGHT_BLUE } else { egui::Color32::from_rgb(0, 60, 120) },
                    "Cryptographic destruction in progress..."
                );
                
                // Check if we're done
                if progress >= 1.0 {
                    // Generate certificate and move to completion
                    let selected_indices: Vec<_> = self.selected_drives.iter().cloned().collect();
                    for &drive_index in &selected_indices {
                        if let Some(drive) = self.available_drives.get(drive_index) {
                            let certificate = format!(
                                "LUKS CRYPTO WIPE COMPLETION CERTIFICATE\n\
                                 Operation ID: {}\n\
                                 Device: {} ({})\n\
                                 Size: {} GB\n\
                                 Method: LUKS2 AES-XTS-256 Encryption\n\
                                 Key Size: 512 bits\n\
                                 Hash: SHA-256\n\
                                 Process:\n\
                                   1. LUKS encryption applied\n\
                                   2. Filled with encrypted random data\n\
                                   3. Encryption keys destroyed\n\
                                   4. LUKS header overwritten\n\
                                 Security: Data is cryptographically unrecoverable\n\
                                 Completion Time: {}\n\
                                 Status: VERIFIED SECURE",
                                operation_id, drive.path.display(), drive.name, drive.size_gb,
                                chrono::Utc::now().format("%Y-%m-%d %H:%M:%S UTC")
                            );
                            self.certificates.push(certificate);
                        }
                    }
                    self.state = UiState::Completion;
                    self.progress_info = None;
                } else {
                    // Continue updating
                    ctx.request_repaint_after(Duration::from_millis(100));
                }
            }
        });
    }

    fn show_completion_screen(&mut self, ui: &mut egui::Ui) {
        ui.with_layout(egui::Layout::top_down(egui::Align::Center), |ui| {
            ui.add_space(30.0);
            ui.heading("Crypto Wipe Complete!");
            ui.add_space(20.0);
            
            ui.colored_label(egui::Color32::GREEN, "All selected devices have been securely wiped");
            ui.colored_label(egui::Color32::GREEN, "Data is cryptographically unrecoverable");
            ui.add_space(20.0);
            
            ui.separator();
            ui.add_space(15.0);
            
            ui.label("Completion Certificates:");
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
                                if ui.button("Copy to Clipboard").clicked() {
                                    ui.ctx().copy_text(certificate.clone());
                                }
                                
                                if ui.button("Save Certificate").clicked() {
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
            
            if ui.add_sized([250.0, 40.0], egui::Button::new("Return to Home")).clicked() {
                self.reset_to_landing();
            }
        });
    }

    // Helper methods
    fn start_crypto_wipe_process(&mut self) {
        let operation_id = format!("LUKS-{:08X}", rand::random::<u32>());
        self.state = UiState::PurgeInProgress;
        self.progress_info = Some(ProgressInfo {
            progress: 0.0,
            status: "Initializing LUKS crypto wipe...".to_string(),
            start_time: Instant::now(),
            current_drive_index: 0,
            total_drives: self.selected_drives.len(),
            operation_id,
        });
        
        // Execute mock crypto wipe process
        self.execute_mock_crypto_wipe();
    }
    
    fn execute_mock_crypto_wipe(&mut self) {
        // Instead of running everything instantly, we'll simulate real-time progress
        // This is a mock implementation that will show visible progress
        
        // For now, just start the first drive and let the UI update loop handle progress
        if !self.selected_drives.is_empty() {
            let selected_indices: Vec<_> = self.selected_drives.iter().cloned().collect();
            if let Some(&first_drive_index) = selected_indices.first() {
                if let Some(_drive) = self.available_drives.get(first_drive_index) {
                    // Start with 0% progress - the UI will simulate progress over time
                    if let Some(ref mut progress_info) = self.progress_info {
                        progress_info.progress = 0.0;
                        progress_info.status = "Starting LUKS encryption setup...".to_string();
                        progress_info.current_drive_index = 0;
                    }
                }
            }
        }
    }
    
    fn get_current_device_name(&self) -> String {
        if let Some(ref progress_info) = self.progress_info {
            let selected_indices: Vec<_> = self.selected_drives.iter().cloned().collect();
            if let Some(&device_index) = selected_indices.get(progress_info.current_drive_index) {
                if let Some(drive) = self.available_drives.get(device_index) {
                    return format!("{} ({})", drive.path.display(), drive.name);
                }
            }
        }
        "Unknown Device".to_string()
    }
    
    fn reset_to_landing(&mut self) {
        *self = Self::default();
    }
}

// Main function to run the standalone preview
fn main() -> eframe::Result<()> {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([900.0, 700.0])
            .with_min_inner_size([700.0, 500.0])
            .with_title("LUKS Crypto Wipe v1.0 - Preview"),
        ..Default::default()
    };
    
    eframe::run_native(
        "LUKS Crypto Wipe v1.0 - Preview",
        options,
        Box::new(|_cc| Ok(Box::new(DriveWipeApp::new()))),
    )
}