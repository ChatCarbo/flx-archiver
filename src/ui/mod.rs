use crate::benchmark::{ContentAnalyzer, ContentProfile, LearningEngine, Recommendation};
use crate::compression::{compress, ArchiveFormat, CompressionLevel};
use crate::decompression::{decompress, FormatDetector, ArchiveFormat as DecompFormat, ProgressUpdate};
use egui::{Color32, Context, Frame, Margin, RichText, Stroke, Vec2};
use rfd::FileDialog;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;
use std::time::Instant;
use tokio::sync::mpsc;

// Color palette derived from the FLX Archiver logo (Mauve & Slate Gray)
pub const COLOR_BG: Color32 = Color32::from_rgb(20, 18, 24);         // Dark background with mauve tint
pub const COLOR_SIDEBAR: Color32 = Color32::from_rgb(28, 26, 36);    // Dark violaceous sidebar
pub const COLOR_CARD: Color32 = Color32::from_rgb(45, 42, 56);       // Cards & UI Elements
pub const COLOR_CARD_BG: Color32 = Color32::from_rgb(33, 30, 42);    // Card background
pub const COLOR_ACCENT: Color32 = Color32::from_rgb(157, 107, 158);  // Logo Mauve / Purple
pub const COLOR_TEXT: Color32 = Color32::from_rgb(240, 238, 244);    // Light text
pub const COLOR_MUTED: Color32 = Color32::from_rgb(148, 142, 158);   // Muted Gray/Purple
pub const COLOR_BORDER: Color32 = Color32::from_rgb(94, 101, 114);   // Logo Slate Gray
pub const COLOR_SUCCESS: Color32 = Color32::from_rgb(34, 197, 94);   // Success Green
pub const COLOR_ERROR: Color32 = Color32::from_rgb(239, 68, 68);     // Error Red

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tab {
    Archive,
    Explorer,
    Benchmark,
    Settings,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ViewState {
    DropZone,
    SelectAction,
    ConfigureCompression,
    Progress,
    Finished,
    Error,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BenchmarkRecordEntry {
    #[serde(default)]
    pub format: String,
    #[serde(default)]
    pub level: String,
    #[serde(default)]
    pub original_size_bytes: u64,
    #[serde(default)]
    pub compressed_size_bytes: u64,
    #[serde(default)]
    pub ratio_percent: f32,
    #[serde(default)]
    pub speed_mb_s: f32,
}

pub struct FlxApp {
    active_tab: Tab,
    view_state: ViewState,
    dropped_files: Vec<PathBuf>,
    custom_archive_name: String,
    output_dir: Option<PathBuf>,
    profile: Option<ContentProfile>,
    recommendation: Option<Recommendation>,
    selected_format: ArchiveFormat,
    selected_level: CompressionLevel,
    progress: ProgressUpdate,
    start_time: Option<Instant>,
    progress_rx: Option<mpsc::Receiver<ProgressUpdate>>,
    learning_engine: LearningEngine,
    rt: tokio::runtime::Handle,
    error_msg: String,
}

impl FlxApp {
    pub fn new(cc: &eframe::CreationContext<'_>, rt: tokio::runtime::Handle) -> Self {
        Self::configure_theme(&cc.egui_ctx);
        egui_extras::install_image_loaders(&cc.egui_ctx);
        let learning_engine = LearningEngine::new("benchmark_history.json");

        Self {
            active_tab: Tab::Archive,
            view_state: ViewState::DropZone,
            dropped_files: Vec::new(),
            custom_archive_name: String::new(),
            output_dir: None,
            profile: None,
            recommendation: None,
            selected_format: ArchiveFormat::Zip,
            selected_level: CompressionLevel::Default,
            progress: ProgressUpdate {
                bytes_processed: 0,
                bytes_total: 0,
                current_file: String::new(),
                percentage: 0.0,
            },
            error_msg: String::new(),
            start_time: None,
            progress_rx: None,
            learning_engine,
            rt,
        }
    }

    fn configure_theme(ctx: &Context) {
        let mut visuals = egui::Visuals::dark();
        visuals.panel_fill = COLOR_BG;
        visuals.window_fill = COLOR_BG;
        visuals.extreme_bg_color = COLOR_SIDEBAR;
        visuals.override_text_color = Some(COLOR_TEXT);

        visuals.widgets.noninteractive.bg_fill = COLOR_CARD_BG;
        visuals.widgets.inactive.bg_fill = COLOR_SIDEBAR;
        visuals.widgets.hovered.bg_fill = COLOR_CARD;
        visuals.widgets.active.bg_fill = COLOR_ACCENT;

        ctx.set_visuals(visuals);
    }

    fn analyze_selection(&mut self) {
        let profile = ContentAnalyzer::profile(&self.dropped_files);
        let rec = self.learning_engine.recommend(&profile);

        self.selected_format = rec.format;
        self.selected_level = rec.level;
        self.recommendation = Some(rec);
        self.profile = Some(profile);
    }

    fn start_extraction(&mut self) {
        if self.dropped_files.is_empty() {
            return;
        }
        let archive = self.dropped_files[0].clone();
        let dest = self.output_dir.clone().unwrap_or_else(|| archive.parent().unwrap_or(&archive).to_path_buf());

        let (tx, rx) = mpsc::channel(100);
        self.progress_rx = Some(rx);
        self.view_state = ViewState::Progress;
        self.start_time = Some(Instant::now());
        self.error_msg.clear();

        self.rt.spawn(async move {
            let res = tokio::task::spawn_blocking(move || {
                decompress(archive, dest, tx)
            })
            .await;

            if let Err(e) = res {
                eprintln!("Extraction error: {:?}", e);
            }
        });
    }

    fn start_compression(&mut self) {
        if self.dropped_files.is_empty() {
            return;
        }
        let (tx, rx) = mpsc::channel(100);
        self.progress_rx = Some(rx);
        self.view_state = ViewState::Progress;
        self.start_time = Some(Instant::now());
        self.error_msg.clear();

        let files = self.dropped_files.clone();
        let first = &files[0];
        let parent = self.output_dir.clone().unwrap_or_else(|| first.parent().unwrap_or(first).to_path_buf());
        let is_multi = files.len() > 1 || first.is_dir();

        let ext = match self.selected_format {
            ArchiveFormat::Zip => "zip",
            ArchiveFormat::SevenZ => "7z",
            ArchiveFormat::Zstd => {
                if is_multi { "tar.zst" } else { "zst" }
            }
            ArchiveFormat::Gzip => {
                if is_multi { "tar.gz" } else { "gz" }
            }
        };

        let archive_name = if self.custom_archive_name.trim().is_empty() {
            "archive".to_string()
        } else {
            self.custom_archive_name.trim().to_string()
        };

        let dest = parent.join(format!("{}.{}", archive_name, ext));
        let format = self.selected_format;
        let level = self.selected_level;

        self.rt.spawn(async move {
            let res = tokio::task::spawn_blocking(move || {
                compress(files, dest, format, level, tx)
            })
            .await;

            if let Err(e) = res {
                eprintln!("Compression error: {:?}", e);
            }
        });
    }

    fn load_history(&self) -> Vec<BenchmarkRecordEntry> {
        if let Ok(content) = fs::read_to_string("benchmark_history.json") {
            #[derive(Deserialize)]
            struct HistoryFile {
                #[serde(default)]
                records: Vec<BenchmarkRecordEntry>,
            }
            if let Ok(parsed) = serde_json::from_str::<HistoryFile>(&content) {
                return parsed.records;
            }
            serde_json::from_str(&content).unwrap_or_default()
        } else {
            Vec::new()
        }
    }
}

impl eframe::App for FlxApp {
    fn update(&mut self, ctx: &Context, _frame: &mut eframe::Frame) {
        if let Some(rx) = &mut self.progress_rx {
            loop {
                match rx.try_recv() {
                    Ok(update) => {
                        self.progress = update;
                        ctx.request_repaint();

                        if self.progress.percentage >= 100.0 {
                            if let (Some(profile), Some(start)) = (&self.profile, self.start_time) {
                                let duration_ms = start.elapsed().as_millis() as u64;
                                self.learning_engine.record_result(
                                    profile,
                                    self.selected_format,
                                    self.selected_level,
                                    profile.total_size,
                                    self.progress.bytes_processed,
                                    duration_ms,
                                );
                            }
                            self.view_state = ViewState::Finished;
                        }
                    }
                    Err(mpsc::error::TryRecvError::Empty) => break,
                    Err(mpsc::error::TryRecvError::Disconnected) => {
                        if self.view_state == ViewState::Progress {
                            if self.progress.percentage < 100.0 {
                                self.error_msg = "File processing failed.".to_string();
                                self.view_state = ViewState::Error;
                            } else {
                                self.view_state = ViewState::Finished;
                            }
                        }
                        self.progress_rx = None;
                        break;
                    }
                }
            }
        }

        // Sidebar Navigation
        egui::SidePanel::left("sidebar")
            .resizable(false)
            .exact_width(205.0)
            .frame(Frame::none().fill(COLOR_SIDEBAR).inner_margin(Margin::same(16.0)))
            .show(ctx, |ui| {
                ui.add_space(5.0);
                
                ui.horizontal(|ui| {
                    ui.label(RichText::new("FLX ").size(20.0).strong().color(COLOR_ACCENT));
                    ui.label(RichText::new("ARCHIVER").size(20.0).strong().color(COLOR_BORDER));
                });
                
                ui.label(RichText::new("v2.2").small().color(COLOR_MUTED));
                ui.add_space(25.0);

                let nav_item = |ui: &mut egui::Ui, label: &str, tab: Tab, current: Tab| -> bool {
                    let active = current == tab;
                    let btn = ui.add_sized(
                        [172.0, 38.0],
                        egui::Button::new(
                            RichText::new(label)
                                .size(13.0)
                                .color(if active { COLOR_TEXT } else { COLOR_MUTED }),
                        )
                        .fill(if active { COLOR_ACCENT } else { Color32::TRANSPARENT })
                    );
                    btn.clicked()
                };

                if nav_item(ui, "Archive / Extract", Tab::Archive, self.active_tab) {
                    self.active_tab = Tab::Archive;
                }
                ui.add_space(8.0);
                if nav_item(ui, "Explorer", Tab::Explorer, self.active_tab) {
                    self.active_tab = Tab::Explorer;
                }
                ui.add_space(8.0);
                if nav_item(ui, "Benchmark & Data", Tab::Benchmark, self.active_tab) {
                    self.active_tab = Tab::Benchmark;
                }
                ui.add_space(8.0);
                if nav_item(ui, "Settings", Tab::Settings, self.active_tab) {
                    self.active_tab = Tab::Settings;
                }
            });

        // Central Panel Content
        egui::CentralPanel::default()
            .frame(Frame::none().fill(COLOR_BG).inner_margin(Margin::same(24.0)))
            .show(ctx, |ui| {
                match self.active_tab {
                    Tab::Archive => self.show_archive_tab(ui, ctx),
                    Tab::Explorer => self.show_explorer_tab(ui),
                    Tab::Benchmark => self.show_benchmark_tab(ui),
                    Tab::Settings => self.show_settings_tab(ui),
                }
            });
    }
}

impl FlxApp {
    fn show_archive_tab(&mut self, ui: &mut egui::Ui, ctx: &Context) {
        match self.view_state {
            ViewState::DropZone => {
                let available_size = Vec2::new(ui.available_width(), 260.0);
                let drop_rect = ui.allocate_response(available_size, egui::Sense::drag());

                Frame::none()
                    .fill(COLOR_CARD_BG)
                    .stroke(Stroke::new(1.5_f32, COLOR_BORDER))
                    .show(ui, |ui| {
                        ui.allocate_ui_at_rect(drop_rect.rect, |ui| {
                            ui.vertical_centered(|ui| {
                                ui.add_space(75.0);
                                ui.label(RichText::new("Drag and drop files or a folder here").size(16.0).strong());
                                ui.label(RichText::new("Drop your data to start smart analysis").small().color(COLOR_MUTED));
                            });
                        });
                    });

                if !ctx.input(|i| i.raw.dropped_files.is_empty()) {
                    let files: Vec<PathBuf> = ctx
                        .input(|i| i.raw.dropped_files.clone())
                        .into_iter()
                        .filter_map(|f| f.path)
                        .collect();

                    if !files.is_empty() {
                        self.dropped_files = files;
                        if let Some(first) = self.dropped_files.first() {
                            if let Some(stem) = first.file_stem() {
                                self.custom_archive_name = stem.to_string_lossy().to_string();
                            }
                            if self.output_dir.is_none() {
                                self.output_dir = first.parent().map(|p| p.to_path_buf());
                            }
                        }
                        self.analyze_selection();
                        self.view_state = ViewState::SelectAction;
                    }
                }
            }

            ViewState::SelectAction => {
                ui.heading(RichText::new("Current Selection").size(18.0).strong());
                ui.add_space(10.0);

                Frame::none()
                    .fill(COLOR_CARD_BG)
                    .inner_margin(Margin::same(16.0))
                    .show(ui, |ui| {
                        ui.label(RichText::new(format!("Items: {}", self.dropped_files.len())).strong());
                        if let Some(p) = &self.profile {
                            ui.label(format!("Total Size: {}", format_bytes(p.total_size)));
                        }
                    });

                ui.add_space(20.0);

                let is_single_archive = self.dropped_files.len() == 1
                    && FormatDetector::detect_format(&self.dropped_files[0]).unwrap_or(DecompFormat::Unknown) != DecompFormat::Unknown;

                ui.horizontal(|ui| {
                    if is_single_archive {
                        if ui.add(egui::Button::new("EXTRACT").fill(COLOR_ACCENT).min_size(Vec2::new(160.0, 40.0))).clicked() {
                            self.start_extraction();
                        }
                    }

                    if ui.add(egui::Button::new("CONFIGURE COMPRESSION").fill(COLOR_CARD).min_size(Vec2::new(180.0, 40.0))).clicked() {
                        self.view_state = ViewState::ConfigureCompression;
                    }
                });
            }

            ViewState::ConfigureCompression => {
                ui.heading(RichText::new("Archive Configuration").size(18.0).strong());
                ui.add_space(15.0);

                Frame::none()
                    .fill(COLOR_CARD_BG)
                    .inner_margin(Margin::same(16.0))
                    .show(ui, |ui| {
                        ui.horizontal(|ui| {
                            ui.label(RichText::new("Archive Name:").strong());
                            ui.add(egui::TextEdit::singleline(&mut self.custom_archive_name).hint_text("e.g. MyArchive"));
                        });

                        ui.add_space(10.0);

                        ui.horizontal(|ui| {
                            ui.label(RichText::new("Output Directory:").strong());
                            let path_display = self
                                .output_dir
                                .as_ref()
                                .map(|p| p.display().to_string())
                                .unwrap_or_else(|| "Source Folder".to_string());
                            ui.monospace(path_display);

                            if ui.button("Browse...").clicked() {
                                if let Some(folder) = FileDialog::new().pick_folder() {
                                    self.output_dir = Some(folder);
                                }
                            }
                        });

                        ui.add_space(15.0);

                        if let Some(rec) = &self.recommendation {
                            Frame::none()
                                .fill(COLOR_CARD)
                                .inner_margin(Margin::same(10.0))
                                .show(ui, |ui| {
                                    ui.label(RichText::new("AI / Benchmark Recommendation:").strong().color(COLOR_ACCENT));
                                    ui.label(&rec.reason);
                                });
                            ui.add_space(15.0);
                        }

                        ui.horizontal(|ui| {
                            ui.label(RichText::new("Format:").strong());
                            ui.selectable_value(&mut self.selected_format, ArchiveFormat::Zip, "ZIP");
                            ui.selectable_value(&mut self.selected_format, ArchiveFormat::SevenZ, "7z");
                            ui.selectable_value(&mut self.selected_format, ArchiveFormat::Zstd, "Zstd");
                            ui.selectable_value(&mut self.selected_format, ArchiveFormat::Gzip, "Gzip");
                        });

                        ui.add_space(15.0);

                        ui.horizontal(|ui| {
                            ui.label(RichText::new("Compression Level:").strong());
                            ui.selectable_value(&mut self.selected_level, CompressionLevel::Fast, "Fast");
                            ui.selectable_value(&mut self.selected_level, CompressionLevel::Default, "Balanced");
                            ui.selectable_value(&mut self.selected_level, CompressionLevel::Maximum, "Maximum");
                        });
                    });

                ui.add_space(20.0);

                if ui.add(egui::Button::new("START COMPRESSION").fill(COLOR_ACCENT).min_size(Vec2::new(220.0, 42.0))).clicked() {
                    self.start_compression();
                }
            }

            ViewState::Progress => {
                ui.heading(RichText::new("Processing...").size(18.0).strong());
                ui.add_space(15.0);

                Frame::none()
                    .fill(COLOR_CARD_BG)
                    .inner_margin(Margin::same(20.0))
                    .show(ui, |ui| {
                        ui.label(format!("Active file: {}", self.progress.current_file));
                        ui.add_space(10.0);
                        ui.add(egui::ProgressBar::new(self.progress.percentage / 100.0).show_percentage().animate(true));
                    });
            }

            ViewState::Finished => {
                Frame::none()
                    .fill(COLOR_CARD_BG)
                    .stroke(Stroke::new(1.0_f32, COLOR_SUCCESS))
                    .inner_margin(Margin::same(20.0))
                    .show(ui, |ui| {
                        ui.label(RichText::new("Operation completed successfully!").color(COLOR_SUCCESS).size(18.0).strong());
                        ui.add_space(15.0);

                        if ui.add(egui::Button::new("New Operation").fill(COLOR_ACCENT).min_size(Vec2::new(160.0, 36.0))).clicked() {
                            self.view_state = ViewState::DropZone;
                            self.dropped_files.clear();
                        }
                    });
            }

            ViewState::Error => {
                Frame::none()
                    .fill(COLOR_CARD_BG)
                    .stroke(Stroke::new(1.0_f32, COLOR_ERROR))
                    .inner_margin(Margin::same(20.0))
                    .show(ui, |ui| {
                        ui.label(RichText::new("An error occurred during operation").color(COLOR_ERROR).size(18.0).strong());
                        ui.add_space(10.0);
                        ui.label(&self.error_msg);
                        ui.add_space(15.0);

                        if ui.add(egui::Button::new("Retry").fill(COLOR_CARD).min_size(Vec2::new(140.0, 36.0))).clicked() {
                            self.view_state = ViewState::DropZone;
                            self.dropped_files.clear();
                        }
                    });
            }
        }
    }

    fn show_explorer_tab(&mut self, ui: &mut egui::Ui) {
        ui.heading(RichText::new("Archive Explorer").size(20.0).strong());
        ui.add_space(10.0);

        Frame::none()
            .fill(COLOR_CARD_BG)
            .inner_margin(Margin::same(8.0))
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    if ui.add(egui::Button::new("Add").min_size(Vec2::new(80.0, 30.0))).clicked() {
                        if let Some(files) = FileDialog::new().pick_files() {
                            self.dropped_files = files;
                            if let Some(first) = self.dropped_files.first() {
                                if let Some(stem) = first.file_stem() {
                                    self.custom_archive_name = stem.to_string_lossy().to_string();
                                }
                            }
                            self.analyze_selection();
                            self.active_tab = Tab::Archive;
                            self.view_state = ViewState::SelectAction;
                        }
                    }
                    if ui.add(egui::Button::new("Extract").min_size(Vec2::new(80.0, 30.0))).clicked() {
                        self.start_extraction();
                    }
                    if ui.add(egui::Button::new("Clear").min_size(Vec2::new(75.0, 30.0))).clicked() {
                        self.dropped_files.clear();
                    }
                });
            });

        ui.add_space(15.0);
        ui.label(RichText::new("Files loaded in memory:").strong());
        ui.add_space(8.0);

        Frame::none()
            .fill(COLOR_CARD_BG)
            .inner_margin(Margin::same(12.0))
            .show(ui, |ui| {
                if self.dropped_files.is_empty() {
                    ui.vertical_centered(|ui| {
                        ui.add_space(40.0);
                        ui.label(RichText::new("No active file in the explorer.").color(COLOR_MUTED));
                        ui.label(RichText::new("Use 'Add' to load files or folders.").small().color(COLOR_MUTED));
                        ui.add_space(40.0);
                    });
                } else {
                    egui::ScrollArea::vertical().max_height(280.0).show(ui, |ui| {
                        egui::Grid::new("explorer_grid")
                            .striped(true)
                            .spacing([20.0, 8.0])
                            .show(ui, |ui| {
                                ui.label(RichText::new("Name").strong().color(COLOR_MUTED));
                                ui.label(RichText::new("Size").strong().color(COLOR_MUTED));
                                ui.label(RichText::new("Type").strong().color(COLOR_MUTED));
                                ui.end_row();

                                for file in &self.dropped_files {
                                    let name = file.file_name().unwrap_or_default().to_string_lossy();
                                    let size = if file.is_file() {
                                        fs::metadata(file).map(|m| format_bytes(m.len())).unwrap_or_default()
                                    } else {
                                        "Folder".to_string()
                                    };
                                    let f_type = if file.is_dir() { "Folder" } else { "File" };

                                    ui.label(name.to_string());
                                    ui.label(size);
                                    ui.label(f_type);
                                    ui.end_row();
                                }
                            });
                    });
                }
            });
    }

    fn show_benchmark_tab(&mut self, ui: &mut egui::Ui) {
        ui.heading(RichText::new("Benchmark & Performance Dashboard").size(20.0).strong());
        ui.add_space(15.0);

        let records = self.load_history();

        if records.is_empty() {
            Frame::none()
                .fill(COLOR_CARD_BG)
                .inner_margin(Margin::same(30.0))
                .show(ui, |ui| {
                    ui.vertical_centered(|ui| {
                        ui.label(RichText::new("No recorded data").size(16.0).strong());
                        ui.label(RichText::new("Perform compressions to feed the learning engine.").color(COLOR_MUTED));
                    });
                });
            return;
        }

        let total = records.len();
        let avg_ratio: f32 = records.iter().map(|r| r.ratio_percent).sum::<f32>() / total as f32;
        let max_speed: f32 = records.iter().map(|r| r.speed_mb_s).fold(0.0f32, f32::max);

        ui.horizontal(|ui| {
            self.draw_stat_card(ui, "TOTAL TESTS", &total.to_string());
            self.draw_stat_card(ui, "AVG RATIO", &format!("{:.1}%", avg_ratio));
            self.draw_stat_card(ui, "MAX SPEED", &format!("{:.1} MB/s", max_speed));
        });

        ui.add_space(20.0);
        ui.label(RichText::new("Detailed Execution History").strong());
        ui.add_space(8.0);

        egui::ScrollArea::vertical().max_height(240.0).show(ui, |ui| {
            egui::Grid::new("bench_table")
                .striped(true)
                .spacing([16.0, 8.0])
                .show(ui, |ui| {
                    ui.label(RichText::new("Format").strong().color(COLOR_MUTED));
                    ui.label(RichText::new("Level").strong().color(COLOR_MUTED));
                    ui.label(RichText::new("Initial Size").strong().color(COLOR_MUTED));
                    ui.label(RichText::new("Final Size").strong().color(COLOR_MUTED));
                    ui.label(RichText::new("Ratio").strong().color(COLOR_MUTED));
                    ui.label(RichText::new("Speed").strong().color(COLOR_MUTED));
                    ui.end_row();

                    for r in records.iter().rev() {
                        ui.label(&r.format);
                        ui.label(&r.level);
                        ui.label(format_bytes(r.original_size_bytes));
                        ui.label(format_bytes(r.compressed_size_bytes));
                        ui.label(format!("{:.1}%", r.ratio_percent));
                        ui.label(format!("{:.1} MB/s", r.speed_mb_s));
                        ui.end_row();
                    }
                });
        });
    }

    fn draw_stat_card(&self, ui: &mut egui::Ui, title: &str, value: &str) {
        Frame::none()
            .fill(COLOR_CARD_BG)
            .inner_margin(Margin::same(12.0))
            .show(ui, |ui| {
                ui.set_width(130.0);
                ui.label(RichText::new(title).small().color(COLOR_MUTED));
                ui.add_space(2.0);
                ui.label(RichText::new(value).size(18.0).strong().color(COLOR_ACCENT));
            });
    }

    fn show_settings_tab(&mut self, ui: &mut egui::Ui) {
        ui.heading(RichText::new("Application Settings").size(20.0).strong());
        ui.add_space(15.0);

        Frame::none()
            .fill(COLOR_CARD_BG)
            .inner_margin(Margin::same(16.0))
            .show(ui, |ui| {
                ui.label(RichText::new("Configured Compression Engines").strong());
                ui.add_space(5.0);
                ui.label("• ZIP: Universal multi-file standard.");
                ui.label("• 7z: Maximum compression ratio.");
                ui.label("• Zstd: Ultra-fast algorithm (Multi-threading enabled).");
                ui.label("• Gzip: Classic UNIX format.");
            });
    }
}

fn format_bytes(bytes: u64) -> String {
    if bytes >= 1_073_741_824 {
        format!("{:.2} GB", bytes as f64 / 1_073_741_824.0)
    } else if bytes >= 1_048_576 {
        format!("{:.2} MB", bytes as f64 / 1_048_576.0)
    } else if bytes >= 1024 {
        format!("{:.2} KB", bytes as f64 / 1024.0)
    } else {
        format!("{} B", bytes)
    }
}