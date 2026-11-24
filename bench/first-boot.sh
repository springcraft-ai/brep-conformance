#!/bin/bash
# SPDX-License-Identifier: MIT OR Apache-2.0
# First-boot setup of the benchmark machine, run once as root: bench/README.md. The Benchmark
# workflow sets RUNNER_VERSION and RUNNER_JITCONFIG ahead of this script; without them the machine
# gets no runner.
set -euo pipefail

LIFETIME_MINUTES=480
KERNEL_ARGS="nosmt intel_idle.max_cstate=1 processor.max_cstate=1"
HOUSEKEEPING_CORES=24
STATE_DIR=/etc/benchmark-runner

# The reboot at the end can interrupt cloud-init before it records this script as done, and it
# would then run again on the next boot and reboot forever.
if [ -e "$STATE_DIR/configured" ]; then
    exit 0
fi
mkdir -p "$STATE_DIR"

# The lifetime is armed first, so a failure in any later step still leaves an instance that ends.
# It is a calendar time and not `shutdown -h +N`, because a pending shutdown is lost on reboot.
deadline="$(date -u -d "+${LIFETIME_MINUTES} minutes" '+%Y-%m-%d %H:%M:%S') UTC"
echo "$deadline" > "$STATE_DIR/deadline"

cat > /etc/systemd/system/benchmark-lifetime.service <<'EOF'
[Unit]
Description=Power off the benchmark instance at the end of its fixed lifetime

[Service]
Type=oneshot
ExecStart=/usr/bin/systemctl poweroff
EOF

cat > /etc/systemd/system/benchmark-lifetime.timer <<EOF
[Unit]
Description=Fixed lifetime of the benchmark instance

[Timer]
OnCalendar=$deadline
AccuracySec=1s

[Install]
WantedBy=timers.target
EOF

systemctl daemon-reload
systemctl enable --now benchmark-lifetime.timer

cat > /usr/local/sbin/benchmark-machine <<'EOF'
#!/bin/bash
# Runs on every boot: enforces the lifetime, fixes the clocks, and records the environment.
set -euo pipefail

STATE_DIR=/etc/benchmark-runner
CPU=/sys/devices/system/cpu

# A calendar timer does not fire for a time that passed while the machine was rebooting.
if [ "$(date -u +%s)" -ge "$(date -u -d "$(cat "$STATE_DIR/deadline")" +%s)" ]; then
    systemctl poweroff
    exit 0
fi

# intel_pstate's no_turbo only lowers the frequency request, and this processor still runs above
# its base frequency with it set. The turbo-disable bit of IA32_MISC_ENABLE holds it at base.
modprobe msr
python3 - <<'PY'
import glob
import os
import struct

MISC_ENABLE = 0x1A0
TURBO_DISABLE = 1 << 38

paths = glob.glob("/dev/cpu/[0-9]*/msr")
if not paths:
    raise SystemExit("benchmark-machine: no MSR devices found")
for path in paths:
    fd = os.open(path, os.O_RDWR)
    (value,) = struct.unpack("<Q", os.pread(fd, 8, MISC_ENABLE))
    os.pwrite(fd, struct.pack("<Q", value | TURBO_DISABLE), MISC_ENABLE)
    (value,) = struct.unpack("<Q", os.pread(fd, 8, MISC_ENABLE))
    os.close(fd)
    if not value & TURBO_DISABLE:
        raise SystemExit(f"benchmark-machine: turbo is still enabled on {path}")
PY
for governor in "$CPU"/cpu[0-9]*/cpufreq/scaling_governor; do
    if [ -e "$governor" ]; then
        echo performance > "$governor"
    fi
done

