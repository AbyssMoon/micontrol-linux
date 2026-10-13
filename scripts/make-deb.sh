#!/bin/bash
# Собирает .deb для Ubuntu 24.04+ / Debian 13+ → dist/micontrol_<версия>_amd64.deb.
# Root не нужен, нужен dpkg-deb. Ставить: sudo apt install ./dist/micontrol_*.deb
#
#   --no-build   взять уже собранную dist/micontrol-portable
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
BUNDLE="$ROOT/dist/micontrol-portable"
COMMON="$ROOT/packaging/common"
PKG="$ROOT/build/deb/micontrol"

[ "${1:-}" = --no-build ] || "$ROOT/scripts/make-portable.sh" --no-archive
[ -x "$BUNDLE/bin/micontrol-gui" ] || { echo "нет $BUNDLE — запустите без --no-build" >&2; exit 1; }

VERSION="$(sed -n 's/^version = "\(.*\)"/\1/p' "$ROOT/Cargo.toml" | head -1)"
ACPI_VER="$(cat "$ROOT/third_party/acpi_call/VERSION")"

rm -rf "$PKG"
install -d "$PKG/opt/micontrol" "$PKG/usr/bin" "$PKG/var/lib/micontrol"
cp -a "$BUNDLE"/{bin,lib,plugins,qml} "$PKG/opt/micontrol/"

install -Dm755 "$COMMON/micontrol-gui" "$PKG/usr/bin/micontrol-gui"
ln -s ../../opt/micontrol/bin/micontrol "$PKG/usr/bin/micontrol"

install -Dm644 "$COMMON/micontrold.service" "$PKG/usr/lib/systemd/system/micontrold.service"
install -d "$PKG/usr/lib/modules-load.d"
echo acpi_call > "$PKG/usr/lib/modules-load.d/acpi_call.conf"

install -Dm644 "$COMMON/micontrol.desktop" "$PKG/usr/share/applications/micontrol.desktop"
install -Dm644 "$COMMON/micontrol-autostart.desktop" "$PKG/etc/xdg/autostart/micontrol.desktop"
install -Dm644 "$ROOT/crates/micontrol-gui/qml/img/tray_auto.svg" \
    "$PKG/usr/share/icons/hicolor/scalable/apps/micontrol.svg"
install -Dm644 "$ROOT/LICENSE" "$PKG/usr/share/doc/micontrol/copyright"

S="$PKG/usr/src/acpi_call-$ACPI_VER"
install -Dm644 "$ROOT/third_party/acpi_call/acpi_call.c" "$S/acpi_call.c"
install -Dm644 "$ROOT/third_party/acpi_call/Makefile" "$S/Makefile"
sed "s/@VERSION@/$ACPI_VER/" "$COMMON/dkms.conf" > "$S/dkms.conf"

install -d "$PKG/DEBIAN"
sed -e "s/@VERSION@/$VERSION/" -e "s/@SIZE@/$(du -sk "$PKG" | cut -f1)/" \
    "$ROOT/packaging/deb/control" > "$PKG/DEBIAN/control"
for f in postinst prerm postrm; do
    sed "s/@ACPI_VER@/$ACPI_VER/" "$ROOT/packaging/deb/$f" > "$PKG/DEBIAN/$f"
    chmod 755 "$PKG/DEBIAN/$f"
done
echo /etc/xdg/autostart/micontrol.desktop > "$PKG/DEBIAN/conffiles"

DEB="$ROOT/dist/micontrol_${VERSION}_amd64.deb"
rm -f "$ROOT"/dist/micontrol_*.deb
dpkg-deb --root-owner-group -Zxz --build "$PKG" "$DEB"
echo
echo "Готово: $DEB ($(du -h "$DEB" | cut -f1))"
echo "Установка: sudo apt install $DEB"
