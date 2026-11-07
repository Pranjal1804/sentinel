# Security model

## Boundaries

- The desktop app runs as the logged-in user, not root. Rust owns operating-system reads and subprocess execution; the frontend cannot supply a shell command.
- The only process control is a confirmation-based `SIGTERM` request. The backend verifies PID start ticks to reduce PID-reuse risk, checks that the target is owned by the current user, and refuses PID 1, SENTINEL itself, and its ancestors. There is no `SIGKILL` fallback.
- Optional scans invoke fixed `/usr/bin/clamscan` and `/usr/bin/yara` paths using argument arrays. The user selects targets and confirms every scan. Root/system-wide paths are rejected. Output is capped at 256 KiB and scans stop after 15 minutes.
- YARA rules are selected by the user and limited to regular `.yar`/`.yara` files no larger than 2 MiB. A rule file can contain costly or misleading rules, so use rules from sources you trust.
- Installing optional ClamAV/YARA packages requires an in-app confirmation followed by Ubuntu's graphical administrator authorization. No package install occurs on launch.
- The file-integrity baseline only hashes a bounded set of regular files under `/etc` when the user requests it. Baseline writes and comparisons are local.
- The app makes no automatic network reputation queries and does not upload telemetry or scan results.

## Interpretation

Process path indicators, failed services, YARA/ClamAV matches, and file changes need context. A scanner match can be a false positive, and a scan with no matches does not prove the host is clean. Permission restrictions can hide data. SENTINEL is a personal monitoring project, not a certified endpoint security product.

## Reporting

Please report reproducible security issues privately to the repository owner rather than publishing sensitive exploit details. Do not include private keys, access tokens, full command lines containing secrets, or personal files in bug reports.
