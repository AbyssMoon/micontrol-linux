#!/bin/bash
# Ставит всё для сборки в .toolchain/ внутри проекта (в систему ничего не ставится):
# Rust через rustup, Qt 6.8.3 через aqtinstall.
# Нужны: curl, python3, gcc/g++, make и системная libGL (mesa).
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
TC="$ROOT/.toolchain"
QT_VERSION=6.8.3
mkdir -p "$TC"

export RUSTUP_HOME="$TC/rustup" CARGO_HOME="$TC/cargo"
if [ ! -x "$CARGO_HOME/bin/cargo" ]; then
    curl -sSf https://sh.rustup.rs -o "$TC/rustup-init.sh"
    sh "$TC/rustup-init.sh" -y --no-modify-path --profile minimal
fi

if [ ! -d "$TC/qt/$QT_VERSION/gcc_64" ]; then
    python3 -m venv "$TC/venv"
    "$TC/venv/bin/pip" install --quiet aqtinstall
    "$TC/venv/bin/aqt" install-qt linux desktop "$QT_VERSION" linux_gcc_64 -O "$TC/qt"
fi

# Линковщику нужен libGL.so (dev-симлинк), а в системе часто есть только libGL.so.1.
mkdir -p "$TC/linkfix"
GL="$(/sbin/ldconfig -p | awk '/libGL\.so\.1 .*x86-64/ {print $NF; exit}')"
[ -n "$GL" ] || { echo "не найдена libGL.so.1 (пакет mesa-libGL / libgl1)" >&2; exit 1; }
ln -sf "$GL" "$TC/linkfix/libGL.so"

echo "Готово. Дальше: source env.sh && cargo build --workspace"
