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

## Versions and releases

The version is the `version` field in `Cargo.toml`. The running app reports that same number, and the in-app update check compares it with the latest GitHub Release of this repository.

To publish a release:

1. Set `version` in `Cargo.toml` to the new number and commit that change on `main`.
2. Tag that commit with `v` plus the same number, then push the tag.

```bash
git tag v0.2.0
git push origin v0.2.0
```

`.github/workflows/release.yml` runs on that tag. It refuses the release if the tag and `Cargo.toml` disagree. When both platform builds pass, it opens a GitHub Release for the tag, writes the notes from the commits since the previous release, and attaches `WorkTracker` and `WorkTracker.exe`. Drafts and prereleases are not used, so the new release becomes the one the update check offers.

Data is stored under `~/.weekly-tracker/` (same `data.json` / `prefs.json` shape as the original). You can also import exported `.md` reports at any time via **Import** in the top bar (or **Settings → Storage**), choosing either **Merge (skip duplicates)** or **Replace date range**, and export reports to `.md` or vector `.pdf`.

## License

GPL-3.0-only.
