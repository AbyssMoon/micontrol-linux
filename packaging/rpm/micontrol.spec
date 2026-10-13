# Собирается из готовой переносной сборки: scripts/make-rpm.sh передаёт пути
# через --define (bundle, pkgsrc, acpisrc, icon, licfile) и версии (mc_version, acpi_ver).

# Qt и ICU лежат внутри /opt/micontrol: не объявлять их как Provides и не
# требовать системные libQt6*. Остальные зависимости (libGL, xcb, wayland…)
# rpmbuild находит сам.
%global __provides_exclude_from ^/opt/micontrol/.*$
%global __requires_exclude ^lib(Qt6|icu).*$
%global debug_package %{nil}
%global _build_id_links none

Name:           micontrol
Version:        %{mc_version}
Release:        1%{?dist}
Summary:        Performance modes and battery charge limit for Xiaomi / REDMI Book laptops
License:        GPL-3.0-or-later
URL:            https://github.com/AbyssMoon/micontrol-linux
ExclusiveArch:  x86_64

Requires:       dkms
Requires:       kernel-devel
Requires:       wireplumber
%{?systemd_requires}

%description
Linux port of MI Control Utility: performance modes, battery charge limit and
microphone LED on Xiaomi / REDMI Book laptops. Includes the micontrold daemon
(systemd), the tray GUI started at login, the micontrol CLI and the acpi_call
kernel module built by DKMS.

%prep
%build

%install
install -d %{buildroot}/opt/micontrol
cp -a %{bundle}/{bin,lib,plugins,qml} %{buildroot}/opt/micontrol/

install -Dm755 %{pkgsrc}/micontrol-gui %{buildroot}%{_bindir}/micontrol-gui
ln -s ../../opt/micontrol/bin/micontrol %{buildroot}%{_bindir}/micontrol

install -Dm644 %{pkgsrc}/micontrold.service %{buildroot}%{_unitdir}/micontrold.service
install -d %{buildroot}%{_modulesloaddir}
echo acpi_call > %{buildroot}%{_modulesloaddir}/acpi_call.conf

install -Dm644 %{pkgsrc}/micontrol.desktop %{buildroot}%{_datadir}/applications/micontrol.desktop
install -Dm644 %{pkgsrc}/micontrol-autostart.desktop %{buildroot}%{_sysconfdir}/xdg/autostart/micontrol.desktop
install -Dm644 %{icon} %{buildroot}%{_datadir}/icons/hicolor/scalable/apps/micontrol.svg

S=%{buildroot}%{_usrsrc}/acpi_call-%{acpi_ver}
install -Dm644 %{acpisrc}/acpi_call.c "$S/acpi_call.c"
install -Dm644 %{acpisrc}/Makefile "$S/Makefile"
sed 's/@VERSION@/%{acpi_ver}/' %{pkgsrc}/dkms.conf > "$S/dkms.conf"

install -d %{buildroot}%{_sharedstatedir}/micontrol
install -Dm644 %{licfile} %{buildroot}%{_defaultlicensedir}/micontrol/LICENSE

%post
# Модуль — под все ядра, для которых есть kernel-devel (в том числе под
# установленное, но ещё не загруженное); новые ядра DKMS подхватит сам.
dkms status -m acpi_call -v %{acpi_ver} | grep -q . || dkms add -m acpi_call -v %{acpi_ver} || :
for k in /lib/modules/*; do
    [ -e "$k/build" ] || continue
    dkms install -m acpi_call -v %{acpi_ver} -k "$(basename "$k")" || \
        echo "micontrol: не удалось собрать acpi_call для $(basename "$k")" >&2
done
modprobe acpi_call || :
%systemd_post micontrold.service
if [ $1 -eq 1 ]; then
    systemctl enable --now micontrold.service || :
fi

%preun
%systemd_preun micontrold.service
if [ $1 -eq 0 ]; then
    modprobe -r acpi_call || :
    dkms remove -m acpi_call -v %{acpi_ver} --all || :
fi

%postun
%systemd_postun_with_restart micontrold.service

%files
%license %{_defaultlicensedir}/micontrol/LICENSE
/opt/micontrol
%{_bindir}/micontrol
%{_bindir}/micontrol-gui
%{_unitdir}/micontrold.service
%{_modulesloaddir}/acpi_call.conf
%{_datadir}/applications/micontrol.desktop
%config(noreplace) %{_sysconfdir}/xdg/autostart/micontrol.desktop
%{_datadir}/icons/hicolor/scalable/apps/micontrol.svg
%{_usrsrc}/acpi_call-%{acpi_ver}
%dir %{_sharedstatedir}/micontrol

%changelog
* Tue Oct 13 2026 AbyssMoon <7919805+AbyssMoon@users.noreply.github.com> - 0.1.1-1
- Пакет RPM: демон (systemd), трей при входе, acpi_call через DKMS
- Интерфейс: --tray, один экземпляр на пользователя
* Tue Oct 13 2026 AbyssMoon <7919805+AbyssMoon@users.noreply.github.com> - 0.1.0-1
- Первый выпуск (только переносная сборка)
