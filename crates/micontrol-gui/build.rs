use cxx_qt_build::{CxxQtBuilder, QmlModule};

fn main() {
    CxxQtBuilder::new_qml_module(
        QmlModule::new("org.micontrol").qml_files(["qml/Main.qml", "qml/ModeTile.qml"]),
    )
    .qt_module("Gui")
    .qt_module("Quick")
    .qt_module("QuickControls2")
    .qrc_resources([
        "qml/img/special_button.svg",
        "qml/img/fn_f7.svg",
        "qml/img/mode_silent.svg",
        "qml/img/mode_balance.svg",
        "qml/img/mode_auto.svg",
        "qml/img/mode_turbo.svg",
        "qml/img/mode_full_speed.svg",
        "qml/img/tray_silent.svg",
        "qml/img/tray_balance.svg",
        "qml/img/tray_auto.svg",
        "qml/img/tray_turbo.svg",
        "qml/img/tray_full_speed.svg",
        "qml/img/tray_unknown.svg",
        "qml/img/mic_on.svg",
        "qml/img/mic_off.svg",
        "qml/img/nav_perf.svg",
        "qml/img/nav_perf_on.svg",
        "qml/img/nav_battery.svg",
        "qml/img/nav_battery_on.svg",
        "qml/img/nav_mic.svg",
        "qml/img/nav_mic_on.svg",
        "qml/img/nav_touchpad.svg",
        "qml/img/nav_touchpad_on.svg",
        "qml/img/nav_keys.svg",
        "qml/img/nav_keys_on.svg",
        "qml/img/nav_system.svg",
        "qml/img/nav_system_on.svg",
        "qml/img/nav_about.svg",
        "qml/img/nav_about_on.svg",
    ])
    .files(["src/controller.rs"])
    .build();
}
