use eframe::egui;
///
mod audio;
mod menu;
use audio::{ALL_PRESETS, AudioEngine, EQ_FREQUENCIES, EngineEvent, Preset, TrackMetadata};
use egui_phosphor::regular as ph;
use elegance::{Accent, Badge, BadgeTone, Button, ButtonSize, Card,Slider, Modal, Theme};
use std::path::Path;

#[derive(Clone, serde::Serialize, serde::Deserialize)]
struct PlaylistItem {
    path: String,
    title: String,
    artist: String,
    duration_sec: Option<f64>,
}

#[derive(serde::Serialize, serde::Deserialize)]
struct SavedState {
    playlist: Vec<PlaylistItem>,
    current_index: Option<usize>,
    volume: f64,
    muted: bool,
    repeat_mode: bool,
    shuffle_mode: bool,
    active_preset: Option<Preset>,
    eq_bands: [f64; 10],
    bass_boost_db: f64,
    balance_pan: f32,
    auto_headroom: bool,
}

impl SavedState {
    const FILE_NAME: &'static str = "state.json";

    fn load() -> Option<Self> {
        let content = std::fs::read_to_string(Self::FILE_NAME).ok()?;
        serde_json::from_str(&content).ok()
    }

    fn save(&self) {
        if let Ok(json) = serde_json::to_string_pretty(self) {
            let _ = std::fs::write(Self::FILE_NAME, json);
        }
    }
}

struct PlayerApp {
    engine: AudioEngine,
    playlist: Vec<PlaylistItem>,
    current_index: Option<usize>,
    selected_index: Option<usize>,
    scroll_to_selected: bool,
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
    show_about_dialog: bool,
    show_eq_window: bool,
    active_preset: Option<Preset>,
    bass_boost_db: f64,
    balance_pan: f32,
    auto_headroom: bool,
}

impl PlayerApp {
    fn new(cc: &eframe::CreationContext<'_>) -> Self {
        let mut fonts = egui::FontDefinitions::default();
        egui_phosphor::add_to_fonts(&mut fonts, egui_phosphor::Variant::Regular);
        fonts.font_data.insert(
            "custom_font".to_owned(),
            std::sync::Arc::new(egui::FontData::from_static(include_bytes!(
                "../assets/FM.ttf"
            ))),
        );
        // הגדרה שהפונט הזה יהיה ברירת המחדל לטקסט רגיל ולטקסט מודגש
        fonts
            .families
            .entry(egui::FontFamily::Proportional)
            .or_default()
            .insert(0, "custom_font".to_owned());
        cc.egui_ctx.set_fonts(fonts);

        Theme::slate().install(&cc.egui_ctx);
        cc.egui_ctx.set_theme(egui::ThemePreference::Dark);

        let mut engine = AudioEngine::new().expect("Failed to init AudioEngine");
        let saved = SavedState::load();
        let playlist = saved
            .as_ref()
            .map(|s| s.playlist.clone())
            .unwrap_or_default();
        let current_index = saved.as_ref().and_then(|s| s.current_index);
        let volume = saved.as_ref().map(|s| s.volume).unwrap_or(0.8);
        let muted = saved.as_ref().map(|s| s.muted).unwrap_or(false);
        let repeat_mode = saved.as_ref().map(|s| s.repeat_mode).unwrap_or(false);
        let shuffle_mode = saved.as_ref().map(|s| s.shuffle_mode).unwrap_or(false);
        let active_preset = saved
            .as_ref()
            .and_then(|s| s.active_preset)
            .or(Some(Preset::Flat));
        let bass_boost_db = saved.as_ref().map(|s| s.bass_boost_db).unwrap_or(0.0);
        let balance_pan = saved.as_ref().map(|s| s.balance_pan).unwrap_or(0.0);
        let auto_headroom = saved.as_ref().map(|s| s.auto_headroom).unwrap_or(true);
        // החלת הערכים השמורים על מנוע האודיו
        engine.set_volume(volume);
        engine.set_mute(muted);
        engine.set_balance(balance_pan);
        engine.set_auto_headroom(auto_headroom);
        if let Some(ref s) = saved {
            engine.set_all_eq_bands(s.eq_bands);
        } else if let Some(p) = active_preset {
            engine.set_preset(p);
        }

        Self {
            engine,
            playlist,
            current_index,
            selected_index: current_index,
            scroll_to_selected: current_index.is_some(),
            metadata: TrackMetadata::default(),
            spectrum: vec![0.0; 64],
            volume,
            muted,
            status: "System Ready".into(),
            seek_pos: 0.0,
            duration: 0.0,
            is_playing: false,
            search_query: String::new(),
            repeat_mode,
            shuffle_mode,
            show_about_dialog: false,
            show_eq_window: false,
            active_preset,
            bass_boost_db,
            balance_pan,
            auto_headroom,
        }
    }

