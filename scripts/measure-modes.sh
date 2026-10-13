#!/bin/bash
# Замер режимов под длительной нагрузкой: частота, потребление CPU, расход
# батареи, температура. По этим замерам заполнена таблица `powerTable` в Main.qml.
#
#   ./scripts/measure-modes.sh [режим ...]        (по умолчанию все четыре)
#
# Нужен запущенный демон (MICONTROL_SOCKET, по умолчанию /run/micontrol-dev.sock,
# см. scripts/dev-daemon.sh). Режим «Полная мощность» включайте заранее:
#   ./target/debug/micontrol pl1sync on|off
# На каждый режим: 15 с на применение лимитов, 60 с нагрузки на все ядра,
# замер на 40–60 с (после окна PL2), 20 с остывания.
set -uo pipefail
export LC_NUMERIC=C
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
export MICONTROL_SOCKET="${MICONTROL_SOCKET:-/run/micontrol-dev.sock}"
C="$ROOT/target/debug/micontrol"
BAT=$(grep -l Battery /sys/class/power_supply/*/type | head -1 | xargs dirname)
HWMON=$(grep -l coretemp /sys/class/hwmon/*/name | head -1 | xargs dirname)
[ $# -gt 0 ] || set -- silent auto turbo full_speed
MODES=("$@")

avg_mhz() { awk '/MHz/ {s += $4; n++} END {printf "%d", s / n}' /proc/cpuinfo; }
cpu_w() { local v; v=$("$C" status | awk '/Мощность CPU/ {print $3}'); echo "${v:-0}"; }

echo "питание: $([ "$(cat /sys/class/power_supply/A*/online 2>/dev/null | head -1)" = 1 ] && echo сеть || echo батарея)"
for m in "${MODES[@]}"; do
    "$C" perf "$m" >/dev/null || exit 1
    sleep 15
    lim=$("$C" status | grep PL1 | sed 's/PL1 CPU: *//')
    for _ in $(seq "$(nproc)"); do timeout 62 sh -c 'while :; do :; done' & done
    sleep 40
    f=0 p=0 b=0
    for _ in 1 2 3 4; do
        f=$((f + $(avg_mhz)))
        p=$(echo "$p + $(cpu_w)" | bc)
        b=$((b + $(cat "$BAT/power_now") / 1000))
        sleep 5
    done
    t=$(($(cat "$HWMON/temp1_input") / 1000))
    wait
    printf '%-10s | %s | %4d МГц | CPU %5.1f Вт | батарея %5.1f Вт | %d °C\n' \
        "$m" "$lim" $((f / 4)) "$(echo "$p / 4" | bc -l)" "$(echo "$b / 4000" | bc -l)" "$t"
    sleep 20
done
