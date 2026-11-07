import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { Activity, AlertTriangle, ArrowDownToLine, ArrowUpFromLine, Battery, Bell, ChevronDown, Cpu, Database, FileClock, Gauge, HardDrive, LayoutDashboard, MemoryStick, Network, Search, Settings, Settings2, Shield, ShieldAlert, ShieldCheck, Thermometer, Wrench, Wifi } from "lucide-react";
import type { Metric, Telemetry } from "./types";
import { FileIntegrityPage, IncidentCentrePage, NetworkMonitorPage, ProcessExplorerPage, SecurityToolsPage, SettingsPage, SystemDiagnosticsPage, SystemHealthPage, ThreatDetectionPage } from "./Pages";

type Page = "command" | "health" | "processes" | "network" | "threats" | "integrity" | "diagnostics" | "performance" | "incidents" | "tools" | "settings";
const navigation: { id: Page; label: string; icon: typeof Cpu; ready: boolean }[] = [
  { id: "command", label: "Command Centre", icon: LayoutDashboard, ready: true },
  { id: "health", label: "System Health", icon: Activity, ready: true },
  { id: "processes", label: "Process Explorer", icon: Cpu, ready: true },
  { id: "network", label: "Network Monitor", icon: Network, ready: true },
  { id: "threats", label: "Threat Detection", icon: ShieldCheck, ready: true },
  { id: "integrity", label: "File Integrity", icon: HardDrive, ready: true },
  { id: "diagnostics", label: "System Diagnostics", icon: Database, ready: false },
  { id: "performance", label: "Performance", icon: Gauge, ready: true },
  { id: "incidents", label: "Incident Centre", icon: FileClock, ready: true },
  { id: "tools", label: "Security Tools", icon: Wrench, ready: true },
  { id: "settings", label: "Settings", icon: Settings, ready: true },
];
const pageLabels = Object.fromEntries(navigation.map(item => [item.id, item.label])) as Record<Page, string>;

const fmtBytes = (n: number) => { const units = ["B", "KB", "MB", "GB", "TB"]; let i = 0, v = n; while (v >= 1024 && i < units.length - 1) { v /= 1024; i++; } return `${v.toFixed(i ? 1 : 0)} ${units[i]}`; };
const pct = (used: number, total: number) => total ? Math.min(100, used / total * 100) : 0;

function MetricCard({ title, value, unit, icon: Icon, tint = "green", detail, percent }: { title: string; value: string; unit?: string; icon: typeof Cpu; tint?: string; detail: string; percent?: number }) {
  return <article className="metric-card">
    <div className="metric-top"><span className="metric-icon"><Icon size={17} /></span><span className="metric-label">{title}</span><span className="live-dot" /></div>
    <div className="metric-value">{value}<small>{unit}</small></div>
    {percent !== undefined && <div className="progress"><i style={{ width: `${percent}%` }} /></div>}
    <div className="metric-detail">{detail}</div>
  </article>;
}

function SourceValue<T>({ metric, children }: { metric: Metric<T>; children: React.ReactNode }) {
  if (!metric.available || metric.value === null) return <div className="unavailable"><AlertTriangle size={15} /><span>Unavailable</span><small>{metric.detail}</small></div>;
  return <>{children}</>;
}

