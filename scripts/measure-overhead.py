#!/usr/bin/env python3
"""Sample SENTINEL process-tree CPU and RSS from procfs (no external tools)."""
import os
import pathlib
import sys
import time

ROOT = pathlib.Path("/proc")


def snapshot():
    processes = {}
    for entry in ROOT.iterdir():
        if not entry.name.isdigit():
            continue
        try:
            stat = (entry / "stat").read_text()
            tail = stat[stat.rfind(")") + 2 :].split()
            ppid = int(tail[1])
            ticks = int(tail[11]) + int(tail[12])
            rss_pages = int((entry / "statm").read_text().split()[1])
            name = (entry / "comm").read_text().strip()
            processes[int(entry.name)] = (ppid, ticks, rss_pages, name)
        except (OSError, ValueError, IndexError):
            continue
    roots = [pid for pid, p in processes.items() if p[3] == "sentinel"]
    if not roots:
        raise RuntimeError("SENTINEL process was not found")
    descendants = set(roots)
    changed = True
    while changed:
        changed = False
        for pid, p in processes.items():
            if p[0] in descendants and pid not in descendants:
                descendants.add(pid)
                changed = True
    root = roots[0]
    page_size = os.sysconf("SC_PAGE_SIZE")
    tree_ticks = sum(processes[pid][1] for pid in descendants if pid in processes)
    tree_rss = sum(processes[pid][2] for pid in descendants if pid in processes) * page_size
    tree_pss = 0
    for pid in descendants:
        try:
            for line in (ROOT / str(pid) / "smaps_rollup").read_text().splitlines():
                if line.startswith("Pss:"):
                    tree_pss += int(line.split()[1]) * 1024
                    break
        except (OSError, ValueError, IndexError):
            pass
    root_pss = 0
    try:
        for line in (ROOT / str(root) / "smaps_rollup").read_text().splitlines():
            if line.startswith("Pss:"):
                root_pss = int(line.split()[1]) * 1024
                break
    except (OSError, ValueError, IndexError):
        pass
    return (processes[root][1], tree_ticks, processes[root][2] * page_size,
            tree_rss, root_pss, tree_pss, len(descendants))


duration = float(sys.argv[1]) if len(sys.argv) > 1 else 30.0
hz = os.sysconf("SC_CLK_TCK")
start_time = time.monotonic()
start_root_ticks, start_tree_ticks, start_root_rss, start_tree_rss, start_root_pss, start_tree_pss, start_count = snapshot()
time.sleep(duration)
end_time = time.monotonic()
end_root_ticks, end_tree_ticks, end_root_rss, end_tree_rss, end_root_pss, end_tree_pss, end_count = snapshot()
elapsed = end_time - start_time
root_cpu_one_core = max(0.0, (end_root_ticks - start_root_ticks) / hz / elapsed * 100.0)
tree_cpu_one_core = max(0.0, (end_tree_ticks - start_tree_ticks) / hz / elapsed * 100.0)
cpu_machine = tree_cpu_one_core / (os.cpu_count() or 1)
print(f"sample_seconds={elapsed:.1f}")
print(f"processes={start_count}..{end_count}")
print(f"main_process_cpu_one_core_percent={root_cpu_one_core:.2f}")
print(f"process_tree_cpu_one_core_percent={tree_cpu_one_core:.2f}")
print(f"average_cpu_machine_percent={cpu_machine:.3f}")
print(f"main_process_rss_start_end_mib={start_root_rss / 1024**2:.1f}..{end_root_rss / 1024**2:.1f}")
print(f"main_process_pss_start_end_mib={start_root_pss / 1024**2:.1f}..{end_root_pss / 1024**2:.1f}")
print(f"process_tree_rss_start_end_mib={start_tree_rss / 1024**2:.1f}..{end_tree_rss / 1024**2:.1f}")
print(f"process_tree_pss_start_end_mib={start_tree_pss / 1024**2:.1f}..{end_tree_pss / 1024**2:.1f}")
print("RSS is summed across parent/child processes and can double-count shared pages.")
