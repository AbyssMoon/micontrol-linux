#!/bin/bash
# Запуск MI Control прямо из этой папки, без установки в систему.
#
#   ./run.sh           — настоящий режим: демон от root (sudo) + интерфейс
#   ./run.sh --mock    — эмуляция прошивки, без root (проверить интерфейс)
#   ./run.sh probe     — только прочитать регистры прошивки (sudo, без записи)
#
# После закрытия окна демон останавливается, сокет удаляется, а модуль
# acpi_call выгружается, если его загружал этот скрипт.
set -uo pipefail

HERE="$(cd "$(dirname "$(readlink -f "$0")")" && pwd)"
SOCK=/run/micontrol-test.sock
LOG="$HERE/state/daemon.log"
mkdir -p "$HERE/state"

export LD_LIBRARY_PATH="$HERE/lib"
export QT_PLUGIN_PATH="$HERE/plugins"
export QML_IMPORT_PATH="$HERE/qml"
export QML2_IMPORT_PATH="$HERE/qml"

LOADED_MODULE=""
DPID=""

cleanup() {
    trap - EXIT HUP INT TERM
    if [ -n "$DPID" ]; then
        # Сигнал процессу sudo до демона не доходит — останавливаем сам демон.
        sudo pkill -TERM -f -- "bin/micontrold.* --socket $SOCK" 2>/dev/null
        wait "$DPID" 2>/dev/null
        sudo rm -f "$SOCK"
    fi
    if [ "$LOADED_MODULE" = 1 ]; then
        sudo rmmod acpi_call && echo "Модуль acpi_call выгружен."
    fi
}

ensure_acpi_call() {
    [ -e /proc/acpi/call ] && return 0
    # Модуль уже есть в системе (пакет acpi_call) — просто загрузить на время.
    if modinfo acpi_call >/dev/null 2>&1; then
        sudo modprobe acpi_call && LOADED_MODULE=1 && return 0
    fi
    # Модуль, собранный в этой папке (./build-acpi_call.sh).
    local ko="$HERE/acpi_call/acpi_call.ko"
    if [ -f "$ko" ]; then
        sudo insmod "$ko" && LOADED_MODULE=1 && return 0
    fi
    echo "Нет /proc/acpi/call: модуль acpi_call не найден." >&2
    echo "Соберите его в этой папке: ./build-acpi_call.sh (см. README.txt)" >&2
    echo "или проверьте интерфейс в эмуляции: ./run.sh --mock" >&2
    return 1
}

start_daemon() {
    # $@ — дополнительные аргументы демона; стартует в фоне.
    "$@" --socket "$SOCK" --state "$HERE/state/state.json" </dev/null >"$LOG" 2>&1 &
    DPID=$!
    for _ in $(seq 50); do
        [ -S "$SOCK" ] && return 0
        if [ ! -d "/proc/$DPID" ]; then
            echo "Демон завершился с ошибкой:" >&2
            cat "$LOG" >&2
            DPID=""
            return 1
        fi
        sleep 0.1
    done
    echo "Демон не открыл сокет за 5 секунд, лог: $LOG" >&2
    return 1
}

MODE="${1:-}"
[ $# -gt 0 ] && shift

case "$MODE" in
    --mock)
        SOCK="${XDG_RUNTIME_DIR:-/tmp}/micontrol-mock.sock"
        trap cleanup EXIT HUP INT TERM
        # В эмуляции root не нужен — подменяем sudo на прямой запуск.
        sudo() { "$@"; }
        start_daemon "$HERE/bin/micontrold" --mock || exit 1
        ;;
    probe)
        sudo -v || exit 1
        trap cleanup EXIT HUP INT TERM
        ensure_acpi_call || exit 1
        sudo "$HERE/bin/micontrol" probe
        exit
        ;;
    "")
        echo "Нужны права root для демона (ACPI-вызовы)."
        sudo -v || exit 1
        trap cleanup EXIT HUP INT TERM
        ensure_acpi_call || exit 1
        start_daemon sudo "$HERE/bin/micontrold" || exit 1
        echo "Демон запущен, лог: $LOG"
        ;;
    *)
        sed -n '2,10p' "$0"
        exit 2
        ;;
esac

# Остальные аргументы передаются интерфейсу (например, --page N).
MICONTROL_SOCKET="$SOCK" "$HERE/bin/micontrol-gui" "$@"