    fn save_state(&self) {
        let state = SavedState {
            playlist: self.playlist.clone(),
            current_index: self.current_index,
            volume: self.volume,
            muted: self.muted,
            repeat_mode: self.repeat_mode,
            shuffle_mode: self.shuffle_mode,
            active_preset: self.active_preset,
            eq_bands: *self.engine.get_eq_bands(),
            bass_boost_db: self.bass_boost_db,
            balance_pan: self.balance_pan,
            auto_headroom: self.auto_headroom,
        };
        state.save();
    }

    fn play_index(&mut self, index: usize) {
        if let Some(item) = self.playlist.get(index).cloned() {
            if let Err(e) = self.engine.load_file(&item.path) {
                self.status = format!("Error: {e}");
                return;
            }
            self.current_index = Some(index);
            self.selected_index = Some(index);
            self.scroll_to_selected = true;
            self.status = format!("Playing: {}", item.title);
            self.is_playing = true;
        }
    }

    fn toggle_play_pause(&mut self) {
        if self.is_playing {
            let _ = self.engine.pause();
            self.is_playing = false;
            self.status = "Paused".into();
        } else if self.duration <= 0.0 && !self.playlist.is_empty() {
            let idx = self.current_index.unwrap_or(0).min(self.playlist.len() - 1);
            self.play_index(idx);
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
                    if ["mp3", "wav", "flac", "ogg", "m4a", "aac", "opus"].contains(&ext.as_str()) {
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

    fn filtered_indices(&self) -> Vec<usize> {
        let query = self.search_query.to_lowercase();
        self.playlist
            .iter()
            .enumerate()
            .filter(|(_, item)| {
                query.is_empty()
                    || item.title.to_lowercase().contains(&query)
                    || item.artist.to_lowercase().contains(&query)
            })
            .map(|(i, _)| i)
            .collect()
    }

    fn handle_keyboard_shortcuts(&mut self, ctx: &egui::Context) {
        if ctx.egui_wants_keyboard_input() {
            return;
        }

        let mut open_eq = false;
        let mut toggle_play = false;
        let mut move_up = false;
        let mut move_down = false;
        let mut move_home = false;
        let mut move_end = false;
        let mut play_selected = false;
        let mut delete_selected = false;
        let mut seek_delta: Option<f64> = None;

        ctx.input(|i| {
            if i.modifiers.command && i.key_pressed(egui::Key::E) {
                open_eq = true;
            }
            if i.key_pressed(egui::Key::Space) {
                toggle_play = true;
            }
            if i.key_pressed(egui::Key::ArrowUp) {
                move_up = true;
            }
            if i.key_pressed(egui::Key::ArrowDown) {
                move_down = true;
            }
            if i.key_pressed(egui::Key::Home) {
                move_home = true;
            }
            if i.key_pressed(egui::Key::End) {
                move_end = true;
            }
            if i.key_pressed(egui::Key::Enter) {
                play_selected = true;
            }
            if i.key_pressed(egui::Key::Delete) {
                delete_selected = true;
            }
            if i.key_pressed(egui::Key::ArrowRight) {
                seek_delta = Some(5.0);
            }
            if i.key_pressed(egui::Key::ArrowLeft) {
                seek_delta = Some(-5.0);
            }
        });

        if open_eq {
            self.show_eq_window = !self.show_eq_window;
        }
        if toggle_play {
            self.toggle_play_pause();
        }
        if let Some(delta) = seek_delta {
            if self.duration > 0.0 {
                let target = (self.seek_pos + delta).clamp(0.0, self.duration);
                let _ = self.engine.seek_position(target);
                self.seek_pos = target;
            }
        }

        let visible = self.filtered_indices();
        if visible.is_empty() {
            return;
        }

        if move_up {
            let base = self.selected_index.or(self.current_index);
            let next_sel = match base {
                Some(curr) => {
                    if let Some(pos) = visible.iter().position(|&idx| idx == curr) {
                        visible[pos.saturating_sub(1)]
                    } else {
                        visible[0]
                    }
                }
                None => visible[0],
            };
            self.selected_index = Some(next_sel);
            self.scroll_to_selected = true;
        }

        if move_down {
            let base = self.selected_index.or(self.current_index);
            let next_sel = match base {
                Some(curr) => {
                    if let Some(pos) = visible.iter().position(|&idx| idx == curr) {
                        visible[(pos + 1).min(visible.len() - 1)]
                    } else {
                        visible[0]
                    }
                }
                None => visible[0],
            };
            self.selected_index = Some(next_sel);
            self.scroll_to_selected = true;
        }

        if move_home {
            self.selected_index = visible.first().copied();
            self.scroll_to_selected = true;
        }

        if move_end {
            self.selected_index = visible.last().copied();
            self.scroll_to_selected = true;
        }

        if play_selected {
            let target = self
                .selected_index
                .or(self.current_index)
                .or_else(|| visible.first().copied());
            if let Some(idx) = target {
                self.play_index(idx);
            }
        }

        if delete_selected {
            if let Some(rem_i) = self.selected_index {
                if rem_i < self.playlist.len() {
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
                    if self.playlist.is_empty() {
                        self.selected_index = None;
                    } else {
                        self.selected_index = Some(rem_i.min(self.playlist.len() - 1));
                    }
                }
            }
        }
    }

    fn draw_equalizer_window(&mut self, ctx: &egui::Context) {
        if !self.show_eq_window {
            return;
        }

        let viewport_id = egui::ViewportId::from_hash_of("equalizer_dsp_viewport");
        let builder = egui::ViewportBuilder::default()
            .with_title("Equalizer & DSP Studio — Audio Engine V9")
            .with_inner_size([740.0, 520.0])
            .with_min_inner_size([620.0, 460.0]);

        ctx.show_viewport_immediate(viewport_id, builder, |ui, class| {
            if class == egui::ViewportClass::EmbeddedWindow {
                let mut open = self.show_eq_window;
                egui::Window::new(format!("{} Equalizer & DSP Studio", ph::SLIDERS))
                    .open(&mut open)
                    .default_width(680.0)
                    .show(ui.ctx(), |ui| {
                        self.render_eq_contents(ui);
                    });
                self.show_eq_window = open;
            } else {
                if ui.ctx().input(|i| i.viewport().close_requested()) {
                    self.show_eq_window = false;
                }
                egui::CentralPanel::default().show(ui, |ui| {
                    let max_w = ui.available_width();
                    ui.set_max_width(max_w);
                    ui.add_space(4.0);
                    self.render_eq_contents(ui);
                });
            }
        });
    }

    fn render_eq_contents(&mut self, ui: &mut egui::Ui) {
        // ─── CARD 1: PRESETS ───
        Card::new()
            .heading(format!("{} Studio Presets", ph::SPARKLE))
            .show(ui, |ui| {
                ui.horizontal_wrapped(|ui| {
                    for preset in ALL_PRESETS {
                        let is_active = self.active_preset == Some(preset);
                        let mut btn = Button::new(preset.to_string()).size(ButtonSize::Medium);
                        if is_active {
                            btn = btn.accent(Accent::Blue);
                        } else {
                            btn = btn.outline();
                        }
                        if ui.add(btn).clicked() {
                            self.engine.set_preset(preset);
                            self.active_preset = Some(preset);
                            self.bass_boost_db = match preset {
                                Preset::BassBoosted => 8.0,
                                Preset::ClubMaster => 9.0,
                                _ => 0.0,
                            };
                            self.status = format!("EQ Preset: {preset}");
                        }
                    }

                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui
                            .add(
                                Button::new(format!("{} Reset Flat", ph::ARROW_COUNTER_CLOCKWISE))
                                    .outline(),
                            )
                            .clicked()
                        {
                            self.engine.reset_eq();
                            self.active_preset = Some(Preset::Flat);
                            self.bass_boost_db = 0.0;
                            self.balance_pan = 0.0;
                            self.engine.set_balance(0.0);
                            self.status = "EQ Reset to Flat (0 dB)".into();
                        }
                    });
                });
            });

        ui.add_space(8.0);

        // ─── CARD 2: 10-BAND GRAPHIC EQUALIZER ───
        Card::new()
            .heading(format!(
                "{} 10-Band Graphic Equalizer (-12 dB to +12 dB)",
                ph::SLIDERS
            ))
            .show(ui, |ui| {
                let current_bands = *self.engine.get_eq_bands();
                let avail_w = ui.available_width();
                let col_w = (avail_w / 10.0 - 8.0).max(42.0);

                ui.horizontal(|ui| {
                    for (i, &freq) in EQ_FREQUENCIES.iter().enumerate() {
                        let mut gain = current_bands[i] as f32;
                        ui.vertical(|ui| {
                            ui.set_width(col_w);
                            ui.vertical_centered(|ui| {
                                let db_color = if gain > 0.05 {
                                    egui::Color32::from_rgb(56, 189, 248)
                                } else if gain < -0.05 {
                                    egui::Color32::from_rgb(248, 113, 113)
                                } else {
                                    egui::Color32::GRAY
                                };
                                ui.label(
                                    egui::RichText::new(format!("{gain:+.1}"))
                                        .monospace()
                                        .small()
                                        .color(db_color),
                                );

                               let resp = vertical_eq_slider(ui, &mut gain, 155.0);
                                if resp.double_clicked() {
                                    gain = 0.0;
                                    self.engine.set_eq_band(i, 0.0);
                                    self.active_preset = None;
                                } else if resp.changed() {
                                    self.engine.set_eq_band(i, gain as f64);
                                    self.active_preset = None;
                                }

                                let freq_str = if freq >= 1000 {
                                    format!("{:.1}k", freq as f32 / 1000.0).replace(".0k", "k")
                                } else {
                                    format!("{freq}")
                                };
                                ui.label(
                                    egui::RichText::new(format!("{freq_str}Hz"))
                                        .monospace()
                                        .small()
                                        .weak(),
                                );
                            });
                        });
                    }
                });
            });

        ui.add_space(8.0);

        // ─── CARD 3: DSP ENHANCERS (BASS BOOST, BALANCE, SMART HEADROOM) ───
        Card::new()
            .heading(format!("{} DSP Enhancers & Master Stage", ph::SPEAKER_HIGH))
            .show(ui, |ui| {
                ui.columns(2, |cols| {
                    // Column 1: Bass Boost
                    cols[0].vertical(|ui| {
                        ui.horizontal(|ui| {
                            ui.label(egui::RichText::new("Sub-Bass Boost").strong());
                            ui.add(Badge::new(
                                format!("+{:.1} dB", self.bass_boost_db),
                                if self.bass_boost_db > 0.0 {
                                    BadgeTone::Ok
                                } else {
                                    BadgeTone::Neutral
                                },
                            ));
                        });
                        ui.add_space(4.0);

                        let mut bass = self.bass_boost_db as f32;
                        let w = (ui.available_width() - 20.0).max(120.0);
                        if ui
                            .add(
                                Slider::new(&mut bass, 0.0..=12.0)
                                .accent(Accent::Green)
                                .desired_width(w)
                                .show_value(false),
                            )
                            .changed()

                        {
                            self.bass_boost_db = bass as f64;
                            self.engine.set_bass_boost(self.bass_boost_db);
                            self.active_preset = None;
                        }
                    });

                    // Column 2: Stereo Balance / Pan
                    cols[1].vertical(|ui| {
                        let pan_label = if self.balance_pan.abs() < 0.02 {
                            "Center".to_string()
                        } else if self.balance_pan < 0.0 {
                            format!("L {:.0}%", -self.balance_pan * 100.0)
                        } else {
                            format!("R {:.0}%", self.balance_pan * 100.0)
                        };
                        ui.horizontal(|ui| {
                            ui.label(egui::RichText::new("Stereo Balance (L/R)").strong());
                            ui.add(Badge::new(pan_label, BadgeTone::Info));
                            if self.balance_pan.abs() > 0.01 {
                                if ui.small_button("Center").clicked() {
                                    self.balance_pan = 0.0;
                                    self.engine.set_balance(0.0);
                                }
                            }
                        });
                        ui.add_space(4.0);

                        let mut pan = self.balance_pan;
                        let w = (ui.available_width() - 20.0).max(120.0);
                        let resp = ui.add(
                            Slider::new(&mut pan, -1.0..=1.0)
                            .accent(Accent::Blue)
                            .desired_width(w)
                            .show_value(false),
                        );
                        if resp.double_clicked() {
                            self.balance_pan = 0.0;
                            self.engine.set_balance(0.0);
                        } else if resp.changed() {
                            self.balance_pan = pan;
                            self.engine.set_balance(self.balance_pan);
                        }
                    });
                });

                ui.add_space(6.0);
                ui.separator();
                ui.add_space(4.0);

                ui.horizontal(|ui| {
                    if ui
                        .checkbox(
                            &mut self.auto_headroom,
                            "Smart Anti-Clipping Pre-Gain (Auto Headroom)",
                        )
                        .changed()
                    {
                        self.engine.set_auto_headroom(self.auto_headroom);
                    }

                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        let vol_pct = (self.engine.get_volume() * 100.0) as i32;
                        let mute_str = if self.engine.is_muted() {
                            " (MUTED)"
                        } else {
                            ""
                        };
                        ui.label(
                            egui::RichText::new(format!("Master Output: {vol_pct}%{mute_str}"))
                                .monospace()
                                .small()
                                .weak(),
                        );
                    });
                });
            });
    }
}

