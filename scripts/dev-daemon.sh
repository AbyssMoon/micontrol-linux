#!/bin/bash
# Отладочный запуск на ноутбуке: sudo ./scripts/dev-daemon.sh
# (старый осиротевший демон, если есть, останавливается)
# Грузит build/acpi_call/acpi_call.ko, запускает target/debug/micontrold --debug
# на /run/micontrol-dev.sock (сокет 0600 для вызвавшего sudo пользователя) и
# пишет лог в build/daemon.log. Ctrl+C — остановить, модуль выгружается.
# Пока он работает, клиенты обходятся без root:
#   MICONTROL_SOCKET=/run/micontrol-dev.sock ./target/debug/micontrol status
set -uo pipefail
ROOT="$(cd "$(dirname "$(readlink -f "$0")")/.." && pwd)"
SOCK=/run/micontrol-dev.sock
[ "$(id -u)" = 0 ] || { echo "нужен root: sudo $0" >&2; exit 1; }

pkill -f "micontrold --debug --socket $SOCK" && sleep 0.5
LOADED=""
if [ ! -e /proc/acpi/call ]; then
    insmod "$ROOT/build/acpi_call/acpi_call.ko" || exit 1
    LOADED=1
fi
mkdir -p "$ROOT/build/dev-state"
chown "${SUDO_UID:-0}:${SUDO_GID:-0}" "$ROOT/build/dev-state"
PID=""
cleanup() {
    [ -n "$PID" ] && kill "$PID" 2>/dev/null && wait "$PID" 2>/dev/null
    rm -f "$SOCK"
    [ -n "$LOADED" ] && rmmod acpi_call && echo "acpi_call выгружен"
    chown -R "${SUDO_UID:-0}:${SUDO_GID:-0}" "$ROOT/build/dev-state" "$ROOT/build/daemon.log" 2>/dev/null
}
trap cleanup EXIT

# Перезапуск демона после пересборки: файл build/dev-restart.
while true; do
    # Без конвейера: $! должен быть PID самого демона.
    "$ROOT/target/debug/micontrold" --debug --socket "$SOCK" \
        --state "$ROOT/build/dev-state/state.json" >>"$ROOT/build/daemon.log" 2>&1 &
    PID=$!
    echo "демон запущен; лог $ROOT/build/daemon.log"
    while kill -0 "$PID" 2>/dev/null && [ ! -e "$ROOT/build/dev-restart" ]; do sleep 0.5; done
    kill "$PID" 2>/dev/null
    wait "$PID" 2>/dev/null
    PID=""
    if [ -e "$ROOT/build/dev-restart" ]; then
        rm -f "$ROOT/build/dev-restart"
        echo "--- перезапуск после пересборки"
        continue
    fi
    break
done
