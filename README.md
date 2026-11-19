# Work Tracker (Rust)

Native desktop port of the Electron [Time Tracker](https://github.com/ritz123/TimeTracker) app using **eframe/egui**, with its UI tokens defined in `src/theme.rs` and `src/ui/chrome.rs`.

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
| Windows | `.github/workflows/windows.yml` | `WorkTracker-setup.exe` |
| Linux | `.github/workflows/linux.yml` | `WorkTracker` |

The Linux job installs the X11, Wayland, and GTK libraries that eframe links against, then runs `cargo build --release`. The resulting file is a native executable for the runner's Ubuntu version.

## Versions and releases

A release is a git tag of the form `vMAJOR.MINOR.PATCH`. Pushing that tag is the version bump. The app reports that tag: `build.rs` reads it at compile time and strips the leading `v`. A build with no tag falls back to `version` in `Cargo.toml`. GitHub passes the pushed tag into the release build, so the published binaries report that version, then publishes a GitHub Release with both files attached.

```bash
./release.sh 0.2.0
```

`release.sh` writes that number into `Cargo.toml` and `Cargo.lock`, commits the change when the version actually moved, tags the commit `v0.2.0`, and pushes the branch and the tag. The working tree must be clean.

The tag has to point at a commit that contains `.github/workflows/release.yml`. The workflow accepts only a three-part numeric tag, so `v0.2.0` publishes and `v0.2` does not. The release notes are the commits since the previous release. The attached files are `WorkTracker-0.2.0-x86_64.AppImage` and `WorkTracker-0.2.0-setup.exe`. The setup installs Work Tracker for the current user and adds a Start menu shortcut. The AppImage needs `chmod +x` before it runs. The in-app update check reads the latest release of this repository and offers it when its tag is newer than the running build.

Data is stored under `~/.weekly-tracker/` (same `data.json` / `prefs.json` shape as the original). You can also import exported `.md` reports at any time via **Import** in the top bar (or **Settings → Storage**), choosing either **Merge (skip duplicates)** or **Replace date range**, and export reports to `.md` or vector `.pdf`.

## License

GPL-3.0-only.
