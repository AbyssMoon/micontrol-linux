#!/bin/bash
# Собирает переносную папку dist/micontrol-portable: скопировал на ноутбук,
# запустил ./run.sh — в системе ничего не устанавливается.
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
# shellcheck source=../env.sh
source "$ROOT/env.sh"

OUT="$ROOT/dist/micontrol-portable"
QT="$QT_DIR"

cargo build --release --workspace --manifest-path "$ROOT/Cargo.toml"

rm -rf "$OUT"
mkdir -p "$OUT"/{bin,lib,plugins,qml,state}

cp "$ROOT"/target/release/{micontrold,micontrol,micontrol-gui} "$OUT/bin/"

# QML-модули, которые использует интерфейс.
cp -a "$QT/qml/QtQuick" "$QT/qml/QtQml" "$OUT/qml/"
rm -rf "$OUT/qml/QtQuick/"{Particles,Dialogs,LocalStorage,Effects,VectorImage,Shapes,tooling}
# Значок в трее и его меню (Qt.labs.platform).
mkdir -p "$OUT/qml/Qt/labs"
cp -a "$QT/qml/Qt/labs/platform" "$OUT/qml/Qt/labs/"

# Плагины: X11 и Wayland, картинки, методы ввода.
for d in platforms wayland-decoration-client wayland-graphics-integration-client \
         wayland-shell-integration xcbglintegrations imageformats iconengines \
         platforminputcontexts egldeviceintegrations; do
    [ -d "$QT/plugins/$d" ] && cp -a "$QT/plugins/$d" "$OUT/plugins/"
done
mkdir -p "$OUT/plugins/platformthemes"
cp "$QT"/plugins/platformthemes/libqxdgdesktopportal.so "$OUT/plugins/platformthemes/" 2>/dev/null || true

# Библиотеки Qt: замыкание зависимостей всех бинарников и плагинов,
# берём только то, что лежит в нашем Qt (системные библиотеки не копируем).
mapfile -t objs < <(find "$OUT/bin/micontrol-gui" "$OUT/plugins" "$OUT/qml" -name '*.so*' -o -name micontrol-gui)
declare -A seen=()
queue=("${objs[@]}")
while [ ${#queue[@]} -gt 0 ]; do
    f="${queue[0]}"
    queue=("${queue[@]:1}")
    while read -r lib; do
        name="$(basename "$lib")"
        [ -n "${seen[$name]:-}" ] && continue
        seen[$name]=1
        cp -L "$lib" "$OUT/lib/$name"
        queue+=("$QT/lib/$name")
    done < <(LD_LIBRARY_PATH="$QT/lib" ldd "$f" 2>/dev/null | awk -v q="$QT/lib/" 'index($3, q) == 1 { print $3 }')
done

# Исходники acpi_call — на случай, если модуля нет в системе ноутбука.
mkdir -p "$OUT/acpi_call"
cp "$ROOT"/third_party/acpi_call/{acpi_call.c,Makefile,LICENSE,VERSION} "$OUT/acpi_call/"
# Модуль, собранный под текущее ядро этой машины (scripts/build-acpi_call.sh).
if [ -f "$ROOT/build/acpi_call/acpi_call.ko" ]; then
    cp "$ROOT/build/acpi_call/acpi_call.ko" "$OUT/acpi_call/"
    echo "acpi_call.ko для ядра $(modinfo -F vermagic "$OUT/acpi_call/acpi_call.ko" | cut -d' ' -f1)"
fi

cp "$ROOT"/packaging/portable/{run.sh,build-acpi_call.sh,README.txt} "$OUT/"
chmod +x "$OUT"/{run.sh,build-acpi_call.sh}

echo
echo "Готово: $OUT ($(du -sh "$OUT" | cut -f1))"
echo "Максимальная требуемая версия glibc:"
objdump -T "$OUT"/bin/* "$OUT"/lib/* 2>/dev/null | grep -o 'GLIBC_[0-9.]*' | sort -uV | tail -1
