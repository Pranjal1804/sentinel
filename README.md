# SENTINEL

**Your System. Your Security. Your Command Centre.**

SENTINEL is a native Linux desktop application for local system telemetry, process and connection inspection, security-tool integrations, and system diagnostics. Built with **Tauri 2 · React · TypeScript · Rust** — no cloud, no telemetry, no root required.

> **Disclaimer:** SENTINEL is a personal project. It is not a certified EDR product, does not guarantee threat detection, and must not be treated as proof that a machine is clean.

---

## Features

| Page | What it does |
|---|---|
| **Command Centre** | Live CPU (per-core), RAM/swap, disk I/O, network throughput, thermals, GPU, battery |
| **Process Explorer** | `/proc` process table — owner, CPU, memory, indicators, optional SIGTERM |
| **Network Monitor** | TCP/UDP socket tables, listening ports, process owners |
| **System Diagnostics** | Failed systemd units |
| **Threat Detection** | Local process indicators and security-tool availability |
| **File Integrity** | SHA-256 baseline for `/etc` files — create and compare on demand |
| **Incident Centre** | Current processes, listeners, and service observations in one view |
| **Security Tools** | ClamAV and YARA scans of user-selected files/folders |
| **Settings** | 9 themes, telemetry polling profile |

Metrics show **unavailable** when the kernel does not expose a supported source — no data is fabricated.

---

## System requirements

| Requirement | Minimum |
|---|---|
| **OS** | Ubuntu 22.04 LTS or 24.04 LTS (x86\_64) |
| **Rust** | 1.77 or newer (install via rustup) |
| **Node.js** | 18 LTS or newer |
| **npm** | 9 or newer (bundled with Node 18+) |

> Other Rust-supported Linux distros may work but are not tested. GTK 3 and WebKitGTK 4.1 must be available as shared libraries.

---

## Install from a release package

If you just want to **run** SENTINEL (no build required):

1. Download the latest `SENTINEL_x.y.z_amd64.deb` from the [Releases](../../releases) page.

2. Install it:

   ```sh
   sudo apt install ./SENTINEL_x.y.z_amd64.deb
   ```

3. Launch from the Applications menu, or:

   ```sh
   sentinel
   ```

**Uninstall:**

```sh
sudo apt remove sentinel
# Optional: remove local app data
rm -rf ~/.local/share/io.sentinel.desktop
```

---

## Build from source (step-by-step)

### 1 — Install Rust

```sh
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
# follow the prompts, then reload your shell
source "$HOME/.cargo/env"
rustc --version   # should print 1.77 or newer
```

### 2 — Install Node.js 18+

```sh
# Using the official NodeSource setup (Ubuntu)
curl -fsSL https://deb.nodesource.com/setup_20.x | sudo -E bash -
sudo apt-get install -y nodejs
node --version    # should print v20.x.x
npm --version     # should print 9.x or newer
```

### 3 — Install Linux build libraries

These are required by Tauri to compile the native desktop app:

```sh
sudo apt-get update
sudo apt-get install -y \
  build-essential curl wget file \
  libssl-dev libsqlite3-dev \
  libayatana-appindicator3-dev \
  librsvg2-dev \
  libwebkit2gtk-4.1-dev \
  libgtk-3-dev \
  pkg-config
```

> You can also run the interactive bootstrap script which checks versions and asks before installing anything:
> ```sh
> bash scripts/bootstrap-ubuntu.sh
> ```

### 4 — Clone and install JS dependencies

```sh
git clone https://github.com/Pranjal1804/sentinel.git
cd sentinel
npm ci
```

### 5 — Run in development mode

```sh
npm run tauri dev
```

The app window will open. Hot-reload is active for the React frontend; Rust changes require a restart.

### 6 — Build a release package

```sh
npm run tauri build
```

The `.deb` package is written to:

```
src-tauri/target/release/bundle/deb/SENTINEL_x.y.z_amd64.deb
```

Install it with `sudo apt install ./path/to/file.deb`.

---

## Verify build dependencies

```sh
bash scripts/check-dependencies.sh
```

This prints OK/MISS for each required tool and library without modifying anything.

---

## Optional security tools (ClamAV & YARA)

ClamAV and YARA are **not** required to build or run SENTINEL. They unlock the Security Tools scan page.

### Install via SENTINEL UI

Open **Security Tools** in the sidebar → click **INSTALL CLAMAV + YARA**. Ubuntu will prompt for your administrator password via `pkexec`.

### Install manually

```sh
sudo apt install clamav yara
```

### Update ClamAV signatures

ClamAV needs current virus definitions before it can detect anything:

```sh
sudo systemctl stop clamav-freshclam   # stop auto-updater if running
sudo freshclam                          # pull latest definitions
sudo systemctl start clamav-freshclam  # re-enable auto-updater
```

> SENTINEL does not update signatures automatically. Run `freshclam` periodically or enable the `clamav-freshclam` systemd service.

### Using YARA

YARA scans require a `.yar` or `.yara` rule file. SENTINEL's file picker will ask you to select one before each scan. Public YARA rule sets include [YARA-Rules](https://github.com/Yara-Rules/rules) and [Neo23x0/signature-base](https://github.com/Neo23x0/signature-base).

Both engines can produce false positives — inspect any detection and its evidence before taking action.

---

## Themes

Nine built-in themes selectable in **Settings**:

| Key | Name | Accent |
|---|---|---|
| `sentinel` | Sentinel Green | `#b4f34d` |
| `arctic` | Arctic Cyan | `#64e8ef` |
| `ember` | Ember Amber | `#ffb35c` |
| `violet` | Ultraviolet | `#bd8cff` |
| `rose` | Neon Rose | `#ff75b5` |
| `jade` | Deep Jade | `#51e0ae` |
| `ocean` | Abyssal Blue | `#55b9ff` |
| `solar` | Solar Flare | `#ffd166` |
| `mono` | Carbon Mono | `#e1e6e4` |

---

## Data and permissions

- All telemetry is read from local kernel interfaces (`/proc`, `/sys`, network, systemd). **No data leaves the machine.**
- SENTINEL does not run as root. Some process and socket details may be hidden by Linux permissions.
- Scan results and history are saved to `~/.local/share/io.sentinel.desktop/sentinel.sqlite` only.
- Scan targets are restricted to user files and directories. System-wide paths (`/`, `/etc`, `/usr`, `/var`, `/proc`, `/sys`, `/dev`, `/boot`, `/run`) are blocked.
- Process termination (SIGTERM) is explicit, confirmation-gated, and limited to processes owned by the current user.
- Scanner installation (`pkexec apt-get install`) only runs after you confirm in the UI.

---

## Project layout

```text
src/
  main.tsx                Entry point
  ui/
    App.tsx               Shell, routing, theme management
    Pages.tsx             All page components
    styles.css            Base design system
    themes.css            Extra theme overrides
    types.ts              Shared TypeScript types

src-tauri/
  src/lib.rs              All Rust commands (telemetry, processes, scanners, …)
  src/main.rs             Tauri entry point
  migrations/             SQLite schema migrations
  capabilities/           Tauri window permission declarations
  icons/                  App icon assets
  Cargo.toml              Rust manifest
  tauri.conf.json         Tauri build configuration

scripts/
  bootstrap-ubuntu.sh     Interactive dependency installer
  check-dependencies.sh   Non-destructive dependency checker
  run-dev.sh              Dev server helper
  build-release.sh        Release build helper
  install.sh              Post-build install helper
  uninstall.sh            Uninstall helper
  measure-overhead.py     CPU/RAM overhead measurement script
```

---

## Troubleshooting

| Problem | Fix |
|---|---|
| `webkit2gtk-4.1 not found` | `sudo apt install libwebkit2gtk-4.1-dev` |
| `libayatana-appindicator3` missing | `sudo apt install libayatana-appindicator3-dev` |
| Blank window on launch | Ensure WebKitGTK 4.1 is installed; check `~/.local/share/io.sentinel.desktop/` for logs |
| ClamAV returns "No signatures" | Run `sudo freshclam` to download virus definitions |
| YARA scan fails | Ensure the rule file ends in `.yar` or `.yara` and is under 2 MiB |
| Process list shows partial data | Expected — SENTINEL doesn't run as root; some `/proc` entries require elevated access |
| `cargo: command not found` | Run `source "$HOME/.cargo/env"` or open a new terminal after rustup install |

---

## See also

- [SECURITY.md](SECURITY.md) — security model and boundaries
- [ARCHITECTURE.md](ARCHITECTURE.md) — component diagram and data flow
- [TASKS.md](TASKS.md) — current scope and known gaps

---

## License

No license has been selected yet. Until one is added, all rights are reserved — do not redistribute or modify without permission.
