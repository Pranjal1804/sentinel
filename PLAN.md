# SENTINEL development plan

## Current environment

- Ubuntu 24.04.4 LTS (Noble), x86_64; kernel `7.0.0-38-generic`.
- Intel Core i5-1235U (12 logical CPUs); 15 GiB RAM, 4 GiB swap.
- Intel Alder Lake-UP3 Iris Xe GPU; KIOXIA 512 GB NVMe (476.9 GiB), ext4 root.
- Rust 1.93.1, Cargo 1.93.1, Node.js 20.20.0, npm 10.8.2.
- Readable `/proc`, `/sys/class/hwmon`, `/sys/class/net`, and CPU frequency interfaces. Thermal sensors are exposed by `coretemp`, `nvme`, `acpitz`, and other hwmon drivers. Wireless interface is `wlp1s0`.
- The host exposes Intel DRM `gt_act_freq_mhz`/`gt_cur_freq_mhz` (active and current clock); no GPU utilization file was present. Battery `BAT0` exposes capacity, charge state, energy, full/design energy, and cycle count. Physical NVMe counters are visible in `/proc/diskstats`.
- At inspection: `/` was 45% used (198 GiB of 468 GiB); `wlp1s0` was up, Docker bridges were down. The battery reported 100% and 36.174 Wh full energy against 43.312 Wh design capacity (~83.5%); Intel GPU current clock was 300 MHz and active clock 0 MHz. These are point-in-time observations, not health conclusions.
- Readable temperature interfaces included hwmon `coretemp`, `nvme`, and `acpitz`; readings varied by sensor (CPU package/core readings around 54–60 °C, NVMe 36.85 °C at inspection).
- `clamscan`, `yara`, `auditctl`, `osqueryi`, `rkhunter`, `chkrootkit`, `lynis`, `smartctl`, and `sensors` were not found on `PATH`.
- GTK3 and WebKitGTK 4.1 development libraries are installed. The application does not install any system packages.

## Delivery sequence

1. Phase 0: record environment, architecture, security boundaries, and a trackable task list.
2. Phase 1: create the Tauri 2 + React + TypeScript desktop shell and Rust backend boundary.
3. Phase 2: collect live CPU, memory, disk, network, and temperature data through `/proc`, `/sys`, and filesystem APIs; show source availability explicitly.
4. Next: process and network ownership, diagnostics, security detections, local persistence, correlation, alerts, and packaging.

## Measurement

Use live `/proc` and `/sys` data only. CPU utilization is calculated from consecutive `/proc/stat` samples. Memory uses `/proc/meminfo`; disk capacity uses `statvfs`; network counters use `/proc/net/dev`; temperature uses readable hwmon and thermal-zone sensors. The collector is polled once per second by default. Resource targets are engineering targets, not guarantees; report measured conditions and sampling limitations.

## Initial measurement (2026-11-06)

Warmed 30-second desktop sample on the final telemetry build: 0.22% of total host CPU (2.60% of one core across the 3-process app tree); main SENTINEL/GTK process 94 MiB PSS; full app tree including WebKit 260–263 MiB PSS. Summed RSS was 459–462 MiB and double-counts shared pages. The full UI process tree exceeds the 150 MiB collector memory target when applied to the entire app. The collector currently shares the main process with the GUI. This was an idle sample on the user's laptop, not a stress test.
