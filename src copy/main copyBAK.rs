use eframe::egui;
mod audio;
mod menu;
use audio::{AudioEngine, EngineEvent, TrackMetadata};
use egui_phosphor::regular as ph;
use elegance::{Accent, Badge, BadgeTone, Button, ButtonSize, Card, Theme};
use std::path::Path;

#[derive(Clone)]
struct PlaylistItem {
    path: String,
    title: String,
    artist: String,
    duration_sec: Option<f64>,
}

struct PlayerApp {
    engine: AudioEngine,
    playlist: Vec<PlaylistItem>,
    current_index: Option<usize>,
    metadata: TrackMetadata,
    spectrum: Vec<f32>,
    volume: f64,
    muted: bool,
    status: String,
    seek_pos: f64,
    duration: f64,
    is_playing: bool,
    search_query: String,
    repeat_mode: bool,
    shuffle_mode: bool,
}

impl PlayerApp {
    fn new(cc: &eframe::CreationContext<'_>) -> Self {
        let mut fonts = egui::FontDefinitions::default();
        egui_phosphor::add_to_fonts(&mut fonts, egui_phosphor::Variant::Regular);
        cc.egui_ctx.set_fonts(fonts);

        Theme::slate().install(&cc.egui_ctx);
        cc.egui_ctx.set_theme(egui::ThemePreference::Dark);

        let engine = AudioEngine::new().expect("Failed to init AudioEngine");

        Self {
            engine,
            playlist: Vec::new(),
            current_index: None,
            metadata: TrackMetadata::default(),
            spectrum: vec![0.0; 64],
            volume: 0.8,
            muted: false,
            status: "System Ready".into(),
            seek_pos: 0.0,
            duration: 0.0,
            is_playing: false,
            search_query: String::new(),
            repeat_mode: false,
            shuffle_mode: false,
        }
    }

    fn play_index(&mut self, index: usize) {
        if let Some(item) = self.playlist.get(index).cloned() {
            if let Err(e) = self.engine.load_file(&item.path) {
                self.status = format!("Error: {e}");
                return;
            }
            self.current_index = Some(index);
            self.status = format!("Playing: {}", item.title);
            self.is_playing = true;
        }
    }

    fn toggle_play_pause(&mut self) {
        if self.is_playing {
            let _ = self.engine.pause();
            self.is_playing = false;
            self.status = "Paused".into();
        } else if self.current_index.is_none() && !self.playlist.is_empty() {
            self.play_index(0);
        } else {
            let _ = self.engine.play();
            self.is_playing = true;
            self.status = "Playing".into();
        }
    }

    fn next_track(&mut self) {
        if self.playlist.is_empty() {
            return;
        }
        if self.shuffle_mode && self.playlist.len() > 1 {
            use std::time::{SystemTime, UNIX_EPOCH};
            let seed = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map(|d| d.subsec_nanos())
                .unwrap_or(1);
            let mut next = (seed as usize) % self.playlist.len();
            if Some(next) == self.current_index {
                next = (next + 1) % self.playlist.len();
            }
            self.play_index(next);
            return;
        }

        let next = match self.current_index {
            Some(i) if i + 1 < self.playlist.len() => i + 1,
            _ => {
                if self.repeat_mode {
                    0
                } else {
                    return;
                }
            }
        };
        self.play_index(next);
    }

    fn prev_track(&mut self) {
        if self.playlist.is_empty() {
            return;
        }
        let prev = match self.current_index {
            Some(0) | None => self.playlist.len().saturating_sub(1),
            Some(i) => i - 1,
        };
        self.play_index(prev);
    }

    fn add_file_paths(&mut self, paths: &[std::path::PathBuf]) {
        for path in paths {
            if path.is_dir() {
                self.add_folder_path(path);
            } else if path.is_file() {
                let filename = path
                    .file_stem()
                    .unwrap_or_default()
                    .to_string_lossy()
                    .into_owned();
                self.playlist.push(PlaylistItem {
                    path: path.to_string_lossy().into_owned(),
                    title: filename,
                    artist: "Unknown Artist".into(),
                    duration_sec: None,
                });
            }
        }
    }

