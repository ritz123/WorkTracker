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

fn main() -> eframe::Result<()> {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([DEFAULT_WINDOW_WIDTH, DEFAULT_WINDOW_HEIGHT])
            .with_min_inner_size([MIN_WINDOW_WIDTH, MIN_WINDOW_HEIGHT])
            .with_title("Time Tracker"),
        ..Default::default()
    };
    eframe::run_native(
        "Time Tracker",
        options,
        Box::new(|cc| Ok(Box::new(app::WeeklyTrackerApp::new(cc)))),
    )
}