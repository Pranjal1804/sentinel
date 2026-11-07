use libc::statvfs;
use serde::Serialize;
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::ffi::{CStr, CString};
use std::fs;
use std::io::Read;
use std::path::Path;
use std::process::{Command, Stdio};
use std::sync::{Mutex, OnceLock};
use std::time::{Instant, SystemTime, UNIX_EPOCH};
use tauri::Manager;

#[derive(Default)]
struct Previous {
    cpu: HashMap<String, (u64, u64)>,
    network: HashMap<String, (u64, u64, Instant)>,
    disk: HashMap<String, (u64, u64, Instant)>,
}

static PREVIOUS: OnceLock<Mutex<Previous>> = OnceLock::new();
static PROCESS_HISTORY: OnceLock<Mutex<HashMap<u32, (u64, Instant)>>> = OnceLock::new();

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
struct Metric<T> {
    value: Option<T>,
    source: String,
    available: bool,
    detail: Option<String>,
}

impl<T> Metric<T> {
    fn from_result(value: Result<T, String>, source: &str) -> Self {
        match value {
            Ok(value) => Self {
                value: Some(value),
                source: source.into(),
                available: true,
                detail: None,
            },
            Err(detail) => Self {
                value: None,
                source: source.into(),
                available: false,
                detail: Some(detail),
            },
        }
    }
}

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
struct NetworkInterface {
    name: String,
    addresses: Vec<String>,
    rx_bytes: u64,
    tx_bytes: u64,
    rx_bytes_per_sec: f64,
    tx_bytes_per_sec: f64,
    rx_packets: u64,
    tx_packets: u64,
    rx_errors: u64,
    tx_errors: u64,
    rx_dropped: u64,
    tx_dropped: u64,
    state: String,
}

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
struct CoreUsage {
    core: String,
    utilization_percent: f64,
}

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
struct ProcessInfo {
    pid: u32,
    parent_pid: u32,
    name: String,
    executable_path: Option<String>,
    command_line: String,
    user_id: u32,
    user_name: String,
    group_id: u32,
    cpu_percent: Option<f64>,
    memory_bytes: u64,
    state: String,
    threads: u32,
    start_time_epoch: Option<u64>,
    start_time_ticks: u64,
    security_indicators: Vec<String>,
    permission_limited: bool,
}

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
struct SocketOwner {
    pid: u32,
    process_name: String,
    user_id: Option<u32>,
}

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
struct NetworkConnection {
    protocol: String,
    local_address: String,
    local_port: u16,
    remote_address: String,
    remote_port: u16,
    state: String,
    scope: String,
    inode: u64,
    listening: bool,
    owners: Vec<SocketOwner>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ConnectionSnapshot {
    observed_at: u64,
    connections: Vec<NetworkConnection>,
    socket_visibility_limited: bool,
}

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
struct FailedService {
    unit: String,
    description: String,
    load_state: String,
    active_state: String,
    sub_state: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ServiceSnapshot {
    observed_at: u64,
    available: bool,
    detail: Option<String>,
    services: Vec<FailedService>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Telemetry {
    sampled_at: u64,
    cpu_percent: Metric<f64>,
    cpu_cores: Metric<Vec<CoreUsage>>,
    cpu_frequency_mhz: Metric<f64>,
    memory: Metric<Memory>,
    disk: Metric<Disk>,
    disk_io: Metric<DiskIo>,
    network: Metric<Vec<NetworkInterface>>,
    temperatures: Metric<Vec<Temperature>>,
    gpu: Metric<Gpu>,
    battery: Metric<Battery>,
    host: Host,
}

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
struct Memory {
    total_bytes: u64,
    used_bytes: u64,
    available_bytes: u64,
    swap_total_bytes: u64,
    swap_used_bytes: u64,
}

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
struct Disk {
    mount: String,
    filesystem: String,
    total_bytes: u64,
    used_bytes: u64,
    available_bytes: u64,
}

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
struct DiskIo {
    read_bytes_per_sec: f64,
    write_bytes_per_sec: f64,
    devices: Vec<String>,
}

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
struct Gpu {
    vendor: String,
    active_frequency_mhz: Option<f64>,
    current_frequency_mhz: Option<f64>,
    utilization_percent: Option<f64>,
    utilization_detail: String,
}

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
struct Battery {
    name: String,
    capacity_percent: f64,
    status: String,
    energy_now_wh: Option<f64>,
    energy_full_wh: Option<f64>,
    health_percent: Option<f64>,
}

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
struct Temperature {
    label: String,
    celsius: f64,
    source: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Host {
    os: String,
    kernel: String,
    architecture: String,
    cpu_model: String,
    logical_cpus: usize,
    gpu: String,
}

#[tauri::command]
async fn collect_telemetry() -> Telemetry {
    tauri::async_runtime::spawn_blocking(collect_telemetry_sync)
        .await
        .expect("telemetry worker stopped unexpectedly")
}

fn collect_telemetry_sync() -> Telemetry {
    let state = PREVIOUS.get_or_init(|| Mutex::new(Previous::default()));
    let mut previous = state.lock().unwrap_or_else(|e| e.into_inner());
    let (cpu_percent, cpu_cores) = match read_cpu(&mut previous) {
        Ok((percent, cores)) => (
            Metric::from_result(Ok(percent), "/proc/stat"),
            Metric::from_result(Ok(cores), "/proc/stat"),
        ),
        Err(error) => (
            Metric::from_result(Err(error.clone()), "/proc/stat"),
            Metric::from_result(Err(error), "/proc/stat"),
        ),
    };
    let cpu_frequency_mhz = Metric::from_result(
        read_cpu_frequency(),
        "/sys/devices/system/cpu/cpu*/cpufreq/scaling_cur_freq",
    );
    let memory = Metric::from_result(read_memory(), "/proc/meminfo");
    let disk = Metric::from_result(read_disk(), "statvfs(/)");
    let disk_io = Metric::from_result(
        read_disk_io(&mut previous),
        "/proc/diskstats + /sys/class/block",
    );
    let network = Metric::from_result(
        read_network(&mut previous),
        "/proc/net/dev + /sys/class/net",
    );
    let temperatures =
        Metric::from_result(read_temperatures(), "/sys/class/hwmon + /sys/class/thermal");
    let gpu = Metric::from_result(read_gpu(), "/sys/class/drm");
    let battery = Metric::from_result(read_battery(), "/sys/class/power_supply");
    Telemetry {
        sampled_at: SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs(),
        cpu_percent,
        cpu_cores,
        cpu_frequency_mhz,
        memory,
        disk,
        disk_io,
        network,
        temperatures,
        gpu,
        battery,
        host: read_host(),
    }
}

#[tauri::command]
async fn list_processes() -> Result<Vec<ProcessInfo>, String> {
    tauri::async_runtime::spawn_blocking(read_processes)
        .await
        .map_err(|error| error.to_string())?
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ActionResult {
    message: String,
    audit_recorded: bool,
}

#[tauri::command]
async fn terminate_process(
    app: tauri::AppHandle,
    pid: u32,
    expected_start_time_ticks: u64,
) -> Result<ActionResult, String> {
    tauri::async_runtime::spawn_blocking(move || {
        if pid <= 1 || pid == std::process::id() { return Err("Refusing to signal this protected process".to_string()); }
        let target = format!("/proc/{pid}");
        let stat = fs::read_to_string(format!("{target}/stat")).map_err(|_| "Process is no longer available".to_string())?;
        let fields: Vec<&str> = stat[stat.rfind(')').ok_or("Invalid process metadata")? + 1..].split_whitespace().collect();
        let start = fields.get(19).and_then(|v| v.parse::<u64>().ok()).ok_or("Could not verify process identity")?;
        if start != expected_start_time_ticks { return Err("Process identity changed; refresh the list before trying again".into()); }
        let status = fs::read_to_string(format!("{target}/status")).map_err(|_| "Process ended before it could be inspected".to_string())?;
        let uid = status.lines().find(|line| line.starts_with("Uid:")).and_then(|line| line.split_whitespace().nth(1)).and_then(|v| v.parse::<u32>().ok()).ok_or("Could not verify process owner")?;
        if uid != unsafe { libc::geteuid() } { return Err("Only processes owned by the current user can be terminated".into()); }
        // Protect SENTINEL and its parent chain from accidental self-termination.
        let mut ancestor = std::process::id();
        for _ in 0..64 {
            if ancestor == pid { return Err("Refusing to terminate SENTINEL or one of its parent processes".into()); }
            let text = match fs::read_to_string(format!("/proc/{ancestor}/stat")) { Ok(s) => s, Err(_) => break };
            let tail = text.get(text.rfind(')').unwrap_or(0) + 1..).unwrap_or("");
            ancestor = tail.split_whitespace().nth(1).and_then(|v| v.parse().ok()).unwrap_or(0);
            if ancestor == 0 || ancestor == 1 { break; }
        }
        // Recheck PID identity immediately before the fixed SIGTERM operation.
        let check = fs::read_to_string(format!("{target}/stat")).map_err(|_| "Process ended before SIGTERM".to_string())?;
        let same = check.get(check.rfind(')').ok_or("Invalid process metadata")? + 1..).unwrap_or("").split_whitespace().nth(19).and_then(|v| v.parse::<u64>().ok()) == Some(expected_start_time_ticks);
        if !same { return Err("Process identity changed; no signal was sent".into()); }
        let result = unsafe { libc::kill(pid as libc::pid_t, libc::SIGTERM) };
        if result != 0 { return Err(std::io::Error::last_os_error().to_string()); }
        let audit_recorded = app.path().app_data_dir().ok().and_then(|dir| {
            let db = rusqlite::Connection::open(dir.join("sentinel.sqlite")).ok()?;
            db.execute("INSERT INTO audit_logs(action,user,timestamp,details) VALUES(?1,?2,?3,?4)", rusqlite::params!["process_sigterm", uid.to_string(), SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_secs(), format!("SIGTERM sent to PID {pid}; start ticks {expected_start_time_ticks}")]).ok()?;
            Some(())
        }).is_some();
        Ok(ActionResult { message: "SIGTERM sent. The process may take time to exit or may handle the request; SENTINEL will not force-kill it.".into(), audit_recorded })
    }).await.map_err(|e| e.to_string())?
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ToolStatus {
    name: String,
    available: bool,
    path: Option<String>,
}

#[tauri::command]
async fn security_tools() -> Vec<ToolStatus> {
    tauri::async_runtime::spawn_blocking(|| {
        [
            ("ClamAV", "/usr/bin/clamscan"),
            ("YARA", "/usr/bin/yara"),
            ("auditd", "/usr/sbin/auditctl"),
            ("osquery", "/usr/bin/osqueryi"),
            ("rkhunter", "/usr/bin/rkhunter"),
            ("chkrootkit", "/usr/sbin/chkrootkit"),
            ("Lynis", "/usr/sbin/lynis"),
            ("smartmontools", "/usr/sbin/smartctl"),
            ("lm-sensors", "/usr/bin/sensors"),
        ]
        .into_iter()
        .map(|(name, path)| ToolStatus {
            name: name.into(),
            available: Path::new(path).is_file(),
            path: Path::new(path).is_file().then(|| path.into()),
        })
        .collect()
    })
    .await
    .unwrap_or_default()
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ScanResult {
    scanner: String,
    target: String,
    rules_path: Option<String>,
    status: String,
    output: String,
    started_at: u64,
    completed_at: u64,
    output_truncated: bool,
}

fn read_capped(mut stream: impl Read) -> (Vec<u8>, bool) {
    const MAX: usize = 256 * 1024;
    let mut kept = Vec::new();
    let mut buf = [0u8; 8192];
    let mut truncated = false;
    loop {
        match stream.read(&mut buf) {
            Ok(0) | Err(_) => break,
            Ok(count) => {
                let room = MAX.saturating_sub(kept.len());
                kept.extend_from_slice(&buf[..count.min(room)]);
                truncated |= count > room;
            }
        }
    }
    (kept, truncated)
}

fn allowed_scan_target(path: &str) -> Result<std::path::PathBuf, String> {
    let canonical = std::fs::canonicalize(path)
        .map_err(|e| format!("Cannot open selected scan target: {e}"))?;
    let metadata = fs::metadata(&canonical).map_err(|e| e.to_string())?;
    if !metadata.is_file() && !metadata.is_dir() {
        return Err("Choose a regular file or directory".into());
    }
    if [
        "/", "/proc", "/sys", "/dev", "/run", "/etc", "/usr", "/boot", "/var",
    ]
    .iter()
    .any(|blocked| canonical == Path::new(blocked) || canonical.starts_with(blocked))
    {
        return Err("That system-wide path is outside the supported scan scope. Select a user file or directory instead.".into());
    }
    Ok(canonical)
}

#[tauri::command]
async fn scan_security_tool(
    app: tauri::AppHandle,
    scanner: String,
    target: String,
    rules_path: Option<String>,
) -> Result<ScanResult, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let target_path = allowed_scan_target(&target)?;
        let target_text = target_path.to_string_lossy().into_owned();
        let (program, args, rules_text) = match scanner.as_str() {
            "clamav" => {
                if !Path::new("/usr/bin/clamscan").is_file() { return Err("ClamAV clamscan is not installed. Use the explicit install action or install it using Ubuntu's package manager.".into()); }
                if rules_path.is_some() { return Err("ClamAV does not accept a YARA rules path".into()); }
                ("/usr/bin/clamscan", vec!["--no-summary".to_string(), "--infected".to_string(), "--recursive".to_string(), target_text.clone()], None)
            }
            "yara" => {
                if !Path::new("/usr/bin/yara").is_file() { return Err("YARA is not installed. Use the explicit install action or install it using Ubuntu's package manager.".into()); }
                let rules = rules_path.ok_or("Select a YARA rule file first")?;
                let canonical = fs::canonicalize(&rules).map_err(|e| format!("Cannot read selected YARA rule file: {e}"))?;
                let metadata = fs::metadata(&canonical).map_err(|e| e.to_string())?;
                if !metadata.is_file() || metadata.len() > 2 * 1024 * 1024 || !matches!(canonical.extension().and_then(|e| e.to_str()), Some("yar" | "yara")) {
                    return Err("Choose a .yar or .yara regular rule file no larger than 2 MiB".into());
                }
                let rules = canonical.to_string_lossy().into_owned();
                ("/usr/bin/yara", vec!["-r".to_string(), rules.clone(), target_text.clone()], Some(rules))
            }
            _ => return Err("Unknown scanner. Supported scanners are ClamAV and YARA.".into()),
        };
        let started_at = SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_secs();
        let mut child = Command::new(program).args(&args).stdout(Stdio::piped()).stderr(Stdio::piped()).spawn().map_err(|e| format!("Could not start scanner: {e}"))?;
        let stdout = child.stdout.take().ok_or("Scanner output unavailable")?;
        let stderr = child.stderr.take().ok_or("Scanner error output unavailable")?;
        let out_reader = std::thread::spawn(move || read_capped(stdout));
        let err_reader = std::thread::spawn(move || read_capped(stderr));
        let deadline = Instant::now() + std::time::Duration::from_secs(900);
        let exit_status = loop {
            if let Some(status) = child.try_wait().map_err(|e| e.to_string())? { break status; }
            if Instant::now() >= deadline { let _ = child.kill(); let _ = child.wait(); return Err("Scan stopped after the 15-minute limit".into()); }
            std::thread::sleep(std::time::Duration::from_millis(200));
        };
        let (mut stdout, out_truncated) = out_reader.join().map_err(|_| "Scanner output reader failed")?;
        let (stderr, err_truncated) = err_reader.join().map_err(|_| "Scanner error reader failed")?;
        if !stderr.is_empty() { stdout.extend_from_slice(b"\n\nScanner messages:\n"); stdout.extend_from_slice(&stderr); }
        let code = exit_status.code().unwrap_or(-1);
        let status = match scanner.as_str() {
            "clamav" if code == 0 => "complete_no_detections",
            "clamav" if code == 1 => "signature_matches_found",
            "yara" if code == 0 && !String::from_utf8_lossy(&stdout).is_empty() => "rules_matched",
            "yara" if code == 1 => "complete_no_matches",
            _ => "scanner_error",
        }.to_string();
        let completed_at = SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_secs();
        let output = String::from_utf8_lossy(&stdout).into_owned();
        let result = ScanResult { scanner: scanner.clone(), target: target_text.clone(), rules_path: rules_text.clone(), status: status.clone(), output: output.clone(), started_at, completed_at, output_truncated: out_truncated || err_truncated };
        let db_path = app.path().app_data_dir().map_err(|e| e.to_string())?.join("sentinel.sqlite");
        let db = rusqlite::Connection::open(db_path).map_err(|e| e.to_string())?;
        let serialized = serde_json::to_string(&result).map_err(|e| e.to_string())?;
        db.execute("INSERT INTO scan_history(scanner,target,status,started_at,completed_at,results) VALUES(?1,?2,?3,?4,?5,?6)", rusqlite::params![scanner, target_text, status, started_at, completed_at, serialized]).map_err(|e| format!("Scan finished but could not save history: {e}"))?;
        Ok(result)
    }).await.map_err(|e| e.to_string())?
}

#[tauri::command]
async fn install_optional_scanners() -> Result<String, String> {
    tauri::async_runtime::spawn_blocking(|| {
        if !Path::new("/usr/bin/pkexec").is_file() {
            return Err(
                "Ubuntu graphical administrator authorization (pkexec) is unavailable".into(),
            );
        }
        let output = Command::new("/usr/bin/pkexec")
            .args(["/usr/bin/apt-get", "install", "-y", "clamav", "yara"])
            .output()
            .map_err(|e| e.to_string())?;
        let text = String::from_utf8_lossy(&output.stdout).to_string()
            + &String::from_utf8_lossy(&output.stderr);
        if output.status.success() {
            Ok(text)
        } else {
            Err(if text.trim().is_empty() {
                format!(
                    "Package installation did not complete (status {}).",
                    output.status
                )
            } else {
                text
            })
        }
    })
    .await
    .map_err(|e| e.to_string())?
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct IntegrityReport {
    scanned: usize,
    baseline_created: bool,
    changes: Vec<String>,
    detail: String,
}

fn etc_hashes() -> Result<HashMap<String, String>, String> {
    fn visit(dir: &Path, output: &mut HashMap<String, String>) -> Result<(), String> {
        if output.len() >= 5000 {
            return Ok(());
        }
        let entries = fs::read_dir(dir).map_err(|e| format!("{}: {e}", dir.display()))?;
        for entry in entries.flatten() {
            if output.len() >= 5000 {
                break;
            }
            let path = entry.path();
            let Ok(meta) = fs::symlink_metadata(&path) else {
                continue;
            };
            if meta.file_type().is_symlink() {
                continue;
            }
            if meta.is_dir() {
                let _ = visit(&path, output);
            } else if meta.is_file() && meta.len() <= 8 * 1024 * 1024 {
                if let Ok(data) = fs::read(&path) {
                    let hash = format!("{:x}", Sha256::digest(&data));
                    output.insert(path.to_string_lossy().into_owned(), hash);
                }
            }
        }
        Ok(())
    }
    let mut output = HashMap::new();
    visit(Path::new("/etc"), &mut output)?;
    Ok(output)
}

#[tauri::command]
async fn file_integrity(
    app: tauri::AppHandle,
    establish_baseline: bool,
) -> Result<IntegrityReport, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let hashes=etc_hashes()?;
        let db_path=app.path().app_data_dir().map_err(|e|e.to_string())?.join("sentinel.sqlite");
        let mut db=rusqlite::Connection::open(db_path).map_err(|e|e.to_string())?;
        let tx=db.transaction().map_err(|e|e.to_string())?;
        if establish_baseline {
            tx.execute("DELETE FROM file_integrity",[]).map_err(|e|e.to_string())?;
            for (path,hash) in &hashes { tx.execute("INSERT INTO file_integrity(path,baseline_hash,current_hash,status,observed_at) VALUES(?1,?2,?2,'baseline',strftime('%s','now'))",[path,hash]).map_err(|e|e.to_string())?; }
            tx.commit().map_err(|e|e.to_string())?;
            Ok(IntegrityReport{scanned:hashes.len(),baseline_created:true,changes:Vec::new(),detail:"Baseline saved for readable regular files under /etc. Symlinks and files above 8 MiB were skipped.".into()})
        } else {
            let mut old=HashMap::new(); let mut stmt=tx.prepare("SELECT path,baseline_hash FROM file_integrity").map_err(|e|e.to_string())?;
            let rows=stmt.query_map([],|r|Ok((r.get::<_,String>(0)?,r.get::<_,String>(1)?))).map_err(|e|e.to_string())?;
            for row in rows.flatten(){old.insert(row.0,row.1);} drop(stmt);
            if old.is_empty(){return Ok(IntegrityReport{scanned:hashes.len(),baseline_created:false,changes:Vec::new(),detail:"No baseline exists. Create one to enable change comparisons.".into()})}
            let mut changes=Vec::new();
            for (path,hash) in &hashes { let status=match old.get(path){None=>Some("new"),Some(before) if before!=hash=>Some("modified"),_=>None}; if let Some(s)=status { changes.push(format!("{s}: {path}")); tx.execute("INSERT INTO file_integrity(path,baseline_hash,current_hash,status,observed_at) VALUES(?1,?2,?3,?4,strftime('%s','now'))",rusqlite::params![path,old.get(path),hash,s]).map_err(|e|e.to_string())?; } }
            for path in old.keys().filter(|p|!hashes.contains_key(*p)){changes.push(format!("missing: {path}"));tx.execute("INSERT INTO file_integrity(path,baseline_hash,current_hash,status,observed_at) VALUES(?1,?2,NULL,'missing',strftime('%s','now'))",rusqlite::params![path,old.get(path)]).map_err(|e|e.to_string())?;}
            tx.commit().map_err(|e|e.to_string())?;
            Ok(IntegrityReport{scanned:hashes.len(),baseline_created:false,changes,detail:"Compared readable regular files under /etc against the saved baseline. Changes are observations and may be expected.".into()})
        }
    }).await.map_err(|e|e.to_string())?
}

#[tauri::command]
async fn list_connections() -> Result<ConnectionSnapshot, String> {
    tauri::async_runtime::spawn_blocking(read_connections)
        .await
        .map_err(|error| error.to_string())?
}

#[tauri::command]
async fn list_failed_services() -> ServiceSnapshot {
    tauri::async_runtime::spawn_blocking(read_failed_services)
        .await
        .unwrap_or_else(|error| ServiceSnapshot {
            observed_at: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs(),
            available: false,
            detail: Some(error.to_string()),
            services: Vec::new(),
        })
}

fn read_failed_services() -> ServiceSnapshot {
    let observed_at = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    let output = std::process::Command::new("/usr/bin/systemctl")
        .args([
            "list-units",
            "--failed",
            "--no-legend",
            "--plain",
            "--no-pager",
            "--output=json",
        ])
        .output();
    let output = match output {
        Ok(output) if output.status.success() => output,
        Ok(output) => {
            return ServiceSnapshot {
                observed_at,
                available: false,
                detail: Some(String::from_utf8_lossy(&output.stderr).trim().to_string()),
                services: Vec::new(),
            }
        }
        Err(error) => {
            return ServiceSnapshot {
                observed_at,
                available: false,
                detail: Some(error.to_string()),
                services: Vec::new(),
            }
        }
    };
    let parsed: Result<Vec<serde_json::Value>, _> = serde_json::from_slice(&output.stdout);
    match parsed {
        Ok(rows) => {
            let services = rows
                .into_iter()
                .filter_map(|row| {
                    Some(FailedService {
                        unit: row.get("unit")?.as_str()?.to_string(),
                        description: row
                            .get("description")
                            .and_then(|value| value.as_str())
                            .unwrap_or("")
                            .to_string(),
                        load_state: row
                            .get("load")
                            .and_then(|value| value.as_str())
                            .unwrap_or("unknown")
                            .to_string(),
                        active_state: row
                            .get("active")
                            .and_then(|value| value.as_str())
                            .unwrap_or("unknown")
                            .to_string(),
                        sub_state: row
                            .get("sub")
                            .and_then(|value| value.as_str())
                            .unwrap_or("unknown")
                            .to_string(),
                    })
                })
                .collect();
            ServiceSnapshot {
                observed_at,
                available: true,
                detail: None,
                services,
            }
        }
        Err(error) => ServiceSnapshot {
            observed_at,
            available: false,
            detail: Some(format!("Invalid systemctl JSON output: {error}")),
            services: Vec::new(),
        },
    }
}

fn read_processes() -> Result<Vec<ProcessInfo>, String> {
    let history = PROCESS_HISTORY.get_or_init(|| Mutex::new(HashMap::new()));
    let mut previous = history.lock().unwrap_or_else(|error| error.into_inner());
    let now = Instant::now();
    let hz = unsafe { libc::sysconf(libc::_SC_CLK_TCK) }.max(1) as u64;
    let page_size = unsafe { libc::sysconf(libc::_SC_PAGESIZE) }.max(1) as u64;
    let boot_time = fs::read_to_string("/proc/stat").ok().and_then(|text| {
        text.lines()
            .find(|line| line.starts_with("btime "))?
            .split_whitespace()
            .nth(1)?
            .parse::<u64>()
            .ok()
    });
    let users = read_user_names();
    let entries = fs::read_dir("/proc").map_err(|error| error.to_string())?;
    let mut processes = Vec::new();
    let mut current = HashMap::new();
    for entry in entries.flatten() {
        let Some(pid) = entry
            .file_name()
            .to_str()
            .and_then(|value| value.parse::<u32>().ok())
        else {
            continue;
        };
        let Ok(stat) = fs::read_to_string(entry.path().join("stat")) else {
            continue;
        };
        let Some(close) = stat.rfind(')') else {
            continue;
        };
        let name = stat
            .get(stat.find('(').unwrap_or(0) + 1..close)
            .unwrap_or("")
            .to_string();
        let fields: Vec<&str> = stat[close + 1..].split_whitespace().collect();
        if fields.len() < 22 {
            continue;
        }
        let state = fields[0].to_string();
        let parent_pid = fields[1].parse::<u32>().unwrap_or(0);
        let cpu_ticks =
            fields[11].parse::<u64>().unwrap_or(0) + fields[12].parse::<u64>().unwrap_or(0);
        let start_ticks = fields[19].parse::<u64>().unwrap_or(0);
        let resident_pages = fields[21].parse::<u64>().unwrap_or(0);
        current.insert(pid, (cpu_ticks, now));
        let cpu_percent = previous.get(&pid).map(|(old_ticks, old_time)| {
            let elapsed = now.duration_since(*old_time).as_secs_f64().max(0.001);
            cpu_ticks.saturating_sub(*old_ticks) as f64 / hz as f64 / elapsed * 100.0
        });
        let status = fs::read_to_string(entry.path().join("status")).unwrap_or_default();
        let user_id = status
            .lines()
            .find(|line| line.starts_with("Uid:"))
            .and_then(|line| line.split_whitespace().nth(1))
            .and_then(|value| value.parse::<u32>().ok())
            .unwrap_or(u32::MAX);
        let group_id = status
            .lines()
            .find(|line| line.starts_with("Gid:"))
            .and_then(|line| line.split_whitespace().nth(1))
            .and_then(|value| value.parse::<u32>().ok())
            .unwrap_or(u32::MAX);
        let threads = status
            .lines()
            .find(|line| line.starts_with("Threads:"))
            .and_then(|line| line.split_whitespace().nth(1))
            .and_then(|value| value.parse::<u32>().ok())
            .unwrap_or(0);
        let executable_path = fs::read_link(entry.path().join("exe"))
            .ok()
            .map(|path| path.to_string_lossy().into_owned());
        let command_line = fs::read(entry.path().join("cmdline"))
            .ok()
            .map(|bytes| {
                String::from_utf8_lossy(&bytes)
                    .split('\0')
                    .filter(|part| !part.is_empty())
                    .collect::<Vec<_>>()
                    .join(" ")
            })
            .filter(|value| !value.is_empty())
            .unwrap_or_else(|| name.clone());
        let permission_limited = executable_path.is_none();
        let security_indicators = process_indicators(executable_path.as_deref(), user_id);
        let start_time_epoch = boot_time.map(|boot| boot + start_ticks / hz);
        processes.push(ProcessInfo {
            pid,
            parent_pid,
            name,
            executable_path,
            command_line,
            user_id,
            user_name: users
                .get(&user_id)
                .cloned()
                .unwrap_or_else(|| user_id.to_string()),
            group_id,
            cpu_percent,
            memory_bytes: resident_pages * page_size,
            state,
            threads,
            start_time_epoch,
            start_time_ticks: start_ticks,
            security_indicators,
            permission_limited,
        });
    }
    *previous = current;
    processes.sort_by_key(|process| process.pid);
    Ok(processes)
}

fn read_user_names() -> HashMap<u32, String> {
    fs::read_to_string("/etc/passwd")
        .unwrap_or_default()
        .lines()
        .filter_map(|line| {
            let fields: Vec<&str> = line.split(':').collect();
            Some((
                fields.get(2)?.parse::<u32>().ok()?,
                fields.first()?.to_string(),
            ))
        })
        .collect()
}

fn process_indicators(path: Option<&str>, uid: u32) -> Vec<String> {
    let mut indicators = Vec::new();
    if uid == 0 {
        indicators.push("Runs as root".into());
    }
    match path {
        None => indicators.push("Executable path unavailable (permission or process race)".into()),
        Some(path) => {
            let executable = std::path::Path::new(path);
            if ["/tmp", "/var/tmp", "/dev/shm"]
                .iter()
                .any(|prefix| executable.starts_with(prefix))
            {
                indicators.push("Executable is under a temporary or shared memory path".into());
            }
            if executable.components().any(|part| {
                part.as_os_str().to_string_lossy().starts_with('.') && part.as_os_str() != "."
            }) {
                indicators.push("Executable path contains a hidden directory".into());
            }
            if path.ends_with(" (deleted)") {
                indicators.push("Executable has been deleted while running".into());
            }
            if let Ok(home) = std::env::var("HOME") {
                if path.starts_with(&format!("{home}/Downloads/")) {
                    indicators.push(
                        "Executable is running from the current user's Downloads directory".into(),
                    );
                }
            }
        }
    }
    indicators
}

fn read_connections() -> Result<ConnectionSnapshot, String> {
    let (owners_by_inode, mut socket_visibility_limited) = socket_owners();
    let mut connections = Vec::new();
    let mut readable_tables = 0;
    for (path, protocol, ipv6) in [
        ("/proc/net/tcp", "TCP", false),
        ("/proc/net/tcp6", "TCP", true),
        ("/proc/net/udp", "UDP", false),
        ("/proc/net/udp6", "UDP", true),
    ] {
        let Ok(content) = fs::read_to_string(path) else {
            socket_visibility_limited = true;
            continue;
        };
        readable_tables += 1;
        for line in content.lines().skip(1) {
            let fields: Vec<&str> = line.split_whitespace().collect();
            if fields.len() < 10 {
                continue;
            }
            let Ok((local_address, local_port)) = parse_socket_address(fields[1], ipv6) else {
                continue;
            };
            let Ok((remote_address, remote_port)) = parse_socket_address(fields[2], ipv6) else {
                continue;
            };
            let state = if protocol == "UDP" && fields[3] == "07" {
                "UNCONNECTED".to_string()
            } else {
                socket_state(fields[3]).to_string()
            };
            let Ok(inode) = fields[9].parse::<u64>() else {
                continue;
            };
            let listening = state == "LISTEN"
                || (protocol == "UDP" && remote_port == 0 && remote_address.is_unspecified());
            let scope = classify_socket_scope(&remote_address, listening);
            connections.push(NetworkConnection {
                protocol: protocol.into(),
                local_address: local_address.to_string(),
                local_port,
                remote_address: remote_address.to_string(),
                remote_port,
                state,
                scope,
                inode,
                listening,
                owners: owners_by_inode.get(&inode).cloned().unwrap_or_default(),
            });
        }
    }
    if readable_tables == 0 {
        return Err("No readable TCP or UDP tables in /proc/net".into());
    }
    connections.sort_by(|a, b| {
        (
            !a.listening,
            &a.protocol,
            a.local_port,
            &a.remote_address,
            a.remote_port,
        )
            .cmp(&(
                !b.listening,
                &b.protocol,
                b.local_port,
                &b.remote_address,
                b.remote_port,
            ))
    });
    Ok(ConnectionSnapshot {
        observed_at: SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs(),
        connections,
        socket_visibility_limited,
    })
}

fn socket_owners() -> (HashMap<u64, Vec<SocketOwner>>, bool) {
    let mut owners: HashMap<u64, Vec<SocketOwner>> = HashMap::new();
    let mut visibility_limited = false;
    let Ok(entries) = fs::read_dir("/proc") else {
        return (owners, true);
    };
    for entry in entries.flatten() {
        let Some(pid) = entry
            .file_name()
            .to_str()
            .and_then(|value| value.parse::<u32>().ok())
        else {
            continue;
        };
        let process_name = fs::read_to_string(entry.path().join("comm"))
            .unwrap_or_else(|_| "unknown".into())
            .trim()
            .to_string();
        let status = fs::read_to_string(entry.path().join("status")).unwrap_or_default();
        let user_id = status
            .lines()
            .find(|line| line.starts_with("Uid:"))
            .and_then(|line| line.split_whitespace().nth(1))
            .and_then(|value| value.parse::<u32>().ok());
        let fd_dir = entry.path().join("fd");
        let Ok(fds) = fs::read_dir(fd_dir) else {
            visibility_limited = true;
            continue;
        };
        for fd in fds.flatten() {
            let Ok(target) = fs::read_link(fd.path()) else {
                continue;
            };
            let Some(target) = target.to_str() else {
                continue;
            };
            let Some(inode) = target
                .strip_prefix("socket:[")
                .and_then(|value| value.strip_suffix(']'))
                .and_then(|value| value.parse::<u64>().ok())
            else {
                continue;
            };
            let list = owners.entry(inode).or_default();
            if !list.iter().any(|owner| owner.pid == pid) {
                list.push(SocketOwner {
                    pid,
                    process_name: process_name.clone(),
                    user_id,
                });
            }
        }
    }
    (owners, visibility_limited)
}

fn parse_socket_address(value: &str, ipv6: bool) -> Result<(std::net::IpAddr, u16), String> {
    let (address, port) = value.split_once(':').ok_or("Invalid proc socket address")?;
    let port = u16::from_str_radix(port, 16).map_err(|error| error.to_string())?;
    if !ipv6 {
        if address.len() != 8 {
            return Err("Invalid IPv4 proc socket address".into());
        }
        let mut bytes = [0u8; 4];
        for (index, byte) in bytes.iter_mut().enumerate() {
            *byte = u8::from_str_radix(&address[index * 2..index * 2 + 2], 16)
                .map_err(|error| error.to_string())?;
        }
        bytes.reverse();
        return Ok((std::net::IpAddr::V4(std::net::Ipv4Addr::from(bytes)), port));
    }
    if address.len() != 32 {
        return Err("Invalid IPv6 proc socket address".into());
    }
    let mut bytes = [0u8; 16];
    for (word_index, chunk) in address.as_bytes().chunks_exact(8).enumerate() {
        let word = std::str::from_utf8(chunk).map_err(|error| error.to_string())?;
        for byte_index in 0..4 {
            bytes[word_index * 4 + byte_index] =
                u8::from_str_radix(&word[byte_index * 2..byte_index * 2 + 2], 16)
                    .map_err(|error| error.to_string())?;
        }
        bytes[word_index * 4..word_index * 4 + 4].reverse();
    }
    Ok((std::net::IpAddr::V6(std::net::Ipv6Addr::from(bytes)), port))
}

fn socket_state(state: &str) -> &str {
    match state {
        "01" => "ESTABLISHED",
        "02" => "SYN_SENT",
        "03" => "SYN_RECV",
        "04" => "FIN_WAIT1",
        "05" => "FIN_WAIT2",
        "06" => "TIME_WAIT",
        "07" => "CLOSE",
        "08" => "CLOSE_WAIT",
        "09" => "LAST_ACK",
        "0A" => "LISTEN",
        "0B" => "CLOSING",
        "0C" => "NEW_SYN_RECV",
        _ => "UNKNOWN",
    }
}

fn classify_socket_scope(remote: &std::net::IpAddr, listening: bool) -> String {
    if listening {
        return "LISTENER".into();
    }
    if remote.is_unspecified() {
        return "UNCONNECTED".into();
    }
    if remote.is_loopback() {
        return "LOOPBACK".into();
    }
    let private = match remote {
        std::net::IpAddr::V4(address) => address.is_private() || address.is_link_local(),
        std::net::IpAddr::V6(address) => {
            address.is_unique_local() || address.is_unicast_link_local()
        }
    };
    if private {
        "PRIVATE NETWORK".into()
    } else {
        "NONLOCAL ADDRESS".into()
    }
}

fn read_cpu(previous: &mut Previous) -> Result<(f64, Vec<CoreUsage>), String> {
    let content = fs::read_to_string("/proc/stat").map_err(|e| e.to_string())?;
    let mut current = HashMap::new();
    for line in content.lines().take_while(|line| line.starts_with("cpu")) {
        let mut fields = line.split_whitespace();
        let Some(name) = fields.next() else { continue };
        let values: Vec<u64> = fields.filter_map(|value| value.parse().ok()).collect();
        if values.len() < 4 {
            continue;
        }
        let idle = values[3] + values.get(4).copied().unwrap_or(0);
        // guest and guest_nice are already included in user and nice in /proc/stat.
        let total: u64 = values.iter().take(8).sum();
        current.insert(name.to_string(), (total, idle));
    }
    let aggregate = current
        .get("cpu")
        .copied()
        .zip(previous.cpu.get("cpu").copied())
        .map(|(now, old)| cpu_percent(now, old));
    let mut cores: Vec<CoreUsage> = current
        .iter()
        .filter(|(name, _)| {
            name.strip_prefix("cpu").is_some_and(|suffix| {
                !suffix.is_empty() && suffix.chars().all(|c| c.is_ascii_digit())
            })
        })
        .filter_map(|(name, counters)| {
            previous.cpu.get(name).map(|old| CoreUsage {
                core: name[3..].to_string(),
                utilization_percent: cpu_percent(*counters, *old),
            })
        })
        .collect();
    cores.sort_by_key(|core| core.core.parse::<usize>().unwrap_or_default());
    previous.cpu = current;
    aggregate
        .map(|percent| (percent, cores))
        .ok_or_else(|| "Waiting for the second CPU sample".into())
}

fn cpu_percent(current: (u64, u64), previous: (u64, u64)) -> f64 {
    let dt = current.0.saturating_sub(previous.0);
    if dt == 0 {
        0.0
    } else {
        (dt.saturating_sub(current.1.saturating_sub(previous.1))) as f64 * 100.0 / dt as f64
    }
}

fn read_memory() -> Result<Memory, String> {
    let text = fs::read_to_string("/proc/meminfo").map_err(|e| e.to_string())?;
    let mut fields = HashMap::new();
    for line in text.lines() {
        if let Some((key, rest)) = line.split_once(':') {
            if let Some(kb) = rest
                .split_whitespace()
                .next()
                .and_then(|v| v.parse::<u64>().ok())
            {
                fields.insert(key, kb * 1024);
            }
        }
    }
    let total = *fields.get("MemTotal").ok_or("MemTotal unavailable")?;
    let available = *fields
        .get("MemAvailable")
        .ok_or("MemAvailable unavailable")?;
    let swap_total = fields.get("SwapTotal").copied().unwrap_or(0);
    let swap_free = fields.get("SwapFree").copied().unwrap_or(0);
    Ok(Memory {
        total_bytes: total,
        used_bytes: total.saturating_sub(available),
        available_bytes: available,
        swap_total_bytes: swap_total,
        swap_used_bytes: swap_total.saturating_sub(swap_free),
    })
}

fn read_disk() -> Result<Disk, String> {
    let path = CString::new("/").map_err(|e| e.to_string())?;
    let mut stat = std::mem::MaybeUninit::<libc::statvfs>::uninit();
    let result = unsafe { statvfs(path.as_ptr(), stat.as_mut_ptr()) };
    if result != 0 {
        return Err(std::io::Error::last_os_error().to_string());
    }
    let stat = unsafe { stat.assume_init() };
    let unit = stat.f_frsize as u64;
    let total = stat.f_blocks as u64 * unit;
    let available = stat.f_bavail as u64 * unit;
    Ok(Disk {
        mount: "/".into(),
        filesystem: "root filesystem".into(),
        total_bytes: total,
        used_bytes: total.saturating_sub(stat.f_bfree as u64 * unit),
        available_bytes: available,
    })
}

fn read_cpu_frequency() -> Result<f64, String> {
    let raw = fs::read_to_string("/sys/devices/system/cpu/cpu0/cpufreq/scaling_cur_freq")
        .map_err(|e| e.to_string())?;
    let khz = raw.trim().parse::<f64>().map_err(|e| e.to_string())?;
    Ok(khz / 1000.0)
}

fn read_disk_io(previous: &mut Previous) -> Result<DiskIo, String> {
    let text = fs::read_to_string("/proc/diskstats").map_err(|e| e.to_string())?;
    let now = Instant::now();
    let mut current = HashMap::new();
    let mut reads_per_sec = 0.0;
    let mut writes_per_sec = 0.0;
    let mut devices = Vec::new();
    for line in text.lines() {
        let fields: Vec<&str> = line.split_whitespace().collect();
        if fields.len() < 14 {
            continue;
        }
        let name = fields[2];
        if name.starts_with("loop") || name.starts_with("ram") || name.starts_with("zram") {
            continue;
        }
        let sys_path = format!("/sys/class/block/{name}");
        if !std::path::Path::new(&sys_path).join("device").exists()
            || std::path::Path::new(&sys_path).join("partition").exists()
        {
            continue;
        }
        let sectors_read = fields[5].parse::<u64>().map_err(|e| e.to_string())?;
        let sectors_written = fields[9].parse::<u64>().map_err(|e| e.to_string())?;
        current.insert(name.to_string(), (sectors_read, sectors_written, now));
        devices.push(name.to_string());
        if let Some((old_read, old_written, old_time)) = previous.disk.get(name) {
            let elapsed = now.duration_since(*old_time).as_secs_f64().max(0.001);
            reads_per_sec += sectors_read.saturating_sub(*old_read) as f64 * 512.0 / elapsed;
            writes_per_sec += sectors_written.saturating_sub(*old_written) as f64 * 512.0 / elapsed;
        }
    }
    previous.disk = current;
    if devices.is_empty() {
        return Err("No readable physical block-device counters".into());
    }
    Ok(DiskIo {
        read_bytes_per_sec: reads_per_sec,
        write_bytes_per_sec: writes_per_sec,
        devices,
    })
}

fn read_gpu() -> Result<Gpu, String> {
    let entries = fs::read_dir("/sys/class/drm").map_err(|e| e.to_string())?;
    for entry in entries.flatten() {
        let card = entry.file_name().to_string_lossy().to_string();
        if !card
            .strip_prefix("card")
            .is_some_and(|n| n.chars().all(|c| c.is_ascii_digit()))
        {
            continue;
        }
        let path = entry.path();
        let Ok(vendor_id) = fs::read_to_string(path.join("device/vendor")) else {
            continue;
        };
        let vendor = match vendor_id.trim() {
            "0x8086" => "Intel",
            "0x10de" => "NVIDIA",
            "0x1002" => "AMD",
            _ => "Unknown",
        }
        .to_string();
        let active_frequency_mhz = read_number(path.join("gt_act_freq_mhz"));
        let current_frequency_mhz = read_number(path.join("gt_cur_freq_mhz"));
        let utilization_percent = read_number(path.join("gpu_busy_percent"));
        return Ok(Gpu {
            vendor,
            active_frequency_mhz,
            current_frequency_mhz,
            utilization_percent,
            utilization_detail: if utilization_percent.is_some() {
                "GPU utilization from kernel DRM interface".into()
            } else {
                "GPU utilization interface is unavailable; no utilization value is inferred".into()
            },
        });
    }
    Err("No readable DRM GPU device".into())
}

fn read_number(path: impl AsRef<std::path::Path>) -> Option<f64> {
    fs::read_to_string(path).ok()?.trim().parse().ok()
}

fn read_battery() -> Result<Battery, String> {
    let entries = fs::read_dir("/sys/class/power_supply").map_err(|e| e.to_string())?;
    for entry in entries.flatten() {
        let path = entry.path();
        let kind = fs::read_to_string(path.join("type")).unwrap_or_default();
        if kind.trim() != "Battery" {
            continue;
        }
        let capacity_percent =
            read_number(path.join("capacity")).ok_or("Battery capacity is unavailable")?;
        let status = fs::read_to_string(path.join("status"))
            .unwrap_or_else(|_| "Unknown".into())
            .trim()
            .to_string();
        let energy_now_wh = read_number(path.join("energy_now")).map(|value| value / 1_000_000.0);
        let energy_full_wh = read_number(path.join("energy_full")).map(|value| value / 1_000_000.0);
        let design = read_number(path.join("energy_full_design"));
        let health_percent = energy_full_wh.zip(design).and_then(|(full, design)| {
            (design > 0.0).then_some((full * 1_000_000.0 / design * 100.0).min(100.0))
        });
        return Ok(Battery {
            name: entry.file_name().to_string_lossy().to_string(),
            capacity_percent,
            status,
            energy_now_wh,
            energy_full_wh,
            health_percent,
        });
    }
    Err("No battery is exposed by /sys/class/power_supply".into())
}

fn read_interface_addresses() -> HashMap<String, Vec<String>> {
    let mut head: *mut libc::ifaddrs = std::ptr::null_mut();
    if unsafe { libc::getifaddrs(&mut head) } != 0 {
        return HashMap::new();
    }
    let mut result: HashMap<String, Vec<String>> = HashMap::new();
    let mut cursor = head;
    while !cursor.is_null() {
        let entry = unsafe { &*cursor };
        if !entry.ifa_name.is_null() && !entry.ifa_addr.is_null() {
            let name = unsafe { CStr::from_ptr(entry.ifa_name) }
                .to_string_lossy()
                .into_owned();
            let family = unsafe { (*entry.ifa_addr).sa_family as i32 };
            let address = match family {
                libc::AF_INET => {
                    let addr = unsafe { &*(entry.ifa_addr as *const libc::sockaddr_in) };
                    Some(std::net::Ipv4Addr::from(addr.sin_addr.s_addr.to_ne_bytes()).to_string())
                }
                libc::AF_INET6 => {
                    let addr = unsafe { &*(entry.ifa_addr as *const libc::sockaddr_in6) };
                    Some(std::net::Ipv6Addr::from(addr.sin6_addr.s6_addr).to_string())
                }
                _ => None,
            };
            if let Some(address) = address {
                let list = result.entry(name).or_default();
                if !list.contains(&address) {
                    list.push(address);
                }
            }
        }
        cursor = entry.ifa_next;
    }
    unsafe { libc::freeifaddrs(head) };
    result
}

fn read_network(previous: &mut Previous) -> Result<Vec<NetworkInterface>, String> {
    let text = fs::read_to_string("/proc/net/dev").map_err(|e| e.to_string())?;
    let mut interfaces = Vec::new();
    let mut current = HashMap::new();
    let sampled_at = Instant::now();
    let addresses = read_interface_addresses();
    for line in text.lines().skip(2) {
        let Some((name, counters)) = line.split_once(':') else {
            continue;
        };
        let v: Vec<u64> = counters
            .split_whitespace()
            .filter_map(|x| x.parse().ok())
            .collect();
        if v.len() < 16 {
            continue;
        }
        let name = name.trim().to_string();
        current.insert(name.clone(), (v[0], v[8], sampled_at));
        let (rx_rate, tx_rate) = previous
            .network
            .get(&name)
            .map(|(rx, tx, old_time)| {
                let elapsed = sampled_at
                    .duration_since(*old_time)
                    .as_secs_f64()
                    .max(0.001);
                (
                    v[0].saturating_sub(*rx) as f64 / elapsed,
                    v[8].saturating_sub(*tx) as f64 / elapsed,
                )
            })
            .unwrap_or((0.0, 0.0));
        let state_path = format!("/sys/class/net/{name}/operstate");
        let state = fs::read_to_string(state_path)
            .unwrap_or_else(|_| "unknown".into())
            .trim()
            .to_string();
        interfaces.push(NetworkInterface {
            addresses: addresses.get(name.as_str()).cloned().unwrap_or_default(),
            name,
            rx_bytes: v[0],
            rx_bytes_per_sec: rx_rate,
            rx_packets: v[1],
            tx_bytes: v[8],
            tx_bytes_per_sec: tx_rate,
            tx_packets: v[9],
            rx_errors: v[2],
            tx_errors: v[10],
            rx_dropped: v[3],
            tx_dropped: v[11],
            state,
        });
    }
    previous.network = current;
    Ok(interfaces)
}

fn read_temperatures() -> Result<Vec<Temperature>, String> {
    let mut results = Vec::new();
    let hwmon = fs::read_dir("/sys/class/hwmon").map_err(|e| e.to_string())?;
    for dir in hwmon.flatten() {
        let name = fs::read_to_string(dir.path().join("name"))
            .unwrap_or_else(|_| "sensor".into())
            .trim()
            .to_string();
        let Ok(entries) = fs::read_dir(dir.path()) else {
            continue;
        };
        for entry in entries.flatten() {
            let file = entry.file_name().to_string_lossy().to_string();
            if !file.starts_with("temp") || !file.ends_with("_input") {
                continue;
            }
            let Ok(raw) = fs::read_to_string(entry.path()) else {
                continue;
            };
            let Ok(milli_c) = raw.trim().parse::<f64>() else {
                continue;
            };
            let input_index = file.trim_start_matches("temp").trim_end_matches("_input");
            let label = fs::read_to_string(dir.path().join(format!("temp{input_index}_label")))
                .unwrap_or_else(|_| format!("{name} sensor {input_index}"))
                .trim()
                .to_string();
            results.push(Temperature {
                label,
                celsius: milli_c / 1000.0,
                source: "hwmon".into(),
            });
        }
    }
    for dir in fs::read_dir("/sys/class/thermal")
        .map_err(|e| e.to_string())?
        .flatten()
    {
        let file_name = dir.file_name().to_string_lossy().to_string();
        if !file_name.starts_with("thermal_zone") {
            continue;
        }
        let (Ok(kind), Ok(raw)) = (
            fs::read_to_string(dir.path().join("type")),
            fs::read_to_string(dir.path().join("temp")),
        ) else {
            continue;
        };
        let Ok(milli_c) = raw.trim().parse::<f64>() else {
            continue;
        };
        let label = kind.trim().to_string();
        if results
            .iter()
            .any(|sensor| sensor.label.eq_ignore_ascii_case(&label))
        {
            continue;
        }
        results.push(Temperature {
            label,
            celsius: milli_c / 1000.0,
            source: "thermal zone".into(),
        });
    }
    results.sort_by(|a, b| a.label.cmp(&b.label));
    if results.is_empty() {
        Err("No readable temperature sensors".into())
    } else {
        Ok(results)
    }
}

fn read_host() -> Host {
    let os = fs::read_to_string("/etc/os-release")
        .ok()
        .and_then(|s| {
            s.lines().find(|l| l.starts_with("PRETTY_NAME=")).map(|l| {
                l.trim_start_matches("PRETTY_NAME=")
                    .trim_matches('"')
                    .to_string()
            })
        })
        .unwrap_or_else(|| "Linux".into());
    let kernel = fs::read_to_string("/proc/sys/kernel/osrelease")
        .unwrap_or_else(|_| "unknown".into())
        .trim()
        .to_string();
    let cpu_model = fs::read_to_string("/proc/cpuinfo")
        .ok()
        .and_then(|s| {
            s.lines()
                .find(|l| l.starts_with("model name"))
                .and_then(|l| l.split_once(':').map(|(_, v)| v.trim().to_string()))
        })
        .unwrap_or_else(|| "Unknown CPU".into());
    let logical_cpus = fs::read_to_string("/proc/stat")
        .map(|s| {
            s.lines()
                .filter(|l| {
                    l.starts_with("cpu") && l.chars().nth(3).is_some_and(|c| c.is_ascii_digit())
                })
                .count()
        })
        .unwrap_or(0);
    let gpu = fs::read_dir("/sys/class/drm")
        .ok()
        .into_iter()
        .flatten()
        .flatten()
        .filter_map(|d| fs::read_to_string(d.path().join("device/vendor")).ok())
        .next()
        .map(|v| {
            match v.trim() {
                "0x8086" => "Intel GPU",
                "0x10de" => "NVIDIA GPU",
                "0x1002" => "AMD GPU",
                _ => "GPU device",
            }
            .to_string()
        })
        .unwrap_or_else(|| "No GPU device identified".into());
    Host {
        os,
        kernel,
        architecture: std::env::consts::ARCH.into(),
        cpu_model,
        logical_cpus,
        gpu,
    }
}

pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .setup(|app| {
            let app_data_dir = app.path().app_data_dir()?;
            fs::create_dir_all(&app_data_dir)?;
            initialize_database(&app_data_dir.join("sentinel.sqlite"))?;
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            collect_telemetry,
            list_processes,
            list_connections,
            list_failed_services,
            terminate_process,
            security_tools,
            file_integrity,
            scan_security_tool,
            install_optional_scanners
        ])
        .run(tauri::generate_context!())
        .expect("error while running SENTINEL");
}

fn initialize_database(path: &Path) -> Result<(), Box<dyn std::error::Error>> {
    let connection = rusqlite::Connection::open(path)?;
    connection.pragma_update(None, "journal_mode", "WAL")?;
    connection.pragma_update(None, "foreign_keys", "ON")?;
    let version: i64 = connection.pragma_query_value(None, "user_version", |row| row.get(0))?;
    if version < 1 {
        connection.execute_batch(include_str!("../migrations/001_initial.sql"))?;
        connection.pragma_update(None, "user_version", 1)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cpu_delta_is_bounded_and_uses_idle_delta() {
        assert!((cpu_percent((200, 120), (100, 70)) - 50.0).abs() < f64::EPSILON);
        assert_eq!(cpu_percent((200, 150), (100, 50)), 0.0);
        assert_eq!(cpu_percent((100, 50), (200, 100)), 0.0);
    }

    #[test]
    fn reads_real_memory_and_root_filesystem() {
        let memory = read_memory().expect("/proc/meminfo should be readable");
        assert!(memory.total_bytes > 0);
        assert!(memory.available_bytes <= memory.total_bytes);
        let disk = read_disk().expect("root filesystem statvfs should work");
        assert_eq!(disk.mount, "/");
        assert!(disk.total_bytes > disk.used_bytes);
    }

    #[test]
    fn reports_per_core_cpu_after_baseline_sample() {
        let mut previous = Previous::default();
        assert!(read_cpu(&mut previous).is_err());
        std::thread::sleep(std::time::Duration::from_millis(20));
        let (_, cores) =
            read_cpu(&mut previous).expect("second /proc/stat sample should calculate CPU use");
        assert!(!cores.is_empty());
        assert!(cores
            .iter()
            .all(|core| (0.0..=100.0).contains(&core.utilization_percent)));
    }

    #[test]
    fn enumerates_this_running_process_from_proc() {
        let processes = read_processes().expect("/proc process enumeration should work");
        let current_pid = std::process::id();
        let current = processes
            .iter()
            .find(|process| process.pid == current_pid)
            .expect("test process should be present in the real process table");
        assert!(!current.name.is_empty());
        assert!(current.threads > 0);
        assert!(current.memory_bytes > 0);
    }

    #[test]
    fn process_indicators_describe_evidence_without_malware_verdicts() {
        let indicators = process_indicators(Some("/tmp/sample-bin"), 0);
        assert!(indicators.iter().any(|item| item == "Runs as root"));
        assert!(indicators
            .iter()
            .any(|item| item.contains("temporary or shared memory path")));
        assert!(process_indicators(None, 1000)
            .iter()
            .any(|item| item.contains("Executable path unavailable")));
    }

    #[test]
    fn parses_proc_ipv4_and_ipv6_socket_addresses() {
        let (ipv4, port) = parse_socket_address("0100007F:1F90", false).unwrap();
        assert_eq!(ipv4, "127.0.0.1".parse::<std::net::IpAddr>().unwrap());
        assert_eq!(port, 8080);
        let (ipv6, port) =
            parse_socket_address("00000000000000000000000001000000:0050", true).unwrap();
        assert_eq!(ipv6, "::1".parse::<std::net::IpAddr>().unwrap());
        assert_eq!(port, 80);
    }

    #[test]
    fn reads_real_socket_tables_and_systemd_failed_units() {
        let sockets = read_connections().expect("local procfs socket tables should be readable");
        assert!(!sockets.connections.is_empty());
        let services = read_failed_services();
        assert!(
            services.available,
            "systemctl query failed: {:?}",
            services.detail
        );
    }

    #[test]
    fn reads_real_network_counters_and_sensors() {
        let mut previous = Previous::default();
        let interfaces = read_network(&mut previous).expect("/proc/net/dev should be readable");
        assert!(!interfaces.is_empty());
        assert!(interfaces
            .iter()
            .any(|interface| !interface.addresses.is_empty()));
        let sensors =
            read_temperatures().expect("at least one host temperature sensor should be readable");
        assert!(!sensors.is_empty());
        assert!(sensors
            .iter()
            .all(|sensor| sensor.celsius > -50.0 && sensor.celsius < 200.0));
    }

    #[test]
    fn reads_available_frequency_disk_gpu_and_battery_interfaces() {
        assert!(read_cpu_frequency().expect("CPU frequency should be readable") > 0.0);
        let mut previous = Previous::default();
        let io = read_disk_io(&mut previous).expect("physical disk counters should be readable");
        assert!(!io.devices.is_empty());
        if let Ok(gpu) = read_gpu() {
            assert!(!gpu.vendor.is_empty());
            assert!(
                gpu.utilization_percent.is_none()
                    || (0.0..=100.0).contains(&gpu.utilization_percent.unwrap())
            );
        }
        if let Ok(battery) = read_battery() {
            assert!((0.0..=100.0).contains(&battery.capacity_percent));
            assert!(!battery.status.is_empty());
        }
    }

    #[test]
    fn initializes_and_reopens_versioned_local_database() {
        let path = std::env::temp_dir().join(format!(
            "sentinel-test-{}-{}.sqlite",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_nanos()
        ));
        initialize_database(&path).expect("schema migration should create the database");
        initialize_database(&path).expect("schema migration should be idempotent");
        let connection = rusqlite::Connection::open(&path).unwrap();
        let version: i64 = connection
            .pragma_query_value(None, "user_version", |row| row.get(0))
            .unwrap();
        assert_eq!(version, 1);
        let table_count: i64 = connection
            .query_row(
                "SELECT COUNT(*) FROM sqlite_master WHERE type = 'table' AND name = 'system_snapshots'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(table_count, 1);
        drop(connection);
        let _ = fs::remove_file(&path);
        let _ = fs::remove_file(path.with_extension("sqlite-wal"));
        let _ = fs::remove_file(path.with_extension("sqlite-shm"));
    }
}
