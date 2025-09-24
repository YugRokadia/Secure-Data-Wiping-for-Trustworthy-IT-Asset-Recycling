use eframe::egui;
use std::time::{Duration, Instant};
use std::sync::mpsc;
use crate::core;

#[derive(Debug, Clone)]
enum ProgressMessage {
    Progress(f32, String),
    Certificate(String),
    Error(String),
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
    available_drives: Vec<core::DeviceInfo>,
    selected_drive: Option<usize>,
    progress_info: Option<ProgressInfo>,
    progress_receiver: Option<mpsc::Receiver<ProgressMessage>>,
    certificates: Vec<String>,
    completion_certificate: Option<String>,
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
            selected_drive: None,
            progress_info: None,
            progress_receiver: None,
            certificates: Vec::new(),
            completion_certificate: None,
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

        // Check for progress updates
        if let Some(ref rx) = self.progress_receiver {
            if let Ok(message) = rx.try_recv() {
                match message {
                    ProgressMessage::Progress(progress, status) => {
                        if let Some(info) = &mut self.progress_info {
                            info.progress = progress;
                            info.status = status.clone();
                        }
                    }
                    ProgressMessage::Certificate(cert) => {
                        self.completion_certificate = Some(cert);
                        self.state = UiState::Completion;
                        self.progress_receiver = None;
                    }
                    ProgressMessage::Error(error_msg) => {
                        self.error_message = Some(error_msg);
                        self.state = UiState::DriveSelection;
                        self.progress_receiver = None;
                    }
                }
            }
        }