    fn add_folder_path(&mut self, folder: &Path) {
        if let Ok(entries) = std::fs::read_dir(folder) {
            let mut files: Vec<_> = entries
                .flatten()
                .map(|e| e.path())
                .filter(|p| p.is_file())
                .collect();
            files.sort();

            for path in files {
                if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
                    let ext = ext.to_lowercase();
                    if ["mp3", "wav", "flac", "ogg", "m4a", "aac", "opus"].contains(&ext.as_str())
                    {
                        let filename = path
                            .file_stem()
                            .unwrap_or_default()
                            .to_string_lossy()
                            .into_owned();
                        self.playlist.push(PlaylistItem {
                            path: path.to_string_lossy().into_owned(),
                            title: filename,
                            artist: "Unknown Artist".into(),
                            duration_sec: None,
                        });
                    }
                }
            }
        }
    }

    fn handle_drag_and_drop(&mut self, ctx: &egui::Context) {
        let dropped_files = ctx.input(|i| i.raw.dropped_files.clone());
        if !dropped_files.is_empty() {
            let paths: Vec<_> = dropped_files
                .into_iter()
                .map(|f| f.path().to_path_buf())
                .collect();
            self.add_file_paths(&paths);
        }
    }

    fn poll_engine(&mut self) {
        for ev in self.engine.poll_events() {
            match ev {
                EngineEvent::Spectrum(data) => self.spectrum = data,
                EngineEvent::EndOfStream => self.next_track(),
                EngineEvent::MetadataUpdated(meta) => {
                    if let Some(idx) = self.current_index {
                        if let Some(item) = self.playlist.get_mut(idx) {
                            if let Some(ref t) = meta.title {
                                item.title = t.clone();
                            }
                            if let Some(ref a) = meta.artist {
                                item.artist = a.clone();
                            }
                        }
                    }
                    self.metadata = meta;
                }
                EngineEvent::Error(msg) => {
                    self.status = format!("Error: {msg}");
                    self.is_playing = false;
                }
            }
        }
        if let Some(pos) = self.engine.get_position() {
            self.seek_pos = pos;
        }
        if let Some(dur) = self.engine.get_duration() {
            self.duration = dur;
            if let Some(idx) = self.current_index {
                if let Some(item) = self.playlist.get_mut(idx) {
                    if item.duration_sec.is_none() && dur > 0.0 {
                        item.duration_sec = Some(dur);
                    }
                }
            }
        }
        self.is_playing = self.engine.is_playing();
    }
}

fn format_time(sec: f64) -> String {
    if !sec.is_finite() || sec < 0.0 {
        return "0:00".into();
    }
    let s = sec as u64;
    format!("{}:{:02}", s / 60, s % 60)
}

impl eframe::App for PlayerApp {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        self.poll_engine();
        self.handle_drag_and_drop(ui.ctx());
        ui.ctx().request_repaint();

        let ctx = ui.ctx().clone();

        // 1. תפריט עליון
        egui::Panel::top("top_menu").show(ui, |ui| {
            if let Some(action) = menu::draw_menu(ui) {
                match action {
                    menu::MenuAction::AddFiles(files) => {
                        self.add_file_paths(&files);
                    }
                    menu::MenuAction::AddFolder(folder) => {
                        self.add_folder_path(&folder);
                    }
                    menu::MenuAction::SetPreset(preset) => self.engine.set_preset(preset),
                    menu::MenuAction::SetTheme(theme) => {
                        match theme {
                            elegance::BuiltInTheme::Slate => {
                                elegance::Theme::slate().install(ui.ctx())
                            }
                            elegance::BuiltInTheme::Charcoal => {
                                elegance::Theme::charcoal().install(ui.ctx())
                            }
                            elegance::BuiltInTheme::Frost => {
                                elegance::Theme::frost().install(ui.ctx())
                            }
                            elegance::BuiltInTheme::Paper => {
                                elegance::Theme::paper().install(ui.ctx())
                            }
                            _ => {}
                        }
                        if theme == elegance::BuiltInTheme::Paper {
                            ui.ctx().set_theme(egui::ThemePreference::Light);
                        } else {
                            ui.ctx().set_theme(egui::ThemePreference::Dark);
                        }
                    }
                    menu::MenuAction::ClearPlaylist => {
                        self.playlist.clear();
                        self.current_index = None;
                        let _ = self.engine.stop();
                        self.is_playing = false;
                        self.status = "Playlist Cleared".into();
                    }
                    menu::MenuAction::Quit => ctx.send_viewport_cmd(egui::ViewportCommand::Close),
                    menu::MenuAction::TogglePlayPause => self.toggle_play_pause(),
                    menu::MenuAction::NextTrack => self.next_track(),
                    menu::MenuAction::PrevTrack => self.prev_track(),
                }
            }
        });

