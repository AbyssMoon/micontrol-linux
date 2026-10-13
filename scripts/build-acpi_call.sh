#!/bin/bash
# Собирает acpi_call.ko под текущее ядро в build/acpi_call (в систему ничего не ставится).
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
mkdir -p "$ROOT/build/acpi_call"
cp "$ROOT"/third_party/acpi_call/{acpi_call.c,Makefile} "$ROOT/build/acpi_call/"
make -C "/lib/modules/$(uname -r)/build" M="$ROOT/build/acpi_call" modules
