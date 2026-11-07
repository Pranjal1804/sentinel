# SENTINEL task status

## Delivered

- [x] Ubuntu host inspection and project plan/security/architecture docs.
- [x] Tauri 2 + React + TypeScript desktop foundation and SQLite schema.
- [x] Live CPU, memory, storage, network, temperature, battery, and available GPU metrics.
- [x] Procfs process inventory and contextual indicators.
- [x] Confirmation-based, current-user-only `SIGTERM` with PID identity checks.
- [x] TCP/UDP socket and listener inventory with process-owner mapping where permitted.
- [x] Read-only systemd failed-unit view.
- [x] `/etc` SHA-256 baseline and comparison initiated by the user.
- [x] Security-tool availability plus explicit ClamAV/YARA file/folder scan flows.
- [x] Optional ClamAV/YARA installation flow with confirmation and Ubuntu authorization.
- [x] Local scan result storage, output bounds, system target restrictions, scan timeout.
- [x] Current-observation incident view with no causal claims.
- [x] Nine visual themes and local settings.
- [x] Ubuntu packaging and user/system installation helpers.

## Known gaps

- [ ] Full process tree visualization, package ownership, and richer per-process open-file details.
- [ ] journald/kernel/package/crash diagnostics and authentication-event monitoring.
- [ ] Persistence monitoring and scheduled integrity checks.
- [ ] Long-lived alert/event correlation with review workflow; Incident Centre currently presents current observations.
- [ ] Optional integrations for auditd, osquery, rkhunter, chkrootkit, Lynis, smartmontools, and lm-sensors beyond presence reporting.
- [ ] Independent collector process, retention controls, and production-scale performance study.
- [ ] Broader security review, scanner integration tests using controlled fixtures, and Ubuntu 22.04 validation.
- [ ] Select and add an open-source license if redistribution is intended.