        // 2. סטטוס בר תחתון
        egui::Panel::bottom("bottom_status_bar").show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.label(
                    egui::RichText::new(format!("{} {}", ph::ACTIVITY, self.status))
                        .small()
                        .weak(),
                );
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    let repeat_tag = if self.repeat_mode { " [Repeat ON]" } else { "" };
                    let shuffle_tag = if self.shuffle_mode { " [Shuffle ON]" } else { "" };
                    ui.label(
                        egui::RichText::new(format!(
                            "{} tracks{}{}",
                            self.playlist.len(),
                            repeat_tag,
                            shuffle_tag
                        ))
                        .small()
                        .weak(),
                    );
                });
            });
        });

        // 3. הפאנל המרכזי
        egui::CentralPanel::default().show(ui, |ui| {
            ui.add_space(4.0);

            // ─── CARD 1: NOW PLAYING & CONTROLS ───
            Card::new()
                .heading(format!("{} Audio Controller", ph::EQUALIZER))
                .show(ui, |ui| {
                    // כותרת השיר המתנגן + אמן
                    let (track_title, track_artist) = if let Some(i) = self.current_index {
                        if let Some(item) = self.playlist.get(i) {
                            (
                                self.metadata.title.as_deref().unwrap_or(&item.title),
                                self.metadata.artist.as_deref().unwrap_or(&item.artist),
                            )
                        } else {
                            ("No Track Selected", "Drop files or use File menu")
                        }
                    } else {
                        ("No Track Selected", "Drop files or use File menu")
                    };

                    ui.horizontal(|ui| {
                        let now_playing_icon = if self.is_playing {
                            ph::VINYL_RECORD
                        } else {
                            ph::MUSIC_NOTE
                        };
                        let tone = if self.is_playing {
                            BadgeTone::Ok
                        } else {
                            BadgeTone::Neutral
                        };
                        ui.add(Badge::new(now_playing_icon, tone));

                        ui.vertical(|ui| {
                            ui.label(egui::RichText::new(track_title).strong().size(15.0));
                            ui.label(egui::RichText::new(track_artist).small().weak());
                        });
                    });

                    ui.add_space(8.0);

                    // שורת השליטה: כפתורים + ציר זמן + ווליום
                    ui.horizontal(|ui| {
                        // כפתורי ניגון
                        ui.horizontal(|ui| {
                            // כפתור Shuffle
                            let mut shuffle_btn = Button::new(ph::SHUFFLE).outline();
                            if self.shuffle_mode {
                                shuffle_btn = shuffle_btn.accent(Accent::Blue);
                            }
                            if ui.add(shuffle_btn).clicked() {
                                self.shuffle_mode = !self.shuffle_mode;
                            }

                            if ui
                                .add(Button::new(ph::SKIP_BACK).size(ButtonSize::Medium))
                                .clicked()
                            {
                                self.prev_track();
                            }

                            let play_icon = if self.is_playing {
                                ph::PAUSE
                            } else {
                                ph::PLAY
                            };
                            if ui
                                .add(
                                    Button::new(play_icon)
                                        .accent(Accent::Blue)
                                        .size(ButtonSize::Large),
                                )
                                .clicked()
                            {
                                self.toggle_play_pause();
                            }

                            if ui
                                .add(Button::new(ph::SKIP_FORWARD).size(ButtonSize::Medium))
                                .clicked()
                            {
                                self.next_track();
                            }

                            // כפתור Repeat
                            let mut repeat_btn = Button::new(ph::REPEAT).outline();
                            if self.repeat_mode {
                                repeat_btn = repeat_btn.accent(Accent::Blue);
                            }
                            if ui.add(repeat_btn).clicked() {
                                self.repeat_mode = !self.repeat_mode;
                            }
                        });

                        ui.add_space(12.0);

                        // ציר זמן (Timeline) עם קריאת זמנים משני הצדדים
                        let timeline_width = (ui.available_width() - 170.0).max(100.0);
                        ui.vertical(|ui| {
                            ui.horizontal(|ui| {
                                ui.label(
                                    egui::RichText::new(format_time(self.seek_pos))
                                        .monospace()
                                        .small()
                                        .weak(),
                                );

                                let mut pos = self.seek_pos as f32;
                                let max = self.duration.max(0.1) as f32;
                                ui.style_mut().spacing.slider_width = timeline_width;

                                let slider = egui::Slider::new(&mut pos, 0.0..=max)
                                    .show_value(false);

                                if ui.add(slider).changed() {
                                    let _ = self.engine.seek_position(pos as f64);
                                    self.seek_pos = pos as f64;
                                }

                                ui.label(
                                    egui::RichText::new(format_time(self.duration))
                                        .monospace()
                                        .small()
                                        .weak(),
                                );
                            });
                        });

                        ui.add_space(8.0);

                        // אזור ווליום
                        ui.horizontal(|ui| {
                            let mute_icon = if self.muted {
                                ph::SPEAKER_SLASH
                            } else {
                                ph::SPEAKER_HIGH
                            };
                            if ui.add(Button::new(mute_icon).outline()).clicked() {
                                self.muted = !self.muted;
                                self.engine.set_mute(self.muted);
                            }

                            let mut vol = self.volume as f32;
                            ui.style_mut().spacing.slider_width = 65.0;
                            let vol_slider = egui::Slider::new(&mut vol, 0.0..=1.5)
                                .show_value(false);

                            if ui.add(vol_slider).changed() {
                                self.volume = vol as f64;
                                self.engine.set_volume(self.volume);
                            }

                            ui.label(
                                egui::RichText::new(format!("{}%", (self.volume * 100.0) as i32))
                                    .monospace()
                                    .small()
                                    .weak(),
                            );
                        });
                    });
                });

            ui.add_space(8.0);

            // ─── CARD 2: SPECTRUM ANALYZER ───
            Card::new()
                .heading(format!("{} Live Spectrum Visualizer", ph::WAVEFORM))
                .show(ui, |ui| {
                    let height = 44.0;
                    let bar_count = self.spectrum.len().max(1);
                    let gap = 2.0;
                    let avail_width = ui.available_width();
                    let bar_width = ((avail_width - (bar_count - 1) as f32 * gap) / bar_count as f32).max(1.5);
                    let total_width = bar_count as f32 * bar_width + (bar_count - 1) as f32 * gap;

                    let (rect, _) = ui.allocate_exact_size(
                        egui::Vec2::new(total_width, height),
                        egui::Sense::hover(),
                    );
                    let painter = ui.painter_at(rect);

                    for (i, &v) in self.spectrum.iter().enumerate() {
                        let t = i as f32 / bar_count as f32;
                        // מעבר צבעים דינמי מתכלת/טורקיז לסגול/מג'נטה זוהר
                        let r = (35.0 + t * 145.0) as u8;
                        let g = (165.0 - t * 90.0) as u8;
                        let b = (245.0 - t * 20.0) as u8;
                        let bar_color = egui::Color32::from_rgb(r, g, b);

                        let h = (v * 1.35).clamp(0.0, 1.0) * height;
                        let x = rect.left() + i as f32 * (bar_width + gap);

                        // עמודת רקע עדינה
                        let bg_rect = egui::Rect::from_min_size(
                            egui::pos2(x, rect.bottom() - height),
                            egui::Vec2::new(bar_width, height),
                        );
                        painter.rect_filled(bg_rect, 1.5, bar_color.linear_multiply(0.08));

                        // עמודת הספקטרום הפעילה
                        if h > 1.0 {
                            let bar_rect = egui::Rect::from_min_size(
                                egui::pos2(x, rect.bottom() - h),
                                egui::Vec2::new(bar_width, h),
                            );
                            painter.rect_filled(bar_rect, 1.5, bar_color);
                        } else {
                            let empty_rect = egui::Rect::from_min_size(
                                egui::pos2(x, rect.bottom() - 2.5),
                                egui::Vec2::new(bar_width, 2.5),
                            );
                            painter.rect_filled(empty_rect, 1.0, bar_color.linear_multiply(0.3));
                        }
                    }
                });

            ui.add_space(8.0);

            // ─── CARD 3: PLAYLIST MODULE ───
            let available_y = ui.available_height() - 4.0;
            Card::new()
                .heading(format!("{} Playlist & Media Library", ph::LIST_DASHES))
                .show(ui, |ui| {
                    // סרגל כלים של הפלייליסט: שדה חיפוש + כפתורי הוספה ומחיקה
                    ui.horizontal(|ui| {
                        ui.add(
                            egui::TextEdit::singleline(&mut self.search_query)
                                .hint_text(format!("{} Filter playlist...", ph::MAGNIFYING_GLASS))
                                .desired_width(180.0),
                        );

                        if !self.search_query.is_empty() {
                            if ui.add(Button::new(ph::X).outline()).clicked() {
                                self.search_query.clear();
                            }
                        }

                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            if ui
                                .add(Button::new(format!("{} Clear", ph::TRASH)).outline())
                                .clicked()
                            {
                                self.playlist.clear();
                                self.current_index = None;
                                let _ = self.engine.stop();
                                self.is_playing = false;
                                self.status = "Playlist Cleared".into();
                            }

                            if ui
                                .add(Button::new(format!("{} Add Files", ph::FILE_PLUS)).outline())
                                .clicked()
                            {
                                if let Some(files) = rfd::FileDialog::new()
                                    .add_filter("Audio", &["mp3", "wav", "flac", "ogg", "m4a", "aac", "opus"])
                                    .pick_files()
                                {
                                    self.add_file_paths(&files);
                                }
                            }
                        });
                    });

                    ui.separator();

                    if self.playlist.is_empty() {
                        ui.centered_and_justified(|ui| {
                            ui.vertical_centered(|ui| {
                                ui.add_space(20.0);
                                ui.label(egui::RichText::new(format!("{} Drag & Drop your audio files here", ph::FILE_ARROW_DOWN)).size(16.0).weak());
                                ui.label(egui::RichText::new("or click 'Add Files' above").small().weak());
                                ui.add_space(20.0);
                            });
                        });
                        return;
                    }

                    // גלילת רשימת ההשמעה עם תמיכה בסינון
                    egui::ScrollArea::vertical()
                        .min_scrolled_height(available_y.max(100.0))
                        .max_height(available_y.max(100.0))
                        .show(ui, |ui| {
                            let mut play_idx = None;
                            let mut remove_idx = None;
                            let query = self.search_query.to_lowercase();

                            for (i, item) in self.playlist.iter().enumerate() {
                                if !query.is_empty()
                                    && !item.title.to_lowercase().contains(&query)
                                    && !item.artist.to_lowercase().contains(&query)
                                {
                                    continue;
                                }

                                ui.horizontal(|ui| {
                                    let is_current = self.current_index == Some(i);
                                    let icon = if is_current {
                                        if self.is_playing {
                                            ph::SPEAKER_HIGH
                                        } else {
                                            ph::PAUSE
                                        }
                                    } else {
                                        ph::MUSIC_NOTE
                                    };
                                    let tone = if is_current {
                                        BadgeTone::Ok
                                    } else {
                                        BadgeTone::Neutral
                                    };

                                    ui.add(Badge::new(icon, tone));

                                    let label = format!("{:02}. {} — {}", i + 1, item.title, item.artist);
                                    let resp = ui.selectable_label(is_current, label);
                                    if resp.double_clicked() {
                                        play_idx = Some(i);
                                    }

                                    // תפריט קליק ימני (Context Menu)
                                    resp.context_menu(|ui| {
                                        if ui.button(format!("{} Play", ph::PLAY)).clicked() {
                                            play_idx = Some(i);
                                            ui.close();
                                        }
                                        if ui.button(format!("{} Remove", ph::TRASH)).clicked() {
                                            remove_idx = Some(i);
                                            ui.close();
                                        }
                                    });

                                    // כפתור מחיקה מהיר מימין
                                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                        if let Some(dur) = item.duration_sec {
                                            ui.label(egui::RichText::new(format_time(dur)).monospace().small().weak());
                                        }
                                        if ui.small_button(ph::X).on_hover_text("Remove from playlist").clicked() {
                                            remove_idx = Some(i);
                                        }
                                    });
                                });
                                ui.separator();
                            }

                            if let Some(rem_i) = remove_idx {
                                self.playlist.remove(rem_i);
                                if self.current_index == Some(rem_i) {
                                    self.current_index = None;
                                    let _ = self.engine.stop();
                                    self.is_playing = false;
                                } else if let Some(curr) = self.current_index {
                                    if curr > rem_i {
                                        self.current_index = Some(curr - 1);
                                    }
                                }
                            }

                            if let Some(i) = play_idx {
                                self.play_index(i);
                            }
                        });
                });
        });
    }
}

fn main() -> eframe::Result<()> {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([820.0, 620.0])
            .with_min_inner_size([580.0, 480.0])
            .with_title("Audio Engine (Elegance Edition)"),
        ..Default::default()
    };
    eframe::run_native(
        "Music Player",
        options,
        Box::new(|cc| Ok(Box::new(PlayerApp::new(cc)))),
    )
}
