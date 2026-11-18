# Work Tracker (Rust)

Native desktop port of [TimeTracker-master](./TimeTracker-master/) using **eframe/egui**, with UI tokens from **ui-ux-pro-max** (see [design-system/weekly-work-tracker/MASTER.md](./design-system/weekly-work-tracker/MASTER.md)).

## Feature list

See [FEATURES.md](./FEATURES.md) for a full comparison with the Electron app.

## Build & run

```bash
cargo build --release
cargo run
```

## Windows build

GitHub Actions builds a Windows executable on every push to `main` (`.github/workflows/windows.yml`). After a run finishes, download `WorkTracker.exe` from that run's **Artifacts**. You can also start a build from the Actions tab with **Run workflow**.

Data is stored under `~/.weekly-tracker/` (same `data.json` / `prefs.json` shape as the original). You can also import exported `.md` reports at any time via **Import** in the top bar (or **Settings → Storage**), choosing either **Merge (skip duplicates)** or **Replace date range**, and export reports to `.md` or vector `.pdf`.

## License

GPL-2.0-only — same as the upstream Time Tracker project.