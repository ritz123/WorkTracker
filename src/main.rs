mod app;
mod categories;
mod ui;
mod dates;
mod markdown;
mod models;
mod storage;
mod theme;
mod updates;

fn main() -> eframe::Result<()> {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1100.0, 720.0])
            .with_title("Time Tracker"),
        ..Default::default()
    };
    eframe::run_native(
        "Time Tracker",
        options,
        Box::new(|cc| Ok(Box::new(app::WeeklyTrackerApp::new(cc)))),
    )
}