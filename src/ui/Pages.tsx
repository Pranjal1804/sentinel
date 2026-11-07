import { useEffect, useMemo, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { open } from "@tauri-apps/plugin-dialog";
import { AlertTriangle, ArrowDownToLine, ArrowUpFromLine, Cpu, HardDrive, MemoryStick, Network, Search, ShieldAlert, Thermometer } from "lucide-react";
import type { ConnectionSnapshot, FailedService, NetworkConnection, ProcessInfo, ServiceSnapshot, Telemetry } from "./types";

const bytes = (n: number) => { const units = ["B", "KB", "MB", "GB", "TB"]; let i = 0, v = n; while (v >= 1024 && i < units.length - 1) { v /= 1024; i++; } return `${v.toFixed(i ? 1 : 0)} ${units[i]}`; };

export function SystemHealthPage({ data }: { data: Telemetry | null }) {
  const memory = data?.memory.value;
  const disk = data?.disk.value;
  const io = data?.diskIo.value;
  const battery = data?.battery.value;
  const gpu = data?.gpu.value;
  return <>
    <div className="page-intro"><div><div className="eyebrow">LIVE HOST TELEMETRY</div><h2>System Health</h2><p>Current readings from kernel and hardware interfaces.</p></div><span className="page-status"><i className="live-dot" /> UPDATING EVERY SECOND</span></div>
    <div className="detail-grid">
      <article className="detail-card"><Cpu/><span>CPU</span><strong>{data?.cpuPercent.value == null ? "—" : `${data.cpuPercent.value.toFixed(1)}%`}</strong><small>{data?.cpuFrequencyMhz.value?.toFixed(0) ?? "—"} MHz · {data?.host.logicalCpus ?? 0} logical CPUs</small><div className="core-health">{data?.cpuCores.value?.map(c => <label key={c.core}>CPU {c.core}<i><b style={{width:`${c.utilizationPercent}%`}}/></i><b>{c.utilizationPercent.toFixed(0)}%</b></label>)}</div></article>
      <article className="detail-card"><MemoryStick/><span>MEMORY & SWAP</span><strong>{memory ? bytes(memory.usedBytes) : "—"}</strong><small>{memory ? `${bytes(memory.availableBytes)} available of ${bytes(memory.totalBytes)}` : "Unavailable"}</small><div className="detail-progress"><b style={{width:`${memory ? memory.usedBytes / memory.totalBytes * 100 : 0}%`}}/></div><small>Swap: {memory ? `${bytes(memory.swapUsedBytes)} / ${bytes(memory.swapTotalBytes)}` : "—"}</small></article>
      <article className="detail-card"><HardDrive/><span>STORAGE & I/O</span><strong>{disk ? `${(disk.usedBytes / disk.totalBytes * 100).toFixed(1)}% used` : "—"}</strong><small>{disk ? `${bytes(disk.usedBytes)} used · ${bytes(disk.availableBytes)} free on /` : "Unavailable"}</small><small>Read {io ? `${bytes(io.readBytesPerSec)}/s` : "—"} · Write {io ? `${bytes(io.writeBytesPerSec)}/s` : "—"}</small></article>
      <article className="detail-card"><Thermometer/><span>THERMALS</span><strong>{data?.temperatures.value?.[0] ? `${data.temperatures.value[0].celsius.toFixed(1)}°C` : "—"}</strong><small>Kernel-reported temperature sensors</small><div className="mini-sensor-list">{data?.temperatures.value?.map((sensor, i) => <label key={`${sensor.label}-${i}`}>{sensor.label}<b>{sensor.celsius.toFixed(1)}°C</b></label>)}</div></article>
      <article className="detail-card"><ActivityIcon/><span>GPU</span><strong>{gpu?.utilizationPercent == null ? "Utilization unavailable" : `${gpu.utilizationPercent.toFixed(1)}%`}</strong><small>{gpu ? `${gpu.vendor} · current ${gpu.currentFrequencyMhz ?? "—"} MHz · active ${gpu.activeFrequencyMhz ?? "—"} MHz` : "No DRM device telemetry"}</small><small>{gpu?.utilizationDetail}</small></article>
      <article className="detail-card"><Network/><span>BATTERY</span><strong>{battery ? `${battery.capacityPercent.toFixed(0)}% · ${battery.status}` : "Unavailable"}</strong><small>{battery ? `${battery.healthPercent?.toFixed(1) ?? "—"}% full-charge capacity vs. design` : "No battery interface exposed"}</small><small>{battery?.energyNowWh == null ? "Energy reading unavailable" : `${battery.energyNowWh.toFixed(1)} Wh remaining`}</small></article>
    </div>
  </>;
}

function ActivityIcon() { return <Cpu/>; }

export function ProcessExplorerPage() {
  const [processes, setProcesses] = useState<ProcessInfo[]>([]);
  const [selectedPid, setSelectedPid] = useState<number | null>(null);
  const [filter, setFilter] = useState("");
  const [error, setError] = useState<string | null>(null);
  const [updatedAt, setUpdatedAt] = useState<Date | null>(null);
  const [actionMessage, setActionMessage] = useState<string | null>(null);
  const [actionBusy, setActionBusy] = useState(false);

  useEffect(() => {
    let live = true;
    const load = async () => {
      try { const rows = await invoke<ProcessInfo[]>("list_processes"); if (!live) return; setProcesses(rows); setError(null); setUpdatedAt(new Date()); }
      catch (e) { if (live) setError(String(e)); }
    };
    void load(); const timer = window.setInterval(load, 2000);
    return () => { live = false; window.clearInterval(timer); };
  }, []);

  const visible = useMemo(() => {
    const needle = filter.toLowerCase();
    return processes.filter(p => !needle || `${p.name} ${p.commandLine} ${p.userName} ${p.pid}`.toLowerCase().includes(needle))
      .sort((a,b) => (b.cpuPercent ?? -1) - (a.cpuPercent ?? -1));
  }, [filter, processes]);
  const selected = processes.find(p => p.pid === selectedPid) ?? null;
  const children = selected ? processes.filter(p => p.parentPid === selected.pid) : [];
  const terminate = async () => {
    if (!selected) return;
    const confirmed = window.confirm(`Send SIGTERM to ${selected.name} (PID ${selected.pid})?\n\nThis asks the process to exit and may cause unsaved work to be lost. SENTINEL will not send SIGKILL. Only processes owned by your user can be targeted.`);
    if (!confirmed) return;
    setActionBusy(true); setActionMessage(null);
    try {
      const result = await invoke<{message:string; auditRecorded:boolean}>("terminate_process", { pid:selected.pid, expectedStartTimeTicks:selected.startTimeTicks });
      setActionMessage(`${result.message}${result.auditRecorded ? " Action recorded in the local audit log." : " Warning: action could not be written to the local audit log."}`);
      const rows = await invoke<ProcessInfo[]>("list_processes"); setProcesses(rows);
    } catch (e) { setActionMessage(String(e)); }
    finally { setActionBusy(false); }
  };

  return <>
    <div className="page-intro"><div><div className="eyebrow">LOCAL PROCESS CONTROL · /PROC</div><h2>Process Explorer</h2><p>Live process inventory and investigation indicators. Indicators are not malware verdicts.</p></div><span className="page-status">{updatedAt ? `UPDATED ${updatedAt.toLocaleTimeString()}` : "READING PROCESS TABLE"}</span></div>
    {error && <div className="error-banner"><AlertTriangle size={16}/>{error}</div>}
    <div className="process-summary"><span><b>{processes.length}</b> visible processes</span><span><b>{processes.filter(p=>p.userId===0).length}</b> running as UID 0</span><span><b>{processes.filter(p=>p.securityIndicators.length>0).length}</b> with investigation indicators</span><span className="process-warning"><ShieldAlert size={14}/>Review indicators in context</span></div>
    <div className="process-layout">
      <section className="panel process-table-panel"><div className="process-toolbar"><label><Search size={15}/><input value={filter} onChange={e=>setFilter(e.target.value)} placeholder="Filter by name, command, user or PID"/></label><span>REFRESH · 2 SEC</span></div><div className="process-table-scroll"><table className="process-table"><thead><tr><th>PID / PPID</th><th>PROCESS / EXECUTABLE</th><th>USER</th><th>CPU</th><th>MEMORY</th><th>STATE</th><th>INDICATORS</th></tr></thead><tbody>{visible.map(p=><tr key={p.pid} className={`${p.pid===selectedPid?"selected":""} ${p.securityIndicators.length?"flagged":""}`} onClick={()=>setSelectedPid(p.pid)}><td><b>{p.pid}</b><small>parent {p.parentPid}</small></td><td><b>{p.name}</b><small>{p.executablePath ?? "Executable path unavailable"}</small></td><td>{p.userName}<small>UID {p.userId}</small></td><td>{p.cpuPercent == null ? "—" : `${p.cpuPercent.toFixed(1)}%`}</td><td>{bytes(p.memoryBytes)}</td><td><span className="state-pill">{p.state}</span></td><td>{p.securityIndicators.length ? <span className="indicator-count"><AlertTriangle size={12}/>{p.securityIndicators.length}</span> : <span className="no-indicator">—</span>}</td></tr>)}</tbody></table>{!visible.length&&<div className="empty-row">{processes.length?"No processes match this filter":"Waiting for /proc data…"}</div>}</div></section>
      <aside className="panel process-detail">{selected ? <><div className="panel-head"><div><div className="eyebrow">PROCESS DETAIL · PID {selected.pid}</div><h3>{selected.name}</h3></div><span className="state-pill">{selected.state}</span></div><dl><dt>Executable</dt><dd className="mono-value">{selected.executablePath ?? "Unavailable (permissions or process ended)"}</dd><dt>Command line</dt><dd className="mono-value command-value">{selected.commandLine || selected.name}</dd><dt>Owner</dt><dd>{selected.userName} (UID {selected.userId}, GID {selected.groupId})</dd><dt>Parent</dt><dd>PID {selected.parentPid}</dd><dt>CPU / memory</dt><dd>{selected.cpuPercent == null ? "Waiting for next sample" : `${selected.cpuPercent.toFixed(1)}%`} · {bytes(selected.memoryBytes)}</dd><dt>Threads / children</dt><dd>{selected.threads} threads · {children.length} visible children</dd><dt>Started</dt><dd>{selected.startTimeEpoch ? new Date(selected.startTimeEpoch*1000).toLocaleString() : "Unavailable"}</dd></dl>{selected.permissionLimited&&<p className="permission-note">Some metadata is incomplete because the current user cannot inspect this process.</p>}<div className="indicator-list"><h4>Investigation indicators</h4>{selected.securityIndicators.length?selected.securityIndicators.map((item,i)=><div key={i}><AlertTriangle size={13}/>{item}</div>):<p>No current path/privilege indicators from the available metadata. This is not a clean bill of health.</p>}</div>{actionMessage&&<p className="permission-note">{actionMessage}</p>}<button className="danger-action" disabled={actionBusy} onClick={terminate}>{actionBusy?"SENDING SIGTERM…":"TERMINATE PROCESS · SIGTERM"}</button><p className="process-caution">Requires confirmation. Sends SIGTERM only, checks process identity and ownership, and never escalates to SIGKILL.</p></> : <div className="select-process"><Cpu size={22}/><b>Select a process</b><span>Inspect executable metadata, owner, parent, resource use, and indicator evidence.</span></div>}</aside>
    </div>
  </>;
}

export function NetworkMonitorPage({ data }: { data: Telemetry | null }) {
  const [snapshot, setSnapshot] = useState<ConnectionSnapshot | null>(null);
  const [connectionError, setConnectionError] = useState<string | null>(null);
  const [socketView, setSocketView] = useState<"connections" | "listeners">("connections");
  const [socketFilter, setSocketFilter] = useState("");
  const interfaces = data?.network.value ?? [];
  useEffect(() => {
    let live = true;
    const load = async () => {
      try { const value = await invoke<ConnectionSnapshot>("list_connections"); if (!live) return; setSnapshot(value); setConnectionError(null); }
      catch (error) { if (live) setConnectionError(String(error)); }
    };
    void load(); const timer = window.setInterval(load, 5000);
    return () => { live = false; window.clearInterval(timer); };
  }, []);
  const sockets = snapshot?.connections.filter(connection => socketView === "listeners" ? connection.listening : !connection.listening) ?? [];
  const visibleSockets = sockets.filter(connection => {
    const query = socketFilter.toLowerCase();
    return !query || `${connection.protocol} ${connection.localAddress} ${connection.localPort} ${connection.remoteAddress} ${connection.remotePort} ${connection.state} ${connection.owners.map(owner => `${owner.processName} ${owner.pid}`).join(" ")}`.toLowerCase().includes(query);
  });
  return <>
    <div className="page-intro"><div><div className="eyebrow">LIVE · LOCAL INTERFACE DATA</div><h2>Network Monitor</h2><p>Addresses and traffic counters collected from this device.</p></div><span className="page-status"><i className="live-dot"/> REFRESHING EVERY SECOND</span></div>
    <div className="network-page-grid">{interfaces.map(i=><article className="panel interface-card" key={i.name}><div className="panel-head"><div><h3><Network size={15}/>{i.name}</h3><p>{i.addresses.join(" · ")||"No address assigned"}</p></div><span className={`state ${i.state}`}>{i.state.toUpperCase()}</span></div><div className="traffic-pair"><div><small>RECEIVED</small><b><ArrowDownToLine size={14}/>{bytes(i.rxBytesPerSec)}/s</b><span>{bytes(i.rxBytes)} total · {i.rxPackets} packets</span><span>{i.rxErrors} errors · {i.rxDropped} dropped</span></div><div><small>TRANSMITTED</small><b><ArrowUpFromLine size={14}/>{bytes(i.txBytesPerSec)}/s</b><span>{bytes(i.txBytes)} total · {i.txPackets} packets</span><span>{i.txErrors} errors · {i.txDropped} dropped</span></div></div></article>)}</div>
    <section className="panel socket-panel"><div className="panel-head"><div><h3>Connections and listening ports</h3><p>Local TCP/UDP tables · refreshed every five seconds</p></div><span className="source-tag">/proc/net/tcp* · udp*</span></div>
      {snapshot?.socketVisibilityLimited&&<div className="permission-note">Some socket owners could not be read with current permissions. Connection rows remain visible where procfs permits.</div>}
      {connectionError&&<div className="error-banner"><AlertTriangle size={14}/>{connectionError}</div>}
      <div className="socket-toolbar"><div className="socket-switch"><button className={socketView==="connections"?"selected":""} onClick={()=>setSocketView("connections")}>Connections <b>{snapshot?.connections.filter(item=>!item.listening).length ?? 0}</b></button><button className={socketView==="listeners"?"selected":""} onClick={()=>setSocketView("listeners")}>Listening ports <b>{snapshot?.connections.filter(item=>item.listening).length ?? 0}</b></button></div><label><Search size={14}/><input value={socketFilter} onChange={event=>setSocketFilter(event.target.value)} placeholder="Filter address, port, protocol or process"/></label></div>
      <div className="socket-table-scroll"><table className="socket-table"><thead><tr><th>PROTO</th><th>LOCAL</th><th>REMOTE</th><th>STATE / SCOPE</th><th>PROCESS OWNER</th></tr></thead><tbody>{visibleSockets.map((connection,index)=><SocketRow key={`${connection.protocol}-${connection.inode}-${index}`} connection={connection}/> )}</tbody></table>{snapshot&&!visibleSockets.length&&<div className="empty-row">{socketFilter?"No sockets match this filter":socketView==="listeners"?"No listening ports found":"No current TCP connections or UDP sockets found"}</div>}{!snapshot&&!connectionError&&<div className="empty-row">Reading local socket tables…</div>}</div>
      <p className="socket-privacy">No remote-IP reputation lookup is performed. Public-address classification describes address scope only; it does not mean malicious.</p>
    </section>
  </>;
}

export function SystemDiagnosticsPage() {
  const [snapshot, setSnapshot] = useState<ServiceSnapshot | null>(null);
  const [error, setError] = useState<string | null>(null);
  useEffect(() => {
    let live = true;
    const load = async () => {
      try { const result = await invoke<ServiceSnapshot>("list_failed_services"); if (!live) return; setSnapshot(result); setError(null); }
      catch (reason) { if (live) setError(String(reason)); }
    };
    void load(); const timer = window.setInterval(load, 15000);
    return () => { live = false; window.clearInterval(timer); };
  }, []);
  const services: FailedService[] = snapshot?.services ?? [];
  return <>
    <div className="page-intro"><div><div className="eyebrow">READ-ONLY · SYSTEMD</div><h2>System Diagnostics</h2><p>Failed system services queried from the local systemd manager.</p></div><span className="page-status">REFRESHES EVERY 15 SECONDS</span></div>
    {error&&<div className="error-banner"><AlertTriangle size={15}/>{error}</div>}
    {snapshot&&!snapshot.available&&<div className="error-banner"><AlertTriangle size={15}/>{snapshot.detail||"systemd service data is unavailable to this user"}</div>}
    {snapshot?.available&&services.length===0&&<div className="diagnostic-ok panel"><span>✓</span><div><b>No failed system services reported</b><p>systemd returned an empty failed-unit list at {new Date(snapshot.observedAt*1000).toLocaleTimeString()}.</p></div></div>}
    {snapshot?.available&&services.length>0&&<section className="panel service-panel"><div className="panel-head"><div><h3>{services.length} failed service{services.length===1?"":"s"}</h3><p>Observed {new Date(snapshot.observedAt*1000).toLocaleTimeString()} · no services changed</p></div><span className="service-state-bad">FAILED</span></div><div className="service-list">{services.map(service=><article key={service.unit}><div><b>{service.unit}</b><p>{service.description||"No description provided by systemd"}</p></div><span>{service.loadState}</span><span>{service.activeState} / {service.subState}</span></article>)}</div><p className="socket-privacy">Findings reflect the systemd state query. SENTINEL does not restart, enable, or disable services.</p></section>}
    {!snapshot&&!error&&<div className="panel empty-row">Querying systemd…</div>}
  </>;
}

type ToolStatus = { name:string; available:boolean; path:string|null };
type IntegrityReport = { scanned:number; baselineCreated:boolean; changes:string[]; detail:string };

export function ThreatDetectionPage() {
  const [processes,setProcesses]=useState<ProcessInfo[]>([]); const [tools,setTools]=useState<ToolStatus[]>([]); const [services,setServices]=useState<ServiceSnapshot|null>(null);
  useEffect(()=>{let live=true; Promise.all([invoke<ProcessInfo[]>("list_processes"),invoke<ToolStatus[]>("security_tools"),invoke<ServiceSnapshot>("list_failed_services")]).then(([p,t,s])=>{if(live){setProcesses(p);setTools(t);setServices(s)}}).catch(()=>{});return()=>{live=false}},[]);
  const flagged=processes.filter(p=>p.securityIndicators.length);
  return <><PageIntro eyebrow="LOCAL OBSERVATIONS · RULE BASED" title="Threat Detection" detail="Current process indicators and scanner availability. No signature scan is implied or launched automatically."/><section className="panel feature-panel"><h3>Process investigation indicators <span className="source-tag">/proc</span></h3>{flagged.length?flagged.map(p=><article className="evidence-row" key={p.pid}><b>{p.name} · PID {p.pid}</b><span>{p.securityIndicators.join(" · ")}</span><small>Indicator only; review path, owner, and context. No malware verdict.</small></article>):<div className="empty-row">No current process indicators were reported by the available metadata. This is not a malware-free determination.</div>}</section><section className="panel feature-panel"><h3>Security tool coverage</h3><div className="tool-grid">{tools.map(t=><div className="tool-card" key={t.name}><b>{t.name}</b><span className={t.available?"available":"unavailable"}>{t.available?"AVAILABLE":"NOT INSTALLED"}</span><small>{t.path??"Optional integration; SENTINEL did not install it."}</small></div>)}</div></section><section className="panel feature-panel"><h3>System service findings</h3>{services?.available?(services.services.length?services.services.map(s=><div className="evidence-row" key={s.unit}><b>{s.unit}</b><span>{s.description} · {s.activeState}/{s.subState}</span></div>):<p className="feature-note">systemd reports no failed units at this observation.</p>):<p className="feature-note">systemd status unavailable or access restricted.</p>}</section></>;
}

export function FileIntegrityPage() {
  const [report,setReport]=useState<IntegrityReport|null>(null); const [busy,setBusy]=useState(false); const [error,setError]=useState("");
  const run=async(baseline:boolean)=>{if(baseline&&!window.confirm("Create or replace the local integrity baseline for readable regular files under /etc? This only stores hashes in SENTINEL's local database."))return;setBusy(true);setError("");try{setReport(await invoke<IntegrityReport>("file_integrity",{establishBaseline:baseline}))}catch(e){setError(String(e))}finally{setBusy(false)}};
  return <><PageIntro eyebrow="ON DEMAND · SHA-256" title="File Integrity" detail="Inspect changes under /etc. Hashing runs only when requested; symlinks and files larger than 8 MiB are skipped."/><section className="panel feature-panel integrity-actions"><div><h3>Configuration integrity</h3><p>Scope: /etc · max 5,000 files · no files are changed.</p></div><button disabled={busy} onClick={()=>void run(true)}>CREATE BASELINE</button><button disabled={busy} onClick={()=>void run(false)}>CHECK CHANGES</button></section>{error&&<div className="error-banner">{error}</div>}{report&&<section className="panel feature-panel"><h3>{report.baselineCreated?"Baseline created":`${report.changes.length} changes observed`} · {report.scanned} files checked</h3><p className="feature-note">{report.detail}</p>{report.changes.map((line,i)=><div className="evidence-row" key={`${line}-${i}`}>{line}</div>)}{!report.changes.length&&!report.baselineCreated&&<p>No differences were found in the scanned set.</p>}</section>}</>;
}

export function IncidentCentrePage() {
  const [processes,setProcesses]=useState<ProcessInfo[]>([]);const [services,setServices]=useState<ServiceSnapshot|null>(null);const [sockets,setSockets]=useState<ConnectionSnapshot|null>(null);
  useEffect(()=>{let live=true;Promise.all([invoke<ProcessInfo[]>("list_processes"),invoke<ServiceSnapshot>("list_failed_services"),invoke<ConnectionSnapshot>("list_connections")]).then(([p,s,n])=>{if(live){setProcesses(p);setServices(s);setSockets(n)}}).catch(()=>{});return()=>{live=false}},[]);
  const items=[...processes.filter(p=>p.securityIndicators.length).map(p=>({key:`p${p.pid}`,title:`Process indicator: ${p.name} (${p.pid})`,evidence:p.securityIndicators.join(" · ")})),...(services?.services??[]).map(s=>({key:s.unit,title:`Failed systemd unit: ${s.unit}`,evidence:`${s.activeState}/${s.subState} · ${s.description}`})),...(sockets?.connections.filter(c=>c.listening)??[]).map(c=>({key:`l${c.protocol}${c.localPort}${c.inode}`,title:`Listening socket: ${c.protocol} ${c.localAddress}:${c.localPort}`,evidence:c.owners.map(o=>`${o.processName} (${o.pid})`).join(", ")||"owner unavailable"}))];
  return <><PageIntro eyebrow="OBSERVATION TIMELINE · LOCAL" title="Incident Centre" detail="Current evidence grouped for review. Temporal proximity does not establish causation; no risk score or incident verdict is inferred."/><section className="panel feature-panel"><h3>{items.length} current observations</h3>{items.map(i=><article className="evidence-row" key={i.key}><b>{i.title}</b><span>{i.evidence}</span><small>Observed from local process, systemd, or procfs data. Review before taking action.</small></article>)}{!items.length&&<div className="empty-row">No current observations from the sources queried. This does not establish that the system is threat-free.</div>}</section></>;
}

export function SecurityToolsPage(){
  const [tools,setTools]=useState<ToolStatus[]>([]);const [busy,setBusy]=useState("");const [message,setMessage]=useState("");const [scan,setScan]=useState<{scanner:string;target:string;rulesPath:string|null;status:string;output:string;outputTruncated:boolean}|null>(null);
  const refresh=()=>void invoke<ToolStatus[]>("security_tools").then(setTools).catch(e=>setMessage(String(e)));
  useEffect(()=>{refresh()},[]);
  const install=async()=>{if(!window.confirm("Install ClamAV and YARA with Ubuntu's package manager?\n\nOperation: pkexec apt-get install -y clamav yara\n\nUbuntu may request administrator authentication. No scans will run automatically."))return;setBusy("install");setMessage("");try{const output=await invoke<string>("install_optional_scanners");setMessage(output||"Installation completed.");refresh()}catch(e){setMessage(String(e))}finally{setBusy("")}};
  const runScan=async(scanner:"clamav"|"yara",directory:boolean)=>{
    if(!tools.some(t=>t.available&&(scanner==="clamav"?t.name==="ClamAV":t.name==="YARA"))){setMessage(`${scanner.toUpperCase()} is unavailable. Install optional scanners first.`);return}
    const target=await open({title:`Choose a ${scanner.toUpperCase()} scan target`,directory,multiple:false});if(typeof target!=="string")return;
    let rulesPath:string|undefined;
    if(scanner==="yara"){const rules=await open({title:"Choose a YARA rule file",directory:false,multiple:false,filters:[{name:"YARA rules",extensions:["yar","yara"]}]});if(typeof rules!=="string")return;rulesPath=rules}
    const description=scanner==="clamav"?`recursively scan ${target} using local ClamAV signatures`:`scan ${target} with YARA rules from ${rulesPath}`;
    if(!window.confirm(`Start ${description}?\n\nThis is a user-requested local scan and may use substantial CPU and disk I/O. SENTINEL sends the selected paths only to the local scanner; it performs no network reputation lookup.`))return;
    setBusy(scanner);setMessage("");setScan(null);try{const result=await invoke<typeof scan>("scan_security_tool",{scanner,target,rulesPath:rulesPath??null});setScan(result)}catch(e){setMessage(String(e))}finally{setBusy("")}
  };
  return <><PageIntro eyebrow="OPTIONAL LOCAL SCANNERS" title="Security Tools" detail="Installed integrations can scan explicitly selected user files. SENTINEL never launches scans automatically."/><section className="panel feature-panel"><div className="panel-head"><div><h3>Available utilities</h3><p>Other integrations are detected for visibility; no configuration is changed.</p></div><button className="scanner-install" onClick={()=>void install()} disabled={busy!==""}>{busy==="install"?"INSTALLING…":"INSTALL CLAMAV + YARA"}</button></div><div className="tool-grid">{tools.map(t=><div className="tool-card" key={t.name}><b>{t.name}</b><span className={t.available?"available":"unavailable"}>{t.available?"AVAILABLE":"NOT INSTALLED"}</span><small>{t.path??"Optional integration"}</small></div>)}</div><div className="scanner-actions"><button disabled={busy!==""||!tools.some(t=>t.available&&t.name==="ClamAV")} onClick={()=>void runScan("clamav",false)}>{busy==="clamav"?"SCANNING WITH CLAMAV…":"CHOOSE FILE · CLAMAV"}</button><button disabled={busy!==""||!tools.some(t=>t.available&&t.name==="ClamAV")} onClick={()=>void runScan("clamav",true)}>CHOOSE FOLDER · CLAMAV</button><button disabled={busy!==""||!tools.some(t=>t.available&&t.name==="YARA")} onClick={()=>void runScan("yara",false)}>{busy==="yara"?"SCANNING WITH YARA…":"CHOOSE FILE + RULES · YARA"}</button><button disabled={busy!==""||!tools.some(t=>t.available&&t.name==="YARA")} onClick={()=>void runScan("yara",true)}>CHOOSE FOLDER + RULES · YARA</button></div>{message&&<pre className="scanner-result">{message}</pre>}</section>{scan&&<section className="panel feature-panel"><h3>Scan result · {scan.scanner.toUpperCase()}</h3><p>Target: <code>{scan.target}</code> · {scan.status.split("_").join(" ")}</p><p className="feature-note">A clean signature/rule result does not prove a system is malware-free. Scanner output can be incomplete due to permissions or unsupported file types.</p><pre className="scanner-result">{scan.output||"No scanner output."}{scan.outputTruncated?"\n[Output capped at 256 KiB]":""}</pre></section>}<section className="panel feature-panel"><h3>Scan safeguards</h3><p>Scans require selecting a target and confirming each run. SENTINEL only invokes fixed ClamAV/YARA executables with argument arrays, rejects broad system paths, stores scan results locally, and stops scans after 15 minutes.</p></section></>;
}

export function SettingsPage({theme,setTheme,profile,setProfile}:{theme:string;setTheme:(v:string)=>void;profile:string;setProfile:(v:string)=>void}) {return <><PageIntro eyebrow="LOCAL PREFERENCES" title="Settings" detail="Theme and telemetry polling profile are stored locally on this device."/><section className="panel feature-panel"><h3>Interface themes</h3><p>Pick an accent and atmosphere for the command centre.</p><div className="theme-grid">{[["sentinel","Sentinel Green"],["arctic","Arctic Cyan"],["ember","Ember Amber"],["violet","Ultraviolet"],["rose","Neon Rose"],["jade","Deep Jade"],["ocean","Abyssal Blue"],["solar","Solar Flare"],["mono","Carbon Mono"]].map(([key,label])=><button className={`theme-choice ${theme===key?"selected":""}`} key={key} onClick={()=>setTheme(key)}><i className={`theme-swatch ${key}`}/>{label}{theme===key&&<small>ACTIVE</small>}</button>)}</div></section><section className="panel feature-panel"><h3>Monitoring profile</h3><p>Changes dashboard telemetry polling; process/network pages retain their own refresh intervals.</p><div className="profile-grid">{[["balanced","Balanced","1 second"],["low","Low Resource","5 seconds"],["intensive","Intensive Diagnostics","1 second"]].map(([key,label,interval])=><button className={`theme-choice ${profile===key?"selected":""}`} key={key} onClick={()=>setProfile(key)}><b>{label}</b><small>{interval} telemetry polling</small></button>)}</div></section></>}

function PageIntro({eyebrow,title,detail}:{eyebrow:string;title:string;detail:string}){return <div className="page-intro"><div><div className="eyebrow">{eyebrow}</div><h2>{title}</h2><p>{detail}</p></div></div>}

function SocketRow({ connection }: { connection: NetworkConnection }) {
  const owners = connection.owners.length ? connection.owners.map(owner => `${owner.processName} (${owner.pid})`).join(", ") : "Owner unavailable";
  return <tr><td><span className="protocol-pill">{connection.protocol}</span></td><td className="mono-cell">{connection.localAddress}:{connection.localPort}</td><td className="mono-cell">{connection.remoteAddress}:{connection.remotePort}</td><td><b className="socket-state">{connection.state}</b><small className="socket-scope">{connection.scope}</small></td><td>{owners}</td></tr>;
}

export function NotReadyPage({ title, description }: { title: string; description: string }) {
  return <div className="not-ready panel"><div className="not-ready-icon"><ShieldAlert size={24}/></div><div className="eyebrow">NOT IMPLEMENTED IN THIS BUILD</div><h2>{title}</h2><p>{description}</p><small>SENTINEL will show a capability here only after it has a real local data source and a verified implementation.</small></div>;
}