fn vertical_eq_slider(ui: &mut egui::Ui, value: &mut f32, height: f32) -> egui::Response {
    let theme = Theme::current(ui.ctx());
    let p = &theme.palette;

    let track_w = 6.0;
    let thumb_d = 14.0;
    let width = thumb_d + 8.0;

    let (rect, mut response) =
    ui.allocate_exact_size(egui::vec2(width, height), egui::Sense::click_and_drag());

    let thumb_pad = thumb_d * 0.5;
    let track_top = rect.min.y + thumb_pad;
    let track_bottom = rect.max.y - thumb_pad;
    let track_span = (track_bottom - track_top).max(1.0);
    let track_x = rect.center().x;

    let track_rect = egui::Rect::from_min_max(
        egui::pos2(track_x - track_w * 0.5, track_top),
                                              egui::pos2(track_x + track_w * 0.5, track_bottom),
    );

    if response.is_pointer_button_down_on() {
        if let Some(pos) = response.interact_pointer_pos() {
            let clamped_y = pos.y.clamp(track_top, track_bottom);
            // למעלה = +12dB, למטה = -12dB
            let frac = 1.0 - ((clamped_y - track_top) / track_span).clamp(0.0, 1.0);
            let new_val = (-12.0 + frac * 24.0).clamp(-12.0, 12.0);
            if (new_val - *value).abs() > 0.01 {
                *value = new_val;
                response.mark_changed();
            }
        }
    }

    if response.hovered() {
        ui.ctx().set_cursor_icon(egui::CursorIcon::Grab);
    }
    if response.is_pointer_button_down_on() {
        ui.ctx().set_cursor_icon(egui::CursorIcon::Grabbing);
    }

    let frac = ((*value - (-12.0)) / 24.0).clamp(0.0, 1.0);
    let thumb_y = track_bottom - track_span * frac;
    let center_y = (track_top + track_bottom) * 0.5;
    let thumb_center = egui::pos2(track_x, thumb_y);

    let painter = ui.painter();
    let track_radius = egui::CornerRadius::same(3);

    // 1. פס רקע בסגנון Elegance
    painter.rect(
        track_rect,
        track_radius,
        p.input_bg,
        egui::Stroke::new(1.0, p.border),
                 egui::StrokeKind::Inside,
    );

    // 2. קו סימון עדין של 0dB באמצע
    painter.line_segment(
        [
            egui::pos2(track_x - 6.0, center_y),
                         egui::pos2(track_x + 6.0, center_y),
        ],
        egui::Stroke::new(1.0, p.border),
    );

    // 3. צבע מילוי דינמי (כחול/תכלת להגברה, אדום/כתום להנחתה)
    let fill_color = if *value >= 0.0 {
        p.blue
    } else {
        egui::Color32::from_rgb(248, 113, 113)
    };

    // מילוי מהאמצע (0dB) לכיוון הידית או מתחתית הסליידר
    let fill_top = thumb_y.min(center_y);
    let fill_bottom = thumb_y.max(center_y);
    if (fill_bottom - fill_top) > 1.0 {
        let fill_rect = egui::Rect::from_min_max(
            egui::pos2(track_rect.min.x, fill_top),
                                                 egui::pos2(track_rect.max.x, fill_bottom),
        );
        painter.rect_filled(fill_rect, track_radius, fill_color);
    }

    // 4. ידית עגולה עם הילה (Halo) וטבעת צבעונית כמו ב-elegance::Slider
    let active = response.hovered() || response.is_pointer_button_down_on();
    if active {
        painter.circle_filled(
            thumb_center,
            thumb_d * 0.5 + 4.0,
            fill_color.linear_multiply(0.25),
        );
    }
    painter.circle(
        thumb_center,
        thumb_d * 0.5,
        p.text,
        egui::Stroke::new(2.0, fill_color),
    );

    response
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
        self.handle_keyboard_shortcuts(ui.ctx());
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
                    menu::MenuAction::SetPreset(preset) => {
                        self.engine.set_preset(preset);
                        self.active_preset = Some(preset);
                        self.bass_boost_db = match preset {
                            Preset::BassBoosted => 8.0,
                            Preset::ClubMaster => 9.0,
                            _ => 0.0,
                        };
                        self.status = format!("EQ Preset: {preset}");
                    }
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
                        self.selected_index = None;
                        let _ = self.engine.stop();
                        self.is_playing = false;
                        self.status = "Playlist Cleared".into();
                    }
                    menu::MenuAction::Quit => ctx.send_viewport_cmd(egui::ViewportCommand::Close),
                    menu::MenuAction::TogglePlayPause => self.toggle_play_pause(),
                    menu::MenuAction::NextTrack => self.next_track(),
                    menu::MenuAction::PrevTrack => self.prev_track(),
                    menu::MenuAction::OpenEqualizer => self.show_eq_window = true,
                    menu::MenuAction::OpenAbout => self.show_about_dialog = true,
                }
            }
        });

        // 2. סטטוס בר תחתון
                // 2. סטטוס בר תחתון
        egui::Panel::bottom("bottom_status_bar").show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.label(
                    egui::RichText::new(format!("{} {}", ph::ACTIVITY, self.status))
                        .small()
                        .weak(),
                );

                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    // 1. נורת LED ירוקה מהבהבת (Pulse Glow)
                    let t = ui.input(|i| i.time) as f32;
                    let speed = if self.is_playing { 4.5 } else { 1.8 };
                    let pulse = ((t * speed).sin() * 0.5 + 0.5).clamp(0.15, 1.0);
                    let (led_rect, _) =
                        ui.allocate_exact_size(egui::vec2(14.0, 14.0), egui::Sense::hover());
                    let painter = ui.painter();
                    let center = led_rect.center();
                    let green_led = egui::Color32::from_rgb(34, 197, 94);

                    // הילה חיצונית זוהרת שפועמת
                    painter.circle_filled(
                        center,
                        3.5 + pulse * 3.5,
                        green_led.linear_multiply(0.35 * pulse),
                    );
                    // ליבת ה-LED הירוקה
                    painter.circle_filled(
                        center,
                        4.0,
                        green_led.linear_multiply(0.45 + 0.55 * pulse),
                    );

                    ui.add_space(4.0);

                    // 2. כיתוב צבעוני בולט: GStreamer | BASS | DOLBY ATMOS
                    ui.label(
                        egui::RichText::new("DOLBY ATMOS")
                            .small()
                            .strong()
                            .color(egui::Color32::from_rgb(56, 189, 248)), // תכלת זוהר
                    );
                    ui.label(egui::RichText::new("•").small().weak());
                    ui.label(
                        egui::RichText::new("BASS")
                            .small()
                            .strong()
                            .color(egui::Color32::from_rgb(250, 204, 21)), // זהב/צהוב בולט
                    );
                    ui.label(egui::RichText::new("•").small().weak());
                    ui.label(
                        egui::RichText::new("GStreamer")
                            .small()
                            .strong()
                            .color(egui::Color32::from_rgb(74, 222, 128)), // ירוק מנטה
                    );

                    ui.separator();

                    // 3. מספר שירים + מצבי Repeat/Shuffle
                    let repeat_tag = if self.repeat_mode { " [Repeat ON]" } else { "" };
                    let shuffle_tag = if self.shuffle_mode {
                        " [Shuffle ON]"
                    } else {
                        ""
                    };
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
            // נועלים את הרוחב המקסימלי לרוחב החלון האמיתי כדי ששום רכיב לא ימתח את הכרטיסיות החוצה
            let max_panel_width = ui.available_width();
            ui.set_max_width(max_panel_width);

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

                            let play_icon = if self.is_playing { ph::PAUSE } else { ph::PLAY };
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

                            // כפתור פתיחת חלון אקולייזר נפרד
                            let mut eq_btn = Button::new(ph::SLIDERS).outline();
                            if self.show_eq_window {
                                eq_btn = eq_btn.accent(Accent::Blue);
                            }
                            if ui
                                .add(eq_btn)
                                .on_hover_text("Open Equalizer & DSP Studio (Ctrl+E)")
                                .clicked()
                            {
                                self.show_eq_window = !self.show_eq_window;
                            }
                        });

                        ui.add_space(12.0);

                        // ציר זמן (Timeline) עם קריאת זמנים משני הצדדים
                        // מפחיתים 310 פיקסלים עבור הזמנים, כפתור ההשתקה, סליידר הווליום, האחוזים והשוליים
                        let timeline_width = (ui.available_width() - 310.0).max(60.0);
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
                                let slider = Slider::new(&mut pos, 0.0..=max)
                                .accent(Accent::Blue)
                                .desired_width(timeline_width)
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
                            let vol_slider = Slider::new(&mut vol, 0.0..=1.5)
                            .accent(Accent::Green)
                            .desired_width(75.0)
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
                    // משאירים 12 פיקסלים מרווח שלא יחרוג מה-Card
                    let avail_width = (ui.available_width() - 12.0).max(100.0);
                    let bar_width =
                        ((avail_width - (bar_count - 1) as f32 * gap) / bar_count as f32).max(1.5);
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
                                    .add_filter(
                                        "Audio",
                                        &["mp3", "wav", "flac", "ogg", "m4a", "aac", "opus"],
                                    )
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
                                ui.label(
                                    egui::RichText::new(format!(
                                        "{} Drag & Drop your audio files here",
                                        ph::FILE_ARROW_DOWN
                                    ))
                                    .size(16.0)
                                    .weak(),
                                );
                                ui.label(
                                    egui::RichText::new("or click 'Add Files' above")
                                        .small()
                                        .weak(),
                                );
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
                                    let is_selected = self.selected_index == Some(i);
                                    let icon = if is_current {
                                        if self.is_playing {
                                            ph::SPEAKER_HIGH
                                        } else {
                                            ph::PAUSE
                                        }
                                    } else if is_selected {
                                        ph::CARET_RIGHT
                                    } else {
                                        ph::MUSIC_NOTE
                                    };
                                    let tone = if is_current {
                                        BadgeTone::Ok
                                    } else if is_selected {
                                        BadgeTone::Info
                                    } else {
                                        BadgeTone::Neutral
                                    };

                                    ui.add(Badge::new(icon, tone));

                                    // רוחב מוגדר לטקסט השיר שלא יגלוש החוצה: משאיר מקום ל-X ולזמן
                                    let text_width = (ui.available_width() - 105.0).max(50.0);
                                    let label =
                                        format!("{:02}. {} — {}", i + 1, item.title, item.artist);

                                    let resp = ui.add_sized(
                                        [text_width, 24.0],
                                        egui::Button::new(label)
                                            .selected(is_current || is_selected)
                                            .truncate(),
                                    );
                                    if is_selected && self.scroll_to_selected {
                                        resp.scroll_to_me(Some(egui::Align::Center));
                                        self.scroll_to_selected = false;
                                    }
                                    if resp.clicked() {
                                        self.selected_index = Some(i);
                                    }
                                    if resp.double_clicked() {
                                        play_idx = Some(i);
                                    }

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

                                    // כפתור מחיקה וזמן בצד ימין
                                    if let Some(dur) = item.duration_sec {
                                        ui.label(
                                            egui::RichText::new(format_time(dur))
                                                .monospace()
                                                .small()
                                                .weak(),
                                        );
                                    }
                                    if ui
                                        .small_button(ph::X)
                                        .on_hover_text("Remove from playlist")
                                        .clicked()
                                    {
                                        remove_idx = Some(i);
                                    }
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

        // ─── ABOUT DIALOG MODAL ───
        let mut close_about = false;
        Modal::new("about_modal", &mut self.show_about_dialog)
            .heading("Audio-Engine V9")
            .show(&ctx, |ui| {
                ui.label(
                    egui::RichText::new("v0.3.0 - High Performance Audio Engine V9")
                        .small()
                        .weak(),
                );
                ui.separator();
                ui.add_space(8.0);
                ui.label(
                    egui::RichText::new("Shay Kadosh © 2000-2026")
                        .strong()
                        .size(16.0),
                );
                ui.add_space(4.0);
                ui.label("⚡ System Architect & Lead Developer");
                ui.label("⚡ LLVM Compiler Infrastructure");
                ui.label("⚡ Built with love 🥰 DEV-TEAM ™");
                ui.add_space(12.0);
                ui.separator();
                ui.horizontal_wrapped(|ui| {
                    ui.add(Badge::new("Native NVMe Speed", BadgeTone::Ok));
                    ui.add(Badge::new("Google Antigravity", BadgeTone::Info));
                    ui.add(Badge::new("Insanely fast", BadgeTone::Danger));
                });
                ui.add_space(8.0);
                if ui.add(Button::new("Close").accent(Accent::Blue)).clicked() {
                    close_about = true;
                }
            });
        if close_about {
            self.show_about_dialog = false;
        }

        // ─── SEPARATE WINDOW: EQUALIZER & DSP STUDIO ───
        self.draw_equalizer_window(&ctx);
    }
    fn on_exit(&mut self) {
        self.save_state();
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