        egui::CentralPanel::default().show(ctx, |ui| {
            // Theme toggle button at top
            ui.with_layout(egui::Layout::top_down(egui::Align::Max), |ui| {
                let theme_text = if self.dark_mode { "🌙" } else { "☀️" };
                if ui.button(theme_text).clicked() {
                    self.dark_mode = !self.dark_mode;
                }
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
        // Refresh available drives list on startup
        let mut app = Self::default();
        if let Ok(drives) = core::list_block_devices() {
            app.available_drives = drives;
        }
        app
    }

    fn show_device_entry(ui: &mut egui::Ui, index: usize, drive: &core::DeviceInfo, selected_drive: &mut Option<usize>) {
        ui.horizontal(|ui| {
            if ui.radio_value(selected_drive, Some(index), "").clicked() {
                // Radio buttons automatically handle exclusive selection
            }

            let device_icon = if drive.is_removable {
                "📱"
            } else if drive.is_partition {
                "💿"
            } else {
                "💾"
            };

            let label_text = format!(
                "{} {} ({}) - {} {}{}",
                device_icon,
                drive.path,
                drive.size,
                drive.device_type,
                if !drive.model.is_empty() && drive.model != "Unknown" {
                    format!("- {}", drive.model)
                } else {
                    String::new()
                },
                if !drive.mountpoint.is_empty() && drive.mountpoint != "-" {
                    format!(" [Mounted: {}]", drive.mountpoint)
                } else {
                    String::new()
                }
            );

            let label_color = if drive.is_removable {
                egui::Color32::LIGHT_BLUE
            } else {
                ui.style().visuals.text_color()
            };

            ui.colored_label(label_color, label_text);
        });
    }

    fn show_landing(&mut self, ui: &mut egui::Ui) {
        ui.with_layout(egui::Layout::top_down(egui::Align::Center), |ui| {
            ui.add_space(30.0);
            
            // Title
            ui.heading("LUKS Crypto Wipe v1.0");
            ui.add_space(10.0);
            ui.label("Advanced Cryptographic Data Destruction Tool");
            ui.add_space(30.0);
            
            ui.separator();
            ui.add_space(20.0);
            
            // Warning
            let warning_color = if self.dark_mode {
                egui::Color32::LIGHT_RED
            } else {
                egui::Color32::from_rgb(180, 0, 0)
            };
            ui.colored_label(warning_color, "⚠️ WARNING: This will PERMANENTLY destroy ALL data!");
            ui.colored_label(warning_color, "This operation cannot be undone!");
            
            ui.add_space(20.0);
            
            // Process description
            ui.group(|ui| {
                ui.label("Secure Data Destruction Process:");
                ui.label("1. Create LUKS2 encryption container");
                ui.label("2. Fill with cryptographically secure random data");
                ui.label("3. Destroy encryption keys");
                ui.label("4. Overwrite LUKS headers");
            });
            
            ui.add_space(30.0);
            ui.add_space(20.0);
            
            // Start button
            let button_color = if self.dark_mode {
                egui::Color32::from_rgb(0, 120, 215)
            } else {
                egui::Color32::from_rgb(0, 90, 180)
            };
            
            if ui.add_sized(
                [250.0, 50.0],
                egui::Button::new("Select Devices to Wipe")
                    .fill(button_color)
            ).clicked() {
                // Refresh device list
                if let Ok(drives) = core::list_block_devices() {
                    self.available_drives = drives;
                    self.state = UiState::DriveSelection;
                } else {
                    self.error_message = Some("Failed to list available devices".to_string());
                }
            }
            
            ui.add_space(20.0);
        });
    }

    fn show_drive_selection(&mut self, ui: &mut egui::Ui) {
        ui.add_space(20.0);
        ui.heading("Storage Device & Partition Selection");
        ui.add_space(20.0);
        
        if let Some(ref error) = self.error_message {
            ui.colored_label(egui::Color32::RED, error);
            ui.add_space(10.0);
        }
        
        ui.separator();
        ui.add_space(15.0);
        
        // Device list with categories
        egui::ScrollArea::vertical().max_height(350.0).show(ui, |ui| {
            // Display devices directly without storing them
            let mut has_removable = false;
            let mut selected = self.selected_drive;
            
            // First show removable devices
            ui.colored_label(egui::Color32::LIGHT_BLUE, "📱 Removable Devices:");
            ui.add_space(5.0);
            for (i, drive) in self.available_drives.iter().enumerate() {
                if drive.is_removable {
                    has_removable = true;
                    DriveWipeApp::show_device_entry(ui, i, drive, &mut selected);
                }
            }
            
            if !has_removable {
                ui.label("  No removable devices found");
            }
            
            ui.add_space(10.0);
            
            // Then show fixed devices
            ui.colored_label(egui::Color32::LIGHT_GREEN, "💾 Fixed Storage Devices:");
            ui.add_space(5.0);
            for (i, drive) in self.available_drives.iter().enumerate() {
                if !drive.is_removable {
                    DriveWipeApp::show_device_entry(ui, i, drive, &mut selected);
                }
            }
            
            self.selected_drive = selected;
        });
        
        ui.add_space(15.0);
        ui.colored_label(
            egui::Color32::LIGHT_YELLOW,
            "⚠️ Warning: Mounted devices will be automatically unmounted"
        );
        
        ui.add_space(20.0);
        ui.separator();
        ui.add_space(15.0);
        
        ui.horizontal(|ui| {
            if ui.button("↺ Refresh List").clicked() {
                if let Ok(drives) = core::list_block_devices() {
                    self.available_drives = drives;
                }
            }
            
            ui.add_space(20.0);
            
            if ui.button("← Back").clicked() {
                self.state = UiState::Landing;
            }
            
            if self.selected_drive.is_some() {
                ui.add_space(20.0);
                let proceed_btn = ui.add_sized(
                    [120.0, 30.0],
                    egui::Button::new("Next →")
                        .fill(egui::Color32::from_rgb(0, 120, 215))
                );
                
                if proceed_btn.clicked() {
                    self.state = UiState::FinalConfirmation;
                }
            }
        });
    }

    fn show_final_confirmation(&mut self, ui: &mut egui::Ui) {
        ui.with_layout(egui::Layout::top_down(egui::Align::Center), |ui| {
            ui.add_space(30.0);
            ui.heading("⚠️ DANGER ZONE ⚠️");
            ui.add_space(20.0);
            
            ui.colored_label(
                egui::Color32::RED,
                "You are about to PERMANENTLY WIPE the selected devices!"
            );
            ui.add_space(15.0);
            
            ui.separator();
            ui.add_space(15.0);
            
            ui.label("The following devices will be processed:");
            ui.add_space(10.0);
            
            egui::ScrollArea::vertical().max_height(200.0).show(ui, |ui| {
                if let Some(idx) = self.selected_drive {
                    if let Some(drive) = self.available_drives.get(idx) {
                        let icon = if drive.is_removable { "📱" } else { "💾" };
                        ui.colored_label(
                            egui::Color32::LIGHT_RED,
                            format!("{} {} ({}) - {}", icon, drive.path, drive.size, drive.device_type)
                        );
                    }
                }
            });
            
            ui.add_space(20.0);
            
            // Process description
            ui.group(|ui| {
                ui.colored_label(egui::Color32::LIGHT_YELLOW, "This process will:");
                ui.label("1. Automatically unmount any mounted devices");
                ui.label("2. Create LUKS2 encryption container");
                ui.label("3. Fill with cryptographically secure random data");
                ui.label("4. Destroy encryption keys");
                ui.label("5. Overwrite LUKS headers");
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
            
            let _ = ui.add_sized(
                [200.0, 30.0],
                egui::TextEdit::singleline(&mut self.confirmation_text)
                    .hint_text(confirm_text)
            );
            
            ui.add_space(20.0);
            
            ui.horizontal(|ui| {
                if ui.button("← Back").clicked() {
                    self.state = UiState::DriveSelection;
                    self.confirmation_text.clear();
                }
                
                ui.add_space(20.0);
                
                let proceed_btn = ui.add_sized(
                    [120.0, 30.0],
                    egui::Button::new("Start Wipe →")
                        .fill(egui::Color32::RED)
                );
                
                let can_proceed = self.force_mode || self.confirmation_text == confirm_text;
                if proceed_btn.clicked() && can_proceed {
                    self.state = UiState::InitializingWipe;
                    self.confirmation_text.clear();
                }
            });
        });
    }

    fn show_initializing_screen(&mut self, ui: &mut egui::Ui, ctx: &egui::Context) {
        ui.add_space(50.0);
        
        ui.heading("Initializing Crypto Wipe...");
        ui.add_space(30.0);
        
        // Simple progress spinner
        let time = ctx.input(|i| i.time) as f32;
        let progress = (time.sin() + 1.0) / 2.0;
        
        ui.add(egui::ProgressBar::new(progress).animate(true));
        
        ui.add_space(30.0);
        ui.label("Setting up cryptographic environment");
        ui.label("Validating selected devices");
        ui.label("Preparing for secure wipe");
        
        ui.add_space(20.0);
        ui.colored_label(egui::Color32::LIGHT_BLUE, "Please wait...");
        
        ctx.request_repaint_after(Duration::from_millis(100));
        
        if time > 3.0 {
            self.start_crypto_wipe_process();
        }
    }

    fn show_progress_screen(&mut self, ui: &mut egui::Ui, ctx: &egui::Context) {
        ui.add_space(30.0);
        ui.heading("🔐 LUKS CRYPTO WIPE IN PROGRESS");
        ui.add_space(30.0);
        
        if let Some(ref info) = self.progress_info {
            // Overall progress
            let total_progress = (info.current_drive_index as f32 + info.progress) 
                / info.total_drives as f32;
            
            ui.add_space(10.0);
            ui.label(format!(
                "Processing device {} of {}", 
                info.current_drive_index + 1,
                info.total_drives
            ));
            
            // Current device name
            let current_device = self.get_current_device_name();
            ui.colored_label(
                egui::Color32::LIGHT_BLUE,
                format!("Current device: {}", current_device)
            );
            
            ui.add_space(20.0);
            
            // Progress bars
            ui.label("Overall Progress:");
            ui.add(
                egui::ProgressBar::new(total_progress)
                    .show_percentage()
                    .animate(true)
            );
            
            ui.add_space(10.0);
                ui.add_space(10.0);
            
            ui.add_space(20.0);
            
            // Status message
            ui.colored_label(
                egui::Color32::LIGHT_GREEN,
                &info.status
            );
            
            // Operation details
            ui.add_space(20.0);
            ui.group(|ui| {
                ui.label(format!("Operation ID: {}", info.operation_id));
                let elapsed = info.start_time.elapsed();
                ui.label(format!(
                    "Elapsed Time: {:02}:{:02}:{:02}",
                    elapsed.as_secs() / 3600,
                    (elapsed.as_secs() % 3600) / 60,
                    elapsed.as_secs() % 60
                ));
            });
        }
        
        ctx.request_repaint_after(Duration::from_millis(100));
    }

    fn show_completion_screen(&mut self, ui: &mut egui::Ui) {
        ui.with_layout(egui::Layout::top_down(egui::Align::Center), |ui| {
            ui.add_space(30.0);
            ui.heading("✅ Operation Complete");
            ui.add_space(20.0);
            
            ui.colored_label(
                egui::Color32::LIGHT_GREEN,
                "Device has been securely wiped!"
            );
            
            ui.add_space(30.0);
            
            // Show completion certificate
            if let Some(ref cert) = self.completion_certificate {
                ui.label("Completion Certificate:");
                ui.add_space(10.0);
                
                egui::ScrollArea::vertical()
                    .max_height(300.0)
                    .show(ui, |ui| {
                        ui.group(|ui| {
                            ui.label(cert);
                        });
                    });
            } else {
                ui.colored_label(
                    egui::Color32::YELLOW,
                    "Certificate not available"
                );
            }
            
            ui.add_space(20.0);
            ui.separator();
            ui.add_space(20.0);
            
            if ui.button("Start New Operation").clicked() {
                self.reset_to_landing();
            }
        });
    }

    fn start_crypto_wipe_process(&mut self) {
        let operation_id = format!("LUKS-{:08X}", rand::random::<u32>());
        self.state = UiState::PurgeInProgress;
        self.progress_info = Some(ProgressInfo {
            progress: 0.0,
            status: "Initializing...".to_string(),
            start_time: Instant::now(),
            current_drive_index: 0,
            total_drives: 1, // Only one drive can be selected
            operation_id: operation_id.clone(),
        });
        
        self.execute_mock_crypto_wipe();
    }
    
    fn execute_mock_crypto_wipe(&mut self) {
        if let Some(idx) = self.selected_drive {
            if let Some(drive) = self.available_drives.get(idx) {
                let (tx, rx) = mpsc::channel();
                self.progress_receiver = Some(rx);
                
                let device_path = drive.path.clone();
                let verify = self.verify_mode;
                
                println!("Starting wipe process for device: {}", device_path);
                
                // Start crypto wipe in a separate thread
                std::thread::spawn(move || {
                    println!("Thread started for wiping: {}", device_path);
                    let sender = tx.clone();
                    
                    // Send initial progress update
                    let _ = sender.send(ProgressMessage::Progress(0.0, "Starting wipe process...".to_string()));
                    
                    match core::perform_luks_crypto_wipe(
                        &device_path,
                        verify,
                        move |progress, status| {
                            println!("Progress: {:.1}% - {}", progress * 100.0, status);
                            let _ = sender.send(ProgressMessage::Progress(progress, status));
                        },
                    ) {
                        Ok(cert) => {
                            println!("Successfully wiped {}: {}", device_path, cert);
                            let _ = tx.send(ProgressMessage::Certificate(cert));
                        }
                        Err(e) => {
                            eprintln!("Failed to wipe {}: {}", device_path, e);
                            let _ = tx.send(ProgressMessage::Error(format!("Wipe failed: {}", e)));
                        }
                    }
                    println!("Thread completed for: {}", device_path);
                });
                
            } else {
                println!("Error: No drive found at selected index");
                self.error_message = Some("Selected drive not found".to_string());
                self.state = UiState::DriveSelection;
            }
        } else {
            println!("Error: No drive selected");
            self.error_message = Some("No drive selected".to_string());
            self.state = UiState::DriveSelection;
        }
    }
    
    fn get_current_device_name(&self) -> String {
        if let Some(idx) = self.selected_drive {
            if let Some(device) = self.available_drives.get(idx) {
                return device.path.clone();
            }
        }
        "Unknown Device".to_string()
    }
    
    fn reset_to_landing(&mut self) {
        *self = Self::default();
    }
}

pub fn run_ui() -> eframe::Result<()> {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([900.0, 700.0])
            .with_min_inner_size([600.0, 400.0])
            .with_resizable(true)
            .with_maximized(false)
            .with_fullscreen(false)
            .with_maximize_button(true)
            .with_title("LUKS Crypto Wipe v1.0"),
        ..Default::default()
    };
    eframe::run_native(
        "LUKS Crypto Wipe v1.0",
        options,
        Box::new(|_cc| Ok(Box::new(DriveWipeApp::new()))),
    )
}