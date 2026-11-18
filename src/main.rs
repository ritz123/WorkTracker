mod app;
mod categories;
mod ui;
mod dates;
mod markdown;
mod models;
mod pdf;
mod storage;
mod theme;
mod updates;

use crate::ui::chrome::{
    DEFAULT_WINDOW_HEIGHT, DEFAULT_WINDOW_WIDTH, MIN_WINDOW_HEIGHT, MIN_WINDOW_WIDTH,
};

/// Taskbar and Alt-Tab picture. The bytes are the 128px clock, raw RGBA with
/// no header. The Windows desktop shortcut uses the same picture from the
/// icon embedded in the executable by `build.rs`.
fn app_icon() -> egui::IconData {
    const SIZE: u32 = 128;
    const RGBA: &[u8] = include_bytes!("../packaging/icon/worktracker-128.rgba");
    const _: () = assert!(RGBA.len() == (SIZE as usize) * (SIZE as usize) * 4);
    egui::IconData {
        rgba: RGBA.to_vec(),
        width: SIZE,
        height: SIZE,
    }
}

fn main() -> eframe::Result<()> {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([DEFAULT_WINDOW_WIDTH, DEFAULT_WINDOW_HEIGHT])
            .with_min_inner_size([MIN_WINDOW_WIDTH, MIN_WINDOW_HEIGHT])
            .with_icon(app_icon())
            .with_title("Work Tracker")
            // The OS title bar, borders, and window buttons are off. The toolbar is the drag
            // surface and hosts Close; `undecorated_window_edges` restores edge resizing.
            .with_decorations(false),
        ..Default::default()
    };
    eframe::run_native(
        "Work Tracker",
        options,
        Box::new(|cc| Ok(Box::new(app::WeeklyTrackerApp::new(cc)))),
    )
}