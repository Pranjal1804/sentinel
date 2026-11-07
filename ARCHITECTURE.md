# Architecture

```text
Linux /proc, /sys, statvfs, systemd, network interfaces, /etc
                           |
                           v
Rust Tauri commands: telemetry, processes, sockets, services,
                     optional scanner runners, integrity hashing
                           |
                 typed JSON boundaries
                           |
                           v
React desktop UI: live health, investigations, local settings
                           |
                           v
                local SQLite application data
```

## Collection and UI

Collection is implemented in Rust and runs away from the UI rendering path. Tauri commands expose a fixed set of typed operations. The UI does not construct shell commands. Procfs process and socket inspections run only on the corresponding pages. Metrics unavailable from kernel/driver interfaces stay unavailable instead of being estimated.

## Optional scanner flow

The user opens a native file/folder picker, chooses ClamAV or YARA, then confirms the operation. Rust canonicalizes the selected target, rejects root and protected system trees, and calls a fixed executable with individual argv values. YARA requires a selected, bounded rule file. A worker drains bounded scanner output, enforces a time limit, and records a result in local SQLite. No scanner runs at startup.

Package installation is a separate explicit action: the user confirms the exact ClamAV/YARA apt operation, then `pkexec` asks Ubuntu to authorize it. This is not a general root shell and accepts no frontend-provided command or package name.

## Local persistence

SQLite is initialized under the Tauri app data directory. It stores the local file-integrity baseline, scan history, and action audit entries. User preferences such as theme and polling profile are stored in the local webview storage. No external telemetry service is configured.

## Known scope

The app currently displays process indicators, active sockets, failed systemd units, optional ClamAV/YARA scan output, and an `/etc` integrity comparison. It does not provide full journald ingestion, persistent event correlation, network reputation, persistence monitoring, package ownership, or a privileged helper. A current observation in Incident Centre is not a confirmed security incident.
