#!/bin/bash
# Собирает модуль ядра acpi_call внутри этой папки (acpi_call/acpi_call.ko).
# В систему ничего не копируется; run.sh загрузит модуль через insmod
# только на время работы и выгрузит при выходе.
#
# Нужны заголовки текущего ядра (/lib/modules/$(uname -r)/build), make и gcc.
set -euo pipefail
HERE="$(cd "$(dirname "$(readlink -f "$0")")" && pwd)"
KDIR="/lib/modules/$(uname -r)/build"
if [ ! -d "$KDIR" ]; then
    echo "Нет заголовков ядра: $KDIR" >&2
    echo "Arch: пакет linux-headers (или linux-zen-headers и т. п.)," >&2
    echo "Fedora: kernel-devel, Debian/Ubuntu: linux-headers-\$(uname -r)." >&2
    echo "Если не хотите их ставить — скажите версию ядра (uname -r) и дистрибутив," >&2
    echo "модуль можно собрать на другой машине." >&2
    exit 1
fi
make -C "$KDIR" M="$HERE/acpi_call" modules
echo "Готово: $HERE/acpi_call/acpi_call.ko"
