get the PID
pgrep -af graphdesk-desktop


check the performance
sleep 15

python3 tests/perf/linux_process.py \
  --pid 1547448 \
  --seconds 30
