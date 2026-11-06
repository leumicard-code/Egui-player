use crate::audio::{ALL_PRESETS, Preset};
use eframe::egui;
use elegance::BuiltInTheme;
use std::path::PathBuf; // ייבוא הפריסטים של האודיו
/// אלו כל הפעולות שהתפריט יכול לבקש מהאפליקציה המרכזית לעשות
pub enum MenuAction {
    AddFiles(Vec<PathBuf>),
    AddFolder(PathBuf),
    ClearPlaylist,
    Quit,
    TogglePlayPause,
    NextTrack,
    PrevTrack,
    SetPreset(Preset),
    SetTheme(BuiltInTheme),
    OpenEqualizer,
    OpenAbout,
}

/// פונקציה שמציירת את התפריט העליון ומחזירה פעולה אם המשתמש לחץ על משהו
pub fn draw_menu(ui: &mut egui::Ui) -> Option<MenuAction> {
    let mut action = None;

    egui::MenuBar::new().ui(ui, |ui| {
        // ── תפריט File ──
        ui.menu_button("File", |ui| {
            if ui.button("Add files…").clicked() {
                if let Some(files) = rfd::FileDialog::new()
                    .add_filter("Audio", &["mp3", "wav", "flac", "ogg", "m4a"])
                    .pick_files()
                {
                    action = Some(MenuAction::AddFiles(files));
                }
                ui.close();
            }

            if ui.button("Add folder…").clicked() {
                if let Some(folder) = rfd::FileDialog::new().pick_folder() {
                    action = Some(MenuAction::AddFolder(folder));
                }
                ui.close();
            }

            ui.separator();

            if ui.button("Clear playlist").clicked() {
                action = Some(MenuAction::ClearPlaylist);
                ui.close();
            }

            ui.separator();

            if ui.button("Quit").clicked() {
                action = Some(MenuAction::Quit);
            }
        });

        // ── תפריט Playback ──
        ui.menu_button("Playback", |ui| {
            if ui.button("Play / Pause").clicked() {
                action = Some(MenuAction::TogglePlayPause);
                ui.close();
            }
            if ui.button("Next").clicked() {
                action = Some(MenuAction::NextTrack);
                ui.close();
            }
            if ui.button("Previous").clicked() {
                action = Some(MenuAction::PrevTrack);
                ui.close();
            }
        });

        // ── תפריט View (אקולייזר) ──
        ui.menu_button("View", |ui| {
            if ui.button("Equalizer & DSP Studio… (Ctrl+E)").clicked() {
                action = Some(MenuAction::OpenEqualizer);
                ui.close();
            }

            ui.separator();

            ui.label("EQ Presets:");
            for preset in ALL_PRESETS {
                if ui.button(format!("{preset}")).clicked() {
                    action = Some(MenuAction::SetPreset(preset));
                    ui.close();
                }
            }

            ui.separator();

            ui.menu_button("App Theme", |ui| {
                for theme in [
                    BuiltInTheme::Slate,
                    BuiltInTheme::Charcoal,
                    BuiltInTheme::Frost,
                    BuiltInTheme::Paper,
                ] {
                    // יוצר כפתור לכל אפשרות, למשל "Slate", "Frost"
                    if ui.button(format!("{theme:?}")).clicked() {
                        action = Some(MenuAction::SetTheme(theme));
                        ui.close();
                    }
                }
            });
        });

        // ── תפריט Help ──
        ui.menu_button("Help", |ui| {
            if ui.button("About Audio-Engine V9…").clicked() {
                action = Some(MenuAction::OpenAbout);
                ui.close();
            }
        });
    });

    action
}