export default function App() {
  const [data, setData] = useState<Telemetry | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [updatedAt, setUpdatedAt] = useState<Date | null>(null);
  const [history, setHistory] = useState<number[]>([]);
  const [page, setPage] = useState<Page>("command");
  const [theme, setThemeState] = useState(() => localStorage.getItem("sentinel-theme") ?? "sentinel");
  const [profile, setProfileState] = useState(() => localStorage.getItem("sentinel-profile") ?? "balanced");
  const setTheme = (value:string) => { localStorage.setItem("sentinel-theme",value); setThemeState(value); };
  const setProfile = (value:string) => { localStorage.setItem("sentinel-profile",value); setProfileState(value); };
  useEffect(() => { document.documentElement.dataset.theme = theme; }, [theme]);

  useEffect(() => {
    let live = true;
    const poll = async () => {
      try {
        const next = await invoke<Telemetry>("collect_telemetry");
        if (!live) return;
        setData(next); setError(null); setUpdatedAt(new Date());
        if (next.cpuPercent.value !== null) setHistory((old) => [...old.slice(-47), next.cpuPercent.value!]);
      } catch (e) { if (live) setError(String(e)); }
    };
    void poll(); const id = window.setInterval(poll, profile === "low" ? 5000 : 1000);
    return () => { live = false; window.clearInterval(id); };
  }, [profile]);

  const m = data?.memory.value;
  const d = data?.disk.value;
  const net = data?.network.value ?? [];
  const networkRate = net.reduce((a, i) => a + i.rxBytesPerSec + i.txBytesPerSec, 0);
  const temps = data?.temperatures.value ?? [];
  const cores = data?.cpuCores.value ?? [];
  const cpu = data?.cpuPercent.value;
  const gpu = data?.gpu.value;
  const battery = data?.battery.value;
  const diskIo = data?.diskIo.value;
  const cpuFrequency = data?.cpuFrequencyMhz.value;
  const cpuPath = history.map((v, i) => `${i * (100 / Math.max(1, history.length - 1))},${36 - v * 0.31}`).join(" ");

  return <div className="app-shell">
    <aside className="sidebar">
      <div className="brand"><div className="brand-mark"><Shield size={22} /></div><div><strong>SENTINEL</strong><small>SYSTEM COMMAND CENTRE</small></div></div>
      <div className="nav-caption">WORKSPACE</div>
      <nav>{navigation.map(({ id, label, icon: Icon }) => <button key={id} className={`nav-item ${page === id ? "active" : ""}`} onClick={() => setPage(id)} aria-current={page === id ? "page" : undefined}><Icon size={17} /><span>{label}</span></button>)}</nav>
      <div className="side-spacer" />
      <div className="profile"><span className="profile-avatar">S</span><span><b>Local device</b><small>USER MODE · READ ONLY</small></span><Settings2 size={15} /></div>
    </aside>

    <main className="main-area">
      <header className="topbar"><div className="breadcrumb">SENTINEL <span>/</span> <b>{pageLabels[page].toUpperCase()}</b></div><div className="top-actions"><div className="status-chip"><span className="live-dot" />COLLECTOR LIVE</div><button className="icon-btn" aria-label="Open process search" onClick={() => setPage("processes")}><Search size={17} /></button><button className="icon-btn" aria-label="Open incident centre" onClick={() => setPage("incidents")}><Bell size={17} /></button><button className="profile-menu" onClick={() => setPage("settings")}><span className="profile-avatar small">P</span> Personal device <ChevronDown size={14} /></button></div></header>

      <div className="content">
        {page === "command" && <section className="heading-row"><div><div className="eyebrow">LOCAL TELEMETRY <span>•</span> LIVE SESSION</div><h1>Command Centre</h1><p className="subtitle">A real-time view of your system health and activity.</p></div><div className="last-updated"><span className="live-dot" /> LAST SAMPLE <b>{updatedAt ? updatedAt.toLocaleTimeString() : "CONNECTING"}</b></div></section>}
        {error && <div className="error-banner"><AlertTriangle size={16} /> Backend telemetry unavailable: {error}</div>}

        {page === "command" && <>
        <section className="section-head"><div><h2>System overview</h2><span>MEASURED FROM LOCAL KERNEL INTERFACES</span></div><button className="range-button" disabled title="Collection interval is fixed at one second in this build">LIVE <span>·</span> 1 SEC <ChevronDown size={13} /></button></section>
        <section className="metric-grid">
          <MetricCard title="CPU UTILISATION" icon={Cpu} value={cpu === null || cpu === undefined ? "—" : cpu.toFixed(1)} unit="%" detail={data ? `${data.host.logicalCpus} logical CPUs · ${data.host.cpuModel}` : "Waiting for telemetry"} percent={cpu ?? undefined} />
          <MetricCard title="MEMORY" icon={MemoryStick} value={m ? fmtBytes(m.usedBytes) : "—"} detail={m ? `${fmtBytes(m.availableBytes)} available of ${fmtBytes(m.totalBytes)}` : "Reading /proc/meminfo"} percent={m ? pct(m.usedBytes, m.totalBytes) : undefined} />
          <MetricCard title="ROOT STORAGE" icon={HardDrive} value={d ? `${pct(d.usedBytes, d.totalBytes).toFixed(0)}%` : "—"} detail={d ? `${fmtBytes(d.usedBytes)} used · ${fmtBytes(d.availableBytes)} free` : "Reading filesystem capacity"} percent={d ? pct(d.usedBytes, d.totalBytes) : undefined} />
          <MetricCard title="NETWORK THROUGHPUT" icon={Wifi} value={fmtBytes(networkRate)} unit="/s" detail={`${net.filter(i => i.state === "up").length} active interface${net.filter(i => i.state === "up").length === 1 ? "" : "s"} · RX + TX`} />
        </section>

        <section className="hardware-grid">
          <MetricCard title="CPU FREQUENCY · CPU 0" icon={Activity} value={cpuFrequency === undefined || cpuFrequency === null ? "—" : cpuFrequency.toFixed(0)} unit="MHz" detail="Kernel cpufreq scaling_cur_freq" />
          <MetricCard title="DISK READ / WRITE" icon={HardDrive} value={diskIo ? `${fmtBytes(diskIo.readBytesPerSec)} / ${fmtBytes(diskIo.writeBytesPerSec)}` : "—"} detail={diskIo ? `${diskIo.devices.join(", ")} · read / write per second` : "Waiting for block-device counters"} />
          <MetricCard title="GPU UTILISATION" icon={Activity} value={gpu?.utilizationPercent === null || gpu?.utilizationPercent === undefined ? "N/A" : gpu.utilizationPercent.toFixed(1)} unit={gpu?.utilizationPercent == null ? undefined : "%"} tint="cyan" detail={gpu ? `${gpu.vendor} · ${gpu.activeFrequencyMhz ?? "—"} MHz active · no estimated utilization` : "No DRM telemetry available"} />
          <MetricCard title="BATTERY" icon={Battery} value={battery ? `${battery.capacityPercent.toFixed(0)}%` : "N/A"} detail={battery ? `${battery.status} · health ${battery.healthPercent === null ? "unavailable" : `${battery.healthPercent.toFixed(1)}% of design`}` : "No battery interface available"} percent={battery?.capacityPercent} />
        </section>

        <section className="panel-grid">
          <article className="panel performance-panel"><div className="panel-head"><div><h3>Performance telemetry</h3><p>CPU load · live samples</p></div><span className="panel-tag"><span className="live-dot" /> LIVE</span></div><div className="chart-wrap"><div className="chart-y"><span>100%</span><span>50%</span><span>0%</span></div><svg className="chart" viewBox="0 0 100 40" preserveAspectRatio="none"><defs><linearGradient id="fill" x1="0" x2="0" y1="0" y2="1"><stop offset="0%" stopColor="#b4f34d" stopOpacity=".22"/><stop offset="100%" stopColor="#b4f34d" stopOpacity="0"/></linearGradient></defs><path d={history.length > 1 ? `M ${cpuPath} L 100 40 L 0 40 Z` : ""} fill="url(#fill)"/><polyline points={cpuPath} fill="none" stroke="#b4f34d" strokeWidth="0.7" vectorEffect="non-scaling-stroke" /></svg></div><div className="chart-foot"><span>CPU · {cpu === null || cpu === undefined ? "Waiting for second sample" : `${cpu.toFixed(1)}% now`}</span><span>1s interval</span></div><div className="core-grid">{cores.map(core => <div className="core-load" key={core.core}><span>CORE {core.core}</span><i><b style={{ width: `${core.utilizationPercent}%` }} /></i><strong>{core.utilizationPercent.toFixed(0)}%</strong></div>)}</div></article>
          <article className="panel thermal-panel"><div className="panel-head"><div><h3>Thermal sensors</h3><p>Kernel-reported temperatures</p></div><Thermometer size={17} className="muted-icon" /></div><SourceValue metric={data?.temperatures ?? { value: null, source: "hwmon", available: false, detail: "Waiting for telemetry" }}>{<div className="sensor-list">{temps.slice(0, 5).map((t, i) => <div className="sensor-row" key={`${t.label}-${i}`}><div className="sensor-name"><span className="sensor-dot" />{t.label}<small>{t.source}</small></div><b className={t.celsius >= 80 ? "hot" : ""}>{t.celsius.toFixed(1)}°C</b></div>)}</div>}</SourceValue><div className="thermal-note">Sensor labels come from the kernel; some platform sensors may not map to a specific component.</div></article>
        </section>

        <section className="lower-grid">
          <article className="panel network-panel"><div className="panel-head"><div><h3>Network interfaces</h3><p>Addresses and counters from local kernel interfaces</p></div><span className="source-tag">/proc/net/dev · getifaddrs</span></div><div className="table-head"><span>INTERFACE / ADDRESS</span><span>STATE</span><span>RECEIVED</span><span>TRANSMITTED</span></div>{net.filter(n => n.name !== "lo").slice(0, 4).map(n => <div className="interface-row" key={n.name}><b><span className="interface-icon"><Network size={14} /></span><span>{n.name}<small className="if-addresses">{n.addresses.join(" · ") || "no address assigned"}</small></span></b><span className={`state ${n.state}`}>{n.state.toUpperCase()}</span><span title={`${n.rxPackets} packets`}><ArrowDownToLine size={12} /> {fmtBytes(n.rxBytesPerSec)}/s<small className="network-errors">{n.rxErrors} errors · {n.rxDropped} dropped</small></span><span title={`${n.txPackets} packets`}><ArrowUpFromLine size={12} /> {fmtBytes(n.txBytesPerSec)}/s<small className="network-errors">{n.txErrors} errors · {n.txDropped} dropped</small></span></div>)}{net.length === 0 && <div className="empty-row">Waiting for interface counters…</div>}</article>
          <article className="panel posture-panel"><div className="panel-head"><div><h3>Monitoring coverage</h3><p>Telemetry sources available now</p></div><ShieldCheck size={17} className="muted-icon" /></div><div className="coverage-list">{[["CPU counters", data?.cpuPercent.available], ["Per-core CPU", data?.cpuCores.available], ["CPU frequency", data?.cpuFrequencyMhz.available], ["Memory", data?.memory.available], ["Root filesystem", data?.disk.available], ["Disk I/O", data?.diskIo.available], ["Network interfaces", data?.network.available], ["Temperature sensors", data?.temperatures.available], ["GPU device", data?.gpu.available], ["Battery", data?.battery.available]].map(([label, ok]) => <div className="coverage-row" key={String(label)}><span><i className={ok ? "check" : "pending"}>{ok ? "✓" : "·"}</i>{label}</span><small>{ok ? "AVAILABLE" : data ? "UNAVAILABLE" : "CONNECTING"}</small></div>)}</div><div className="coverage-foot"><ShieldCheck size={14} /> No security posture score is inferred from telemetry availability.</div></article>
        </section>

        <footer className="footer"><span><span className="live-dot" /> LOCAL COLLECTION ACTIVE</span><span>{data ? `${data.host.os} · kernel ${data.host.kernel} · ${data.host.architecture}` : "Connecting to Rust collector"}</span><span>{gpu ? `${gpu.vendor} GPU · ${gpu.utilizationDetail}` : "GPU telemetry unavailable"}</span><span><Battery size={13} /> {battery ? `${battery.name} · ${battery.status}` : "Battery telemetry unavailable"}</span></footer>
        <div className="sr-only">{data?.sampledAt}</div>
        </>}
        {page === "health" && <SystemHealthPage data={data}/>}
        {page === "performance" && <SystemHealthPage data={data}/>}
        {page === "processes" && <ProcessExplorerPage/>}
        {page === "network" && <NetworkMonitorPage data={data}/>}
        {page === "threats" && <ThreatDetectionPage/>}
        {page === "integrity" && <FileIntegrityPage/>}
        {page === "diagnostics" && <SystemDiagnosticsPage/>}
        {page === "incidents" && <IncidentCentrePage/>}
        {page === "tools" && <SecurityToolsPage/>}
        {page === "settings" && <SettingsPage theme={theme} setTheme={setTheme} profile={profile} setProfile={setProfile}/>}
      </div>
    </main>
  </div>;
}
