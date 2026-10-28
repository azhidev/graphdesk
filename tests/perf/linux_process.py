"""Read-only Linux process-tree idle CPU/PSS sample for a running release app.

Includes current descendants (e.g. WebKit helpers), not unrelated OS services.
No app IPC, graph reads, root permissions, cache flushing or output files.
"""
import argparse
import json
import os
from pathlib import Path
import time


def tree(root: int) -> set[int]:
    seen = set()
    pending = [root]
    while pending:
        pid = pending.pop()
        if pid in seen:
            continue
        seen.add(pid)
        try:
            children = Path(f"/proc/{pid}/task/{pid}/children").read_text()
            pending.extend(int(child) for child in children.split())
        except (FileNotFoundError, PermissionError, ProcessLookupError):
            continue  # Exited/sandboxed subprocess; the final count is a lower bound.
    return seen


def stats(pid: int):
    try:
        fields = Path(f"/proc/{pid}/stat").read_text().rsplit(") ", 1)[1].split()
        # Fields start at Linux stat field 3 (state).
        start, ticks = int(fields[19]), int(fields[11]) + int(fields[12])
    except (FileNotFoundError, PermissionError, ProcessLookupError, IndexError, ValueError):
        return None
    try:
        lines = Path(f"/proc/{pid}/smaps_rollup").read_text().splitlines()
        pss = int(next(line.split()[1] for line in lines if line.startswith("Pss:")))
    except (FileNotFoundError, PermissionError, ProcessLookupError, StopIteration, ValueError):
        pss = None
    return start, ticks, pss


def sample(root: int):
    return {pid: value for pid in tree(root) if (value := stats(pid)) is not None}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--pid", type=int, required=True, help="PID of the launched release app")
    parser.add_argument("--seconds", type=float, default=30, help="Idle sample duration (default: 30)")
    args = parser.parse_args()
    if args.pid <= 0 or not (1 <= args.seconds <= 600):
        parser.error("PID must be positive and seconds must be between 1 and 600")
    if not Path(f"/proc/{args.pid}").exists():
        parser.error("PID no longer exists; launch the first GraphDesk instance")

    started = time.monotonic()
    before = sample(args.pid)
    if args.pid not in before:
        parser.error("cannot read process statistics for this PID")
    time.sleep(args.seconds)
    after = sample(args.pid)
    elapsed = time.monotonic() - started
    if args.pid not in after or after[args.pid][0] != before[args.pid][0]:
        parser.error("app exited/restarted during sample; no valid idle measurement")

    same = [pid for pid in after if pid in before and after[pid][0] == before[pid][0]]
    delta = sum(max(0, after[pid][1] - before[pid][1]) for pid in same)
    print(json.dumps({
        "seconds": round(elapsed, 2),
        "processes": len(after),
        "new_processes_cpu_excluded": len(after) - len(same),
        "missing_pss": sum(value[2] is None for value in after.values()),
        "pss_kib": sum(value[2] or 0 for value in after.values()),
        "cpu_percent_one_core": round(100 * delta / os.sysconf("SC_CLK_TCK") / elapsed, 2),
    }, sort_keys=True))


if __name__ == "__main__":
    main()
