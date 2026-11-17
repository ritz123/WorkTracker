# Weekly Work Tracker (Rust)

Native desktop port of [TimeTracker-master](./TimeTracker-master/) using **eframe/egui**, with UI tokens from **ui-ux-pro-max** (see [design-system/weekly-work-tracker/MASTER.md](./design-system/weekly-work-tracker/MASTER.md)).

## Feature list

See [FEATURES.md](./FEATURES.md) for a full comparison with the Electron app.

## Build & run

```bash
cargo build --release
cargo run
```

Data is stored under `~/.weekly-tracker/` (same `data.json` / `prefs.json` shape as the original).

## License

GPL-2.0-only — same as the upstream Time Tracker project.