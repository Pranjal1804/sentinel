BEGIN IMMEDIATE;

CREATE TABLE IF NOT EXISTS system_snapshots (
    id INTEGER PRIMARY KEY,
    timestamp INTEGER NOT NULL,
    cpu REAL,
    memory_json TEXT,
    disk_json TEXT,
    gpu_json TEXT,
    temperatures_json TEXT,
    network_json TEXT
);
CREATE INDEX IF NOT EXISTS idx_system_snapshots_timestamp ON system_snapshots(timestamp);

CREATE TABLE IF NOT EXISTS processes (
    id INTEGER PRIMARY KEY,
    pid INTEGER NOT NULL,
    ppid INTEGER,
    name TEXT NOT NULL,
    executable_path TEXT,
    user_id INTEGER,
    cpu_usage REAL,
    memory_usage INTEGER,
    start_time INTEGER,
    observed_at INTEGER NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_processes_pid_observed ON processes(pid, observed_at);

CREATE TABLE IF NOT EXISTS network_connections (
    id INTEGER PRIMARY KEY,
    pid INTEGER,
    local_address TEXT,
    local_port INTEGER,
    remote_address TEXT,
    remote_port INTEGER,
    protocol TEXT,
    state TEXT,
    observed_at INTEGER NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_network_connections_observed ON network_connections(observed_at);

CREATE TABLE IF NOT EXISTS security_events (
    id INTEGER PRIMARY KEY,
    event_type TEXT NOT NULL,
    severity TEXT NOT NULL,
    source TEXT NOT NULL,
    description TEXT NOT NULL,
    evidence TEXT,
    timestamp INTEGER NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_security_events_timestamp ON security_events(timestamp);

CREATE TABLE IF NOT EXISTS alerts (
    id INTEGER PRIMARY KEY,
    fingerprint TEXT NOT NULL UNIQUE,
    severity TEXT NOT NULL,
    title TEXT NOT NULL,
    description TEXT NOT NULL,
    status TEXT NOT NULL DEFAULT 'new',
    created_at INTEGER NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_alerts_status_created ON alerts(status, created_at);

CREATE TABLE IF NOT EXISTS incidents (
    id INTEGER PRIMARY KEY,
    title TEXT NOT NULL,
    severity TEXT NOT NULL,
    confidence REAL,
    status TEXT NOT NULL DEFAULT 'open',
    first_seen INTEGER NOT NULL,
    last_seen INTEGER NOT NULL,
    evidence TEXT
);

CREATE TABLE IF NOT EXISTS file_integrity (
    id INTEGER PRIMARY KEY,
    path TEXT NOT NULL,
    baseline_hash TEXT,
    current_hash TEXT,
    status TEXT NOT NULL,
    observed_at INTEGER NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_file_integrity_path ON file_integrity(path, observed_at);

CREATE TABLE IF NOT EXISTS service_events (
    id INTEGER PRIMARY KEY,
    service_name TEXT NOT NULL,
    state TEXT NOT NULL,
    error TEXT,
    timestamp INTEGER NOT NULL
);

CREATE TABLE IF NOT EXISTS scan_history (
    id INTEGER PRIMARY KEY,
    scanner TEXT NOT NULL,
    target TEXT NOT NULL,
    status TEXT NOT NULL,
    started_at INTEGER NOT NULL,
    completed_at INTEGER,
    results TEXT
);

CREATE TABLE IF NOT EXISTS application_settings (
    id INTEGER PRIMARY KEY,
    key TEXT NOT NULL UNIQUE,
    value TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS audit_logs (
    id INTEGER PRIMARY KEY,
    action TEXT NOT NULL,
    user TEXT,
    timestamp INTEGER NOT NULL,
    details TEXT
);
CREATE INDEX IF NOT EXISTS idx_audit_logs_timestamp ON audit_logs(timestamp);

COMMIT;
