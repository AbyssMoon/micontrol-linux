#!/bin/bash
# Диагностика на ноутбуке: ТОЛЬКО ЧТЕНИЕ (команды 0xFA), ничего не меняет.
# Запуск: sudo ./scripts/probe-hw.sh
# Результат: build/probe/ (вывод вызовов + копия ACPI-таблиц для разбора WMAA).
set -uo pipefail

ROOT="$(cd "$(dirname "$(readlink -f "$0")")/.." && pwd)"
OUT="$ROOT/build/probe"
KO="$ROOT/build/acpi_call/acpi_call.ko"
METHOD='\_SB.PC00.WMID.WMAA'
[ "$(id -u)" = 0 ] || { echo "нужен root: sudo $0" >&2; exit 1; }
mkdir -p "$OUT/tables"

LOADED=""
if [ ! -e /proc/acpi/call ]; then
    insmod "$KO" || { echo "не удалось загрузить $KO" >&2; exit 1; }
    LOADED=1
fi
trap '[ -n "$LOADED" ] && rmmod acpi_call; chown -R "${SUDO_UID:-0}:${SUDO_GID:-0}" "$OUT"' EXIT

call() { # op cmd arg arg2
    local buf="0x00 $1 0x00 $2 $3 0x00 $4 0x00"
    for _ in $(seq 24); do buf="$buf 0x00"; done
    echo "$METHOD 0x0 0x1 {$buf}" > /proc/acpi/call
    printf '%s cmd=%s arg=%s arg2=%s -> %s\n' "$1" "$2" "$3" "$4" "$(tr -d '\0' < /proc/acpi/call)"
}

{
    echo "== $(date -Is) $(uname -r)"
    cat /sys/class/dmi/id/{sys_vendor,product_name,product_sku,bios_version}
    echo "AC: $(cat /sys/class/power_supply/ADP1/online 2>/dev/null)  BAT: $(cat /sys/class/power_supply/BAT0/{capacity,status} 2>/dev/null | tr '\n' ' ')"
    echo "wpctl: $(sudo -u "#${SUDO_UID:-0}" XDG_RUNTIME_DIR=/run/user/${SUDO_UID:-0} wpctl get-volume @DEFAULT_AUDIO_SOURCE@ 2>&1)"
    echo "== известные команды"
    call 0xfa 0x08 0x00 0x00
    call 0xfa 0x0a 0x05 0x00
    call 0xfa 0x10 0x02 0x00
    echo "== подкоманды 0x0A и 0x10"
    for a in 0x00 0x01 0x02 0x03 0x04 0x06; do call 0xfa 0x0a $a 0x00; done
    for a in 0x00 0x01 0x03 0x04; do call 0xfa 0x10 $a 0x00; done
    echo "== функции, которые читает драйвер ядра bitland-mifs-wmi"
    for c in 0x09 0x0d 0x10 0x12 0x13 0x16; do call 0xfa $c 0x00 0x00; done
} 2>&1 | tee "$OUT/calls.txt"

for t in /sys/firmware/acpi/tables/{DSDT,SSDT*}; do cp "$t" "$OUT/tables/"; done
echo
echo "Готово: $OUT"
