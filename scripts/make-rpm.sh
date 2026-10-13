#!/bin/bash
# Собирает RPM для Fedora → dist/micontrol-<версия>-1.<dist>.x86_64.rpm.
# Root не нужен; ставить: sudo dnf install dist/micontrol-*.rpm
#
#   --no-build   взять уже собранную dist/micontrol-portable
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
BUNDLE="$ROOT/dist/micontrol-portable"
TOP="$ROOT/build/rpm"

[ "${1:-}" = --no-build ] || "$ROOT/scripts/make-portable.sh" --no-archive
[ -x "$BUNDLE/bin/micontrol-gui" ] || { echo "нет $BUNDLE — запустите без --no-build" >&2; exit 1; }

VERSION="$(sed -n 's/^version = "\(.*\)"/\1/p' "$ROOT/Cargo.toml" | head -1)"
ACPI_VER="$(cat "$ROOT/third_party/acpi_call/VERSION")"

rm -rf "$TOP"
rpmbuild -bb "$ROOT/packaging/rpm/micontrol.spec" \
    --define "_topdir $TOP" \
    --define "mc_version $VERSION" \
    --define "acpi_ver $ACPI_VER" \
    --define "bundle $BUNDLE" \
    --define "pkgsrc $ROOT/packaging/rpm" \
    --define "acpisrc $ROOT/third_party/acpi_call" \
    --define "icon $ROOT/crates/micontrol-gui/qml/img/tray_auto.svg" \
    --define "licfile $ROOT/LICENSE"

rm -f "$ROOT"/dist/micontrol-*.rpm
cp "$TOP"/RPMS/x86_64/micontrol-*.rpm "$ROOT/dist/"
RPM="$(ls "$ROOT"/dist/micontrol-*.rpm)"
echo
echo "Готово: $RPM ($(du -h "$RPM" | cut -f1))"
echo "Установка: sudo dnf install $RPM"