token=$(curl -sf -X PUT -H 'X-aws-ec2-metadata-token-ttl-seconds: 60' \
    http://169.254.169.254/latest/api/token)
metadata() {
    curl -sf -H "X-aws-ec2-metadata-token: $token" "http://169.254.169.254/latest/meta-data/$1"
}

cat > "$STATE_DIR/environment" <<ENV
instance_type=$(metadata instance-type)
ami_id=$(metadata ami-id)
availability_zone=$(metadata placement/availability-zone)
kernel=$(uname -r)
kernel_cmdline=$(cat /proc/cmdline)
cpu_model=$(grep -m1 'model name' /proc/cpuinfo | cut -d: -f2- | sed 's/^ //')
microcode=$(grep -m1 microcode /proc/cpuinfo | cut -d: -f2- | sed 's/^ //')
online_cpus=$(cat "$CPU/online")
isolated_cpus=$(cat "$CPU/isolated")
smt=$(cat "$CPU/smt/control")
turbo_off=1
base_mhz=$(($(cat "$CPU/cpu0/cpufreq/base_frequency") / 1000))
governor=$(cat "$CPU/cpu0/cpufreq/scaling_governor")
lifetime_ends=$(cat "$STATE_DIR/deadline")
ENV

# The first boot has no isolated cores yet; only the boot after the reboot is ready for a session.
if [ -n "$(cat "$CPU/isolated")" ]; then
    mkdir -p /run/benchmark-runner
    touch /run/benchmark-runner/ready
fi
EOF
chmod 755 /usr/local/sbin/benchmark-machine

cat > /etc/systemd/system/benchmark-machine.service <<'EOF'
[Unit]
Description=Fix the clocks of the benchmark instance and record its environment
Wants=network-online.target
After=network-online.target

[Service]
Type=oneshot
RemainAfterExit=yes
ExecStart=/usr/local/sbin/benchmark-machine

[Install]
WantedBy=multi-user.target
EOF
systemctl daemon-reload
systemctl enable benchmark-machine.service

# The runner prints the host name in every job's public log, and the default one is the private IP.
# cloud-init would set the default back on each boot.
hostnamectl set-hostname benchmark-runner
echo "127.0.1.1 benchmark-runner" >> /etc/hosts
echo "preserve_hostname: true" > /etc/cloud/cloud.cfg.d/99-benchmark-runner.cfg

# Background package work would run beside the cases being timed.
systemctl mask --now apt-daily.timer apt-daily-upgrade.timer unattended-upgrades.service || true

# One logical CPU per physical core: the lowest-numbered sibling of each. The first
# HOUSEKEEPING_CORES of them stay with the operating system, and the rest are isolated.
primaries=$(cat /sys/devices/system/cpu/cpu[0-9]*/topology/thread_siblings_list \
    | sed 's/[,-].*//' | sort -nu)
isolated=$(echo "$primaries" | tail -n "+$((HOUSEKEEPING_CORES + 1))" | paste -sd, -)
if [ -z "$isolated" ]; then
    echo "user-data: no cores left to isolate" >&2
    exit 1
fi

cat > /etc/default/grub.d/99-benchmark-runner.cfg <<EOF
GRUB_CMDLINE_LINUX_DEFAULT="\$GRUB_CMDLINE_LINUX_DEFAULT $KERNEL_ARGS isolcpus=$isolated"
EOF
update-grub

# One GitHub Actions runner, registered for one job. It starts on the boot after this one, once the
# clocks are fixed and the cores isolated.
if [ -n "${RUNNER_JITCONFIG:-}" ]; then
    runner_dir=/opt/actions-runner
    install -d -o ubuntu -g ubuntu "$runner_dir"
    release="https://github.com/actions/runner/releases/download/v$RUNNER_VERSION"
    curl -sSfL "$release/actions-runner-linux-x64-$RUNNER_VERSION.tar.gz" \
        | sudo -u ubuntu tar -xz -C "$runner_dir"
    "$runner_dir/bin/installdependencies.sh"
    install -m 600 -o ubuntu -g ubuntu /dev/null "$STATE_DIR/jitconfig"
    printf '%s' "$RUNNER_JITCONFIG" > "$STATE_DIR/jitconfig"

    cat > /etc/systemd/system/benchmark-actions-runner.service <<EOF
[Unit]
Description=GitHub Actions runner for one benchmark session
Wants=network-online.target
After=network-online.target benchmark-machine.service
ConditionPathExists=/run/benchmark-runner/ready

[Service]
User=ubuntu
WorkingDirectory=$runner_dir
ExecStart=/bin/bash -c 'exec ./run.sh --jitconfig "\$(cat $STATE_DIR/jitconfig)"'

[Install]
WantedBy=multi-user.target
EOF
    systemctl daemon-reload
    systemctl enable benchmark-actions-runner.service
fi

touch "$STATE_DIR/configured"
systemctl reboot
