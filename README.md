# Renamer

Renamer is a cross-platform anonymization tool for frontend products. It includes a reusable core, a CLI, and a lightweight desktop shell for macOS and Windows.

## What it anonymizes

- App names, keywords, bundle identifiers, Android package names, reverse-DNS identifiers, and other text values via repeated `FROM=TO` replacement rules.
- Launch, tray, favicon, and platform icon files by copying one replacement image over known icon file names.
- Web, iOS, and Android project trees while skipping common generated or dependency directories such as `.git`, `node_modules`, `target`, `build`, and `DerivedData`.

## First-time setup: install Rust and run Renamer

If your local machine has no Rust tooling installed yet, use one of the bootstrap scripts below. They install Rust with `rustup`, make `cargo` available, clone/build the project if needed, and start the desktop app.

> The scripts use the official Rust installer at `https://sh.rustup.rs` on macOS/Linux/WSL and `https://win.rustup.rs` on Windows. Review scripts before running them if your environment has stricter security requirements.

### macOS

Apple's linker tools are required for Rust builds. If `xcode-select` is missing, the script asks macOS to install Command Line Tools first.

```bash
#!/usr/bin/env bash
set -euo pipefail

if ! xcode-select -p >/dev/null 2>&1; then
  echo "Installing Apple Command Line Tools. Re-run this script after the installer finishes."
  xcode-select --install
  exit 1
fi

if ! command -v cargo >/dev/null 2>&1; then
  curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
  . "$HOME/.cargo/env"
fi

cargo --version
cargo run --bin renamer-desktop
```

### Linux or WSL

Install the native build prerequisites first, then install Rust and launch the desktop shell:

```bash
#!/usr/bin/env bash
set -euo pipefail

if command -v apt-get >/dev/null 2>&1; then
  sudo apt-get update
  sudo apt-get install -y build-essential curl pkg-config
elif command -v dnf >/dev/null 2>&1; then
  sudo dnf install -y gcc gcc-c++ make curl pkg-config
elif command -v pacman >/dev/null 2>&1; then
  sudo pacman -Sy --needed base-devel curl pkg-config
fi

if ! command -v cargo >/dev/null 2>&1; then
  curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
  . "$HOME/.cargo/env"
fi

cargo --version
cargo run --bin renamer-desktop
```

### Windows PowerShell

Run PowerShell as your normal user. If `cargo` is not installed, the script downloads and starts `rustup-init.exe`. After the Rust installer completes, open a new PowerShell window or let the script add Cargo to the current session path.

```powershell
$ErrorActionPreference = "Stop"

if (-not (Get-Command cargo -ErrorAction SilentlyContinue)) {
    $rustup = Join-Path $env:TEMP "rustup-init.exe"
    Invoke-WebRequest -Uri "https://win.rustup.rs/x86_64" -OutFile $rustup
    & $rustup -y
    $cargoBin = Join-Path $env:USERPROFILE ".cargo\bin"
    $env:Path = "$cargoBin;$env:Path"
}

cargo --version
cargo run --bin renamer-desktop
```

### Run from a freshly cloned checkout

If you are starting from an empty folder, clone the repository first and then run the same Cargo commands:

```bash
git clone <REPOSITORY_URL> renamer
cd renamer
cargo test
cargo run --bin renamer-desktop
```

For Windows PowerShell:

```powershell
git clone <REPOSITORY_URL> renamer
Set-Location renamer
cargo test
cargo run --bin renamer-desktop
```

Replace `<REPOSITORY_URL>` with the real Git URL for this repository.

## Desktop app

Build and run the universal desktop shell:

```bash
cargo run --bin renamer-desktop
```

The desktop binary starts a local UI in the system browser so it works without external GUI dependencies on macOS and Windows. From the UI a user can create a new anonymization project, choose or paste the project folder, enable/disable icon anonymization, enable/disable name and string anonymization, provide a new package name, and preview/apply the changes. If the new package field is empty, Renamer uses the built-in default package `app.anonymized.default`. The neutral default PNG icon is generated deterministically from Rust code at runtime, so the repository stays text-only and PR-friendly.

## CLI usage

Dry-run is the default so you can review the planned changes before writing files:

```bash
cargo run --bin renamer -- \
  --path ./project \
  --replace com.old.brand=com.neutral.app \
  --replace OldBrand=NeutralApp \
  --icon-source ./neutral-icon.png
```

Apply the same changes:

```bash
cargo run --bin renamer -- \
  --path ./project \
  --replace com.old.brand=com.neutral.app \
  --replace OldBrand=NeutralApp \
  --icon-source ./neutral-icon.png \
  --apply
```

If you installed or copied the compiled binary onto your `PATH`, you can use `renamer ...` instead of `cargo run --bin renamer -- ...`.

By default, Renamer also renames matching file and directory paths, including package directories such as `com/example/app` to `io/neutral/app`. Add `--no-rename-paths` if you only want content and icon updates.

Add extra icon file names when a project uses custom naming:

```bash
cargo run --bin renamer -- --path ./project --icon-source ./neutral-icon.png --icon-name status-bar.png --apply
```

## Testing

- Unit tests cover text replacement, dry-run safety, icon replacement, package-directory path renaming, excluded directories, custom icon names, binary-file skipping, desktop form parsing, and desktop default package/icon behavior.
- CLI integration tests use an Android-like fixture to verify dry-run and apply behavior end to end.
- Manual Android smoke-test notes are in [`docs/android-smoke-test.md`](docs/android-smoke-test.md).

Run all tests:

```bash
cargo test
```

## Desktop roadmap

The desktop shell is intentionally dependency-light and reuses `src/lib.rs`. Next improvements can add:

1. Native packaged installers (`.app`/`.dmg` for macOS and `.msi`/`.exe` for Windows).
2. Persisted anonymization-project presets.
3. Per-file diff summaries before applying changes.
4. Rollback backup support.
5. Optional user-selected icon packs in addition to the generated neutral default.
