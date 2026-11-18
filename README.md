# Work Tracker (Rust)

Native desktop port of [TimeTracker-master](./TimeTracker-master/) using **eframe/egui**, with UI tokens from **ui-ux-pro-max** (see [design-system/weekly-work-tracker/MASTER.md](./design-system/weekly-work-tracker/MASTER.md)).

## Feature list

See [FEATURES.md](./FEATURES.md) for a full comparison with the Electron app.

## Build & run

```bash
cargo build --release
cargo run
```

## GitHub builds

Each push to `main` builds the app on GitHub Actions. The same workflows also run on pull requests, and you can start one from the Actions tab with **Run workflow**. When a run finishes, download the binary from that run's **Artifacts**.

| Platform | Workflow | Artifact |
| --- | --- | --- |
| Windows | `.github/workflows/windows.yml` | `WorkTracker.exe` |
| Linux | `.github/workflows/linux.yml` | `WorkTracker` |

The Linux job installs the X11, Wayland, and GTK libraries that eframe links against, then runs `cargo build --release`. The resulting file is a native executable for the runner's Ubuntu version.

Data is stored under `~/.weekly-tracker/` (same `data.json` / `prefs.json` shape as the original). You can also import exported `.md` reports at any time via **Import** in the top bar (or **Settings → Storage**), choosing either **Merge (skip duplicates)** or **Replace date range**, and export reports to `.md` or vector `.pdf`.

## License

GPL-2.0-only — same as the upstream Time Tracker project.