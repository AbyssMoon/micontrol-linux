import QtQuick
import QtQuick.Controls
import QtQuick.Layouts

import Qt.labs.platform as Platform

import org.micontrol

ApplicationWindow {
    id: win

    width: 760
    height: 640
    minimumWidth: 560
    minimumHeight: 420
    visible: true
    title: "MI Control"

    Controller {
        id: ctl
    }

    // --- Трей ---
    property bool quitting: false
    property bool trayHintShown: false

    function showWindow() {
        win.show();
        win.raise();
        win.requestActivate();
    }

    onClosing: close => {
        if (tray.available && !win.quitting && win.argValue("--screenshot") === "") {
            close.accepted = false;
            win.hide();
            if (!win.trayHintShown) {
                win.trayHintShown = true;
                tray.showMessage("MI Control работает в трее",
                                 "Правый клик по значку — состояние и быстрые действия.");
            }
        }
    }

    readonly property string traySummary: {
        if (!ctl.connected)
            return "нет связи с демоном";
        let s = [win.modeTitle[ctl.perfMode] || "режим ?"];
        if (ctl.batteryCapacity >= 0)
            s.push(ctl.batteryCapacity + "%");
        return s.join(" · ");
    }

    Platform.SystemTrayIcon {
        id: tray
        visible: available
        icon.source: Qt.resolvedUrl("img/tray_" + (ctl.connected && ctl.perfMode ? ctl.perfMode : "unknown") + ".svg")
        tooltip: "MI Control — " + win.traySummary

        onActivated: reason => {
            if (reason === Platform.SystemTrayIcon.Trigger) {
                if (win.visible)
                    win.hide();
                else
                    win.showWindow();
            }
        }

        menu: Platform.Menu {
            Platform.MenuItem {
                enabled: false
                text: ctl.connected ? "Режим: " + (win.modeTitle[ctl.perfMode] || "?") : "Нет связи с демоном"
            }
            Platform.MenuItem {
                enabled: false
                visible: ctl.cpuTemp >= 0
                text: "CPU: " + (ctl.cpuPower >= 0 ? ctl.cpuPower.toFixed(1) + " Вт · " : "")
                      + (ctl.cpuFreq > 0 ? (ctl.cpuFreq / 1000).toFixed(1) + " ГГц · " : "")
                      + ctl.cpuTemp + " °C"
            }
            Platform.MenuItem {
                enabled: false
                visible: ctl.batteryCapacity >= 0
                text: {
                    let s = "Батарея: " + ctl.batteryCapacity + "%";
                    const p = ctl.batteryPower;
                    if (Math.abs(p) >= 0.1)
                        s += " · " + (p > 0 ? "+" : "−") + Math.abs(p).toFixed(1) + " Вт";
                    if (ctl.chargeLimit !== "" && ctl.chargeLimit !== "off")
                        s += " · лимит " + ctl.chargeLimit + "%";
                    return s;
                }
            }
            Platform.MenuSeparator {}

            Platform.MenuItemGroup { id: trayModes }
            Platform.MenuItem {
                text: "Тихий"; checkable: true; group: trayModes
                checked: ctl.perfMode === "silent"; enabled: ctl.connected
                onTriggered: ctl.setPerf("silent")
            }
            Platform.MenuItem {
                text: "Баланс"; checkable: true; group: trayModes
                visible: !ctl.redmibook2025
                checked: ctl.perfMode === "balance"; enabled: ctl.connected
                onTriggered: ctl.setPerf("balance")
            }
            Platform.MenuItem {
                text: "Авто"; checkable: true; group: trayModes
                visible: ctl.redmibook2025
                checked: ctl.perfMode === "auto"; enabled: ctl.connected
                onTriggered: ctl.setPerf("auto")
            }
            Platform.MenuItem {
                text: "Турбо"; checkable: true; group: trayModes
                checked: ctl.perfMode === "turbo"; enabled: ctl.connected
                onTriggered: ctl.setPerf("turbo")
            }
            Platform.MenuItem {
                text: "Максимум"; checkable: true; group: trayModes
                checked: ctl.perfMode === "full_speed"; enabled: ctl.connected
                onTriggered: ctl.setPerf("full_speed")
            }
            Platform.MenuItem {
                text: "Полная мощность режимов"; checkable: true
                visible: ctl.pl1Dtt > 0
                checked: ctl.pl1Sync; enabled: ctl.connected
                onTriggered: ctl.requestPl1Sync(!ctl.pl1Sync)
            }
            Platform.MenuSeparator {}
            Platform.MenuItem {
                text: win.visible ? "Скрыть окно" : "Открыть окно"
                onTriggered: win.visible ? win.hide() : win.showWindow()
            }
            Platform.MenuItem {
                text: "Выход"
                onTriggered: {
                    win.quitting = true;
                    Qt.quit();
                }
            }
        }
    }

    // Отладка: micontrol-gui --page N --screenshot файл.png
    function argValue(name) {
        const a = Qt.application.arguments;
        const i = a.indexOf(name);
        return i >= 0 && i + 1 < a.length ? a[i + 1] : "";
    }

    Timer {
        interval: 800
        running: win.argValue("--screenshot") !== ""
        onTriggered: body.grabToImage(r => {
            r.saveToFile(win.argValue("--screenshot"));
            Qt.quit();
        })
    }

    Component.onCompleted: {
        const page = win.argValue("--page");
        if (page !== "")
            nav.currentIndex = parseInt(page);
    }

    Timer {
        // Режим может смениться по Fn+K, а микрофон — по Fn+F4.
        interval: win.visible ? 2000 : 5000
        running: true
        repeat: true
        triggeredOnStart: true
        onTriggered: ctl.refresh()
    }

    readonly property var modeTitle: ({
        silent: "Тихий", balance: "Баланс", auto: "Авто", turbo: "Турбо", full_speed: "Максимум"
    })
    readonly property var modeColor: ({
        silent: "#38bdf8", balance: "#34d399", auto: "#a78bfa", turbo: "#fbbf24", full_speed: "#f43f5e"
    })
    readonly property var modeHint: ({
        silent: "Тише и холоднее, дольше от батареи",
        balance: "Баланс тишины и скорости",
        auto: "Прошивка подбирает сама",
        turbo: "Быстро, вентиляторы громче",
        full_speed: "Вентиляторы на максимум"
    })

    // Замеры на REDMI Book Pro 14 2025 (TM2411, Core Ultra 255H): длительная
    // нагрузка на все ядра; on/off — с «Полной мощностью» и без.
    readonly property var powerTable: ({
        silent:     { on:  { ac: "35–45 Вт · 2,4 ГГц", dc: "17 Вт · 1,8 ГГц" },
                      off: { ac: "28 Вт · 2,5 ГГц",    dc: "22 Вт · 2,0 ГГц" } },
        auto:       { on:  { ac: "65 Вт · 3,5 ГГц",    dc: "36 Вт · 2,5 ГГц" },
                      off: { ac: "28 Вт · 2,5 ГГц",    dc: "28 Вт · 2,3 ГГц" } },
        turbo:      { on:  { ac: "80 Вт · 3,5 ГГц",    dc: "36 Вт · 2,5 ГГц" },
                      off: { ac: "28 Вт · 2,5 ГГц",    dc: "28 Вт · 2,4 ГГц" } },
        full_speed: { on:  { ac: "90 Вт · 3,5 ГГц",    dc: "36 Вт · 2,5 ГГц" },
                      off: { ac: "28 Вт · 2,5 ГГц",    dc: "28 Вт · 2,3 ГГц" } }
    })

    function modePower(mode, sync, ac) {
        const row = ctl.redmibook2025 ? powerTable[mode] : undefined;
        return row ? row[sync ? "on" : "off"][ac ? "ac" : "dc"] : "";
    }

    readonly property var pages: [
        { title: "Производительность" },
        { title: "Батарея" },
        { title: "Микрофон" },
        { title: "Система" },
        { title: "О программе" }
    ]

    header: ToolBar {
        RowLayout {
            anchors.fill: parent
            anchors.leftMargin: 12
            anchors.rightMargin: 12

            Label {
                text: "MI Control"
                font.pixelSize: 18
                font.bold: true
            }
            Item { Layout.fillWidth: true }
            Rectangle {
                width: 10
                height: 10
                radius: 5
                color: ctl.connected ? "#3cb371" : "#d9534f"
            }
            Label {
                text: ctl.connected ? ctl.backend : "нет связи с демоном"
                elide: Text.ElideRight
                Layout.maximumWidth: 320
            }
        }
    }

    Pane {
        id: body
        anchors.fill: parent
        padding: 0

        ColumnLayout {
            anchors.fill: parent
            spacing: 0

            // Предупреждения
            Pane {
                Layout.fillWidth: true
                visible: warning.text.length > 0
                background: Rectangle { color: "#fff3cd" }

                Label {
                    id: warning
                    width: parent.width
                    wrapMode: Text.Wrap
                    color: "#664d03"
                    text: {
                        if (!ctl.connected)
                            return ctl.errorText + "\nДемон не запущен — запустите ./run.sh";
                        let s = ctl.actionError;
                        if (ctl.errorText)
                            s = (s ? s + "\n" : "") + ctl.errorText;
                        if (!ctl.isXiaomi && !ctl.backend.startsWith("mock"))
                            s = (s ? s + "\n" : "") + "Это не ноутбук Xiaomi — команды, скорее всего, не сработают.";
                        return s;
                    }
                }
            }

            RowLayout {
                Layout.fillWidth: true
                Layout.fillHeight: true
                spacing: 0

                ListView {
                    id: nav
                    Layout.preferredWidth: 190
                    Layout.fillHeight: true
                    model: win.pages
                    currentIndex: 0
                    focus: true
                    keyNavigationEnabled: true

                    delegate: ItemDelegate {
                        required property var modelData
                        required property int index
                        width: ListView.view.width
                        text: modelData.title
                        highlighted: ListView.isCurrentItem
                        onClicked: nav.currentIndex = index
                    }
                }

                ToolSeparator {
                    orientation: Qt.Vertical
                    Layout.fillHeight: true
                }

                StackLayout {
                    Layout.fillWidth: true
                    Layout.fillHeight: true
                    Layout.margins: 16
                    currentIndex: nav.currentIndex

                    // --- Производительность ---
                    ScrollView {
                        id: perfScroll
                        clip: true
                        contentWidth: availableWidth

                    ColumnLayout {
                        width: perfScroll.availableWidth
                        spacing: 14

                        Label {
                            text: "Производительность"
                            font.pixelSize: 16
                            font.bold: true
                        }

                        RowLayout {
                            Layout.fillWidth: true
                            spacing: 10

                            Repeater {
                                model: [
                                    { mode: "silent", show: true },
                                    { mode: "balance", show: !ctl.redmibook2025 },
                                    { mode: "auto", show: ctl.redmibook2025 || ctl.perfMode === "auto" },
                                    { mode: "turbo", show: true },
                                    { mode: "full_speed", show: true }
                                ]

                                ModeTile {
                                    required property var modelData
                                    readonly property string mode: modelData.mode
                                    visible: modelData.show
                                    Layout.fillWidth: true
                                    Layout.preferredWidth: 1
                                    imageSize: 64
                                    image: "img/mode_" + mode + ".svg"
                                    caption: win.modeTitle[mode]
                                    subtitle: win.modePower(mode, ctl.pl1Sync, ctl.acOnline !== 0)
                                    accent: win.modeColor[mode]
                                    checked: ctl.perfMode === mode
                                    enabled: ctl.connected
                                    ToolTip.visible: hovered
                                    ToolTip.delay: 400
                                    ToolTip.text: {
                                        let t = win.modeHint[mode];
                                        const ac = win.modePower(mode, ctl.pl1Sync, true);
                                        if (ac)
                                            t += "\nОт сети: " + ac + "\nОт батареи: " + win.modePower(mode, ctl.pl1Sync, false);
                                        return t;
                                    }
                                    // Не даём кнопке переключиться до ответа демона.
                                    onClicked: {
                                        checked = Qt.binding(() => ctl.perfMode === mode);
                                        ctl.setPerf(mode);
                                    }
                                }
                            }
                        }

                        Label {
                            visible: ctl.connected && ctl.perfRaw >= 0 && ctl.perfMode === ""
                            text: "Прошивка сообщает режим без названия: 0x" + ctl.perfRaw.toString(16).padStart(2, "0")
                        }

                        // Карточка «сейчас».
                        Rectangle {
                            Layout.fillWidth: true
                            implicitHeight: nowCol.implicitHeight + 28
                            radius: 12
                            color: "#1f1f1f"
                            visible: ctl.cpuTemp >= 0 || ctl.pl1Msr > 0

                            ColumnLayout {
                                id: nowCol
                                anchors.fill: parent
                                anchors.margins: 14
                                spacing: 8

                                Row {
                                    id: metrics
                                    Layout.fillWidth: true

                                    Repeater {
                                        model: [
                                            { v: ctl.cpuPower >= 0 ? ctl.cpuPower.toFixed(1) : "—", u: "Вт", l: "потребление CPU" },
                                            { v: ctl.cpuFreq > 0 ? (ctl.cpuFreq / 1000).toFixed(2) : "—", u: "ГГц", l: "частота" },
                                            { v: ctl.cpuTemp >= 0 ? ctl.cpuTemp : "—", u: "°C", l: "температура",
                                              hot: ctl.cpuTemp >= 95 }
                                        ]
                                        Column {
                                            required property var modelData
                                            width: metrics.width / 3
                                            Label {
                                                anchors.horizontalCenter: parent.horizontalCenter
                                                textFormat: Text.StyledText
                                                text: "<b>" + modelData.v + "</b> <small>" + modelData.u + "</small>"
                                                color: modelData.hot ? "#f43f5e" : "white"
                                                font.pixelSize: 24
                                            }
                                            Label {
                                                anchors.horizontalCenter: parent.horizontalCenter
                                                text: modelData.l
                                                color: "#8a8a8a"
                                                font.pixelSize: 11
                                            }
                                        }
                                    }
                                }

                                Label {
                                    Layout.alignment: Qt.AlignHCenter
                                    visible: ctl.pl1Msr > 0
                                    color: "#8a8a8a"
                                    font.pixelSize: 12
                                    text: "лимит " + (ctl.pl1Dtt > 0 ? Math.min(ctl.pl1Dtt, ctl.pl1Msr) : ctl.pl1Msr) + " Вт"
                                        + (ctl.pl2 > 0 ? " · кратковременно " + ctl.pl2 + " Вт" : "")
                                }
                            }
                        }

                        // Полная мощность режимов.
                        RowLayout {
                            visible: ctl.pl1Dtt > 0
                            Layout.fillWidth: true
                            spacing: 10

                            Switch {
                                id: pl1Switch
                                enabled: ctl.connected
                                checked: ctl.pl1Sync
                                onToggled: {
                                    const want = checked;
                                    checked = Qt.binding(() => ctl.pl1Sync);
                                    ctl.requestPl1Sync(want);
                                }
                            }
                            ColumnLayout {
                                Layout.fillWidth: true
                                spacing: 0
                                Label {
                                    text: "Полная мощность режимов"
                                    font.bold: true
                                }
                                Label {
                                    Layout.fillWidth: true
                                    wrapMode: Text.Wrap
                                    opacity: 0.65
                                    font.pixelSize: 12
                                    text: ctl.pl1Sync
                                        ? "Режимы работают в полную силу. Выключите, чтобы вернуть лимит BIOS."
                                        : "BIOS ограничивает CPU до " + ctl.pl1Msr + " Вт в любом режиме. "
                                          + "Включите: от сети до +40% скорости, от батареи около +10% и выше расход."
                                }
                            }
                        }

                        Label {
                            text: "Fn+K — переключение режима с клавиатуры"
                            opacity: 0.5
                            font.pixelSize: 12
                        }
                        Item { Layout.fillHeight: true }
                    }
                    }

                    // --- Батарея ---
                    ScrollView {
                        id: batScroll
                        clip: true
                        contentWidth: availableWidth

                    ColumnLayout {
                        id: batPage
                        width: batScroll.availableWidth
                        spacing: 14

                        // Лимит из прошивки в процентах (100 — без ограничения).
                        readonly property int limitPct: ctl.chargeLimit === "" || ctl.chargeLimit === "off"
                                                        ? 100 : parseInt(ctl.chargeLimit)
                        // Значение под ползунком во время перетаскивания.
                        readonly property int shownPct: limitSlider.pressed ? limitSlider.value : limitPct

                        function hm(hours) {
                            if (!isFinite(hours) || hours <= 0 || hours > 48)
                                return "";
                            const h = Math.floor(hours), m = Math.round((hours - h) * 60);
                            return h > 0 ? h + " ч " + m + " мин" : m + " мин";
                        }

                        Label {
                            text: "Батарея"
                            font.pixelSize: 16
                            font.bold: true
                        }

                        // Карточка состояния.
                        Rectangle {
                            Layout.fillWidth: true
                            implicitHeight: card.implicitHeight + 32
                            radius: 12
                            color: "#1f1f1f"

                            ColumnLayout {
                                id: card
                                anchors.fill: parent
                                anchors.margins: 16
                                spacing: 10

                                RowLayout {
                                    spacing: 16

                                    Label {
                                        text: ctl.batteryCapacity >= 0 ? ctl.batteryCapacity + "%" : "—"
                                        color: "white"
                                        font.pixelSize: 40
                                        font.bold: true
                                    }
                                    ColumnLayout {
                                        spacing: 2
                                        Label {
                                            color: "white"
                                            font.pixelSize: 15
                                            text: {
                                                const st = { "Charging": "⚡ Заряжается", "Discharging": "Разряжается",
                                                             "Not charging": "⏸ Держит лимит", "Full": "Заряжена" };
                                                return st[ctl.batteryStatus] || ctl.batteryStatus || "—";
                                            }
                                        }
                                        Label {
                                            color: "#bbbbbb"
                                            text: {
                                                const p = ctl.batteryPower;
                                                let s = ctl.acOnline === 1 ? "от сети" : "от батареи";
                                                if (Math.abs(p) >= 0.1)
                                                    s += " · " + (p > 0 ? "+" : "−") + Math.abs(p).toFixed(1) + " Вт";
                                                const e = ctl.batteryEnergy, f = ctl.batteryEnergyFull;
                                                if (p < -0.5 && e > 0)
                                                    s += " · осталось ~" + batPage.hm(e / -p);
                                                else if (p > 0.5 && e > 0 && f > 0) {
                                                    const t = batPage.hm((f * batPage.limitPct / 100 - e) / p);
                                                    if (t) s += " · до " + batPage.limitPct + "% ~" + t;
                                                }
                                                return s;
                                            }
                                        }
                                    }
                                }

                                // Полоса заряда с отметкой лимита.
                                Item {
                                    Layout.fillWidth: true
                                    implicitHeight: 46

                                    Rectangle {
                                        id: bar
                                        width: parent.width - 8
                                        height: 26
                                        radius: 6
                                        color: "#333333"
                                        clip: true

                                        // Зона выше лимита.
                                        Rectangle {
                                            x: parent.width * batPage.shownPct / 100
                                            width: parent.width - x
                                            height: parent.height
                                            color: "#2a2a2a"
                                            visible: batPage.shownPct < 100
                                            Behavior on x { NumberAnimation { duration: 150 } }
                                        }
                                        Rectangle {
                                            width: parent.width * Math.max(0, ctl.batteryCapacity) / 100
                                            height: parent.height
                                            radius: 6
                                            color: ctl.batteryStatus === "Charging" ? "#ff6900"
                                                 : ctl.batteryCapacity <= 15 ? "#d9534f" : "#3cb371"
                                            Behavior on width { NumberAnimation { duration: 300 } }
                                        }
                                    }
                                    // Носик батареи.
                                    Rectangle {
                                        x: bar.width + 2; y: 7
                                        width: 5; height: 12; radius: 2
                                        color: "#333333"
                                    }
                                    // Отметка лимита.
                                    Rectangle {
                                        visible: batPage.shownPct < 100
                                        x: bar.width * batPage.shownPct / 100 - 1
                                        y: -3
                                        width: 3; height: bar.height + 6; radius: 1
                                        color: "white"
                                        Behavior on x { NumberAnimation { duration: 150 } }
                                    }
                                    Label {
                                        visible: batPage.shownPct < 100
                                        x: Math.min(bar.width * batPage.shownPct / 100 - width / 2, bar.width - width)
                                        y: bar.height + 3
                                        text: "лимит " + batPage.shownPct + "%"
                                        color: "#dddddd"
                                        font.pixelSize: 11
                                    }
                                }

                                Label {
                                    color: "#999999"
                                    font.pixelSize: 12
                                    visible: text.length > 0
                                    text: {
                                        let s = [];
                                        if (ctl.batteryHealth > 0)
                                            s.push("здоровье " + ctl.batteryHealth + "%");
                                        if (ctl.batteryEnergy > 0 && ctl.batteryEnergyFull > 0)
                                            s.push(ctl.batteryEnergy.toFixed(1) + " из " + ctl.batteryEnergyFull.toFixed(1) + " Вт·ч");
                                        return s.join(" · ");
                                    }
                                }
                            }
                        }

                        // Выбор лимита.
                        Label {
                            text: "Заряжать до"
                            font.pixelSize: 15
                            font.bold: true
                        }
                        RowLayout {
                            Layout.fillWidth: true
                            spacing: 12

                            Slider {
                                id: limitSlider
                                Layout.fillWidth: true
                                from: 40; to: 100; stepSize: 10
                                snapMode: Slider.SnapAlways
                                enabled: ctl.connected
                                value: batPage.limitPct
                                onPressedChanged: {
                                    if (pressed)
                                        return;
                                    const v = Math.round(value);
                                    value = Qt.binding(() => batPage.limitPct);
                                    if (v !== batPage.limitPct)
                                        ctl.setCharge(v >= 100 ? "off" : String(v));
                                }

                                background: Item {
                                    x: limitSlider.leftPadding
                                    y: limitSlider.topPadding + limitSlider.availableHeight / 2 - 3
                                    width: limitSlider.availableWidth
                                    height: 6
                                    Rectangle {
                                        anchors.fill: parent
                                        radius: 3
                                        color: "#d0d0d0"
                                    }
                                    // Ручка ходит по (ширина − 24): деления и заливка — по тем же точкам.
                                    Rectangle {
                                        width: 12 + limitSlider.visualPosition * (parent.width - 24)
                                        height: parent.height
                                        radius: 3
                                        color: "#ff6900"
                                    }
                                    Repeater {
                                        model: 7
                                        Rectangle {
                                            required property int index
                                            x: 12 + (parent.width - 24) * index / 6 - 2
                                            y: -3
                                            width: 4; height: 12; radius: 2
                                            color: index / 6 <= limitSlider.visualPosition ? "#ff6900" : "#b0b0b0"
                                        }
                                    }
                                }
                                handle: Rectangle {
                                    x: limitSlider.leftPadding + limitSlider.visualPosition * (limitSlider.availableWidth - width)
                                    y: limitSlider.topPadding + limitSlider.availableHeight / 2 - height / 2
                                    width: 24; height: 24; radius: 12
                                    color: limitSlider.pressed ? "#ff8a3d" : "#ff6900"
                                    border.color: "white"
                                    border.width: 3
                                }
                            }
                            Label {
                                Layout.preferredWidth: 130
                                text: batPage.shownPct >= 100 ? "100% · без лимита" : batPage.shownPct + "%"
                                font.pixelSize: 15
                                font.bold: true
                            }
                        }
                        // Подписи делений.
                        Item {
                            Layout.fillWidth: true
                            Layout.rightMargin: 142
                            implicitHeight: 16
                            Repeater {
                                model: [40, 50, 60, 70, 80, 90, 100]
                                Label {
                                    required property int index
                                    required property int modelData
                                    x: limitSlider.leftPadding + (limitSlider.availableWidth - 24) * index / 6 + 12 - width / 2
                                    text: modelData
                                    font.pixelSize: 11
                                    opacity: 0.6
                                }
                            }
                        }

                        // Быстрый выбор.
                        RowLayout {
                            spacing: 8
                            Repeater {
                                model: [
                                    { pct: 60, text: "Всегда от сети", hint: "Ноутбук почти всё время на зарядке — бережёт батарею сильнее всего" },
                                    { pct: 80, text: "Сбалансированно", hint: "Хороший запас хода и щадящий режим для батареи" },
                                    { pct: 100, text: "В дорогу", hint: "Полный заряд, когда нужен максимум автономности" }
                                ]
                                AbstractButton {
                                    id: preset
                                    required property var modelData
                                    readonly property bool active: batPage.limitPct === modelData.pct
                                    text: modelData.text
                                    enabled: ctl.connected
                                    hoverEnabled: true
                                    implicitWidth: presetText.implicitWidth + 28
                                    implicitHeight: 34
                                    ToolTip.visible: hovered
                                    ToolTip.delay: 400
                                    ToolTip.text: modelData.hint + " (" + modelData.pct + "%)"
                                    background: Rectangle {
                                        radius: 17
                                        color: preset.active ? "#ff6900" : (preset.hovered ? "#333333" : "#1f1f1f")
                                        opacity: preset.enabled ? 1 : 0.4
                                        Behavior on color { ColorAnimation { duration: 120 } }
                                    }
                                    contentItem: Label {
                                        id: presetText
                                        text: preset.text + "  " + preset.modelData.pct + "%"
                                        color: "white"
                                        font.bold: preset.active
                                        horizontalAlignment: Text.AlignHCenter
                                        verticalAlignment: Text.AlignVCenter
                                    }
                                    onClicked: ctl.setCharge(modelData.pct >= 100 ? "off" : String(modelData.pct))
                                }
                            }
                        }

                        Label {
                            Layout.fillWidth: true
                            wrapMode: Text.Wrap
                            opacity: 0.7
                            text: "Зарядка останавливается на выбранном уровне; если заряд уже выше, он не сбрасывается, "
                                + "а просто расходуется до лимита. Выбор сохраняется и применяется снова после сна и переподключения зарядки."
                                + (ctl.chargeRaw >= 0 && ctl.chargeLimit === ""
                                   ? "\nПрошивка сообщает неизвестное значение: 0x" + ctl.chargeRaw.toString(16).padStart(2, "0") : "")
                        }
                        Item { Layout.fillHeight: true }
                    }
                    }

                    // --- Микрофон ---
                    ColumnLayout {
                        spacing: 12

                        Label {
                            text: "Микрофон"
                            font.pixelSize: 16
                            font.bold: true
                        }

                        RowLayout {
                            spacing: 10

                            ModeTile {
                                image: "img/mic_off.svg"
                                accent: "#f43f5e"
                                imageSize: 80
                                caption: "Выключен"
                                checked: ctl.micState === 0
                                enabled: ctl.connected
                                onClicked: {
                                    checked = Qt.binding(() => ctl.micState === 0);
                                    ctl.setMic(false);
                                }
                            }
                            ModeTile {
                                image: "img/mic_on.svg"
                                accent: "#34d399"
                                imageSize: 80
                                caption: "Включён"
                                checked: ctl.micState === 1
                                enabled: ctl.connected
                                onClicked: {
                                    checked = Qt.binding(() => ctl.micState === 1);
                                    ctl.setMic(true);
                                }
                            }
                        }

                        Label {
                            Layout.fillWidth: true
                            wrapMode: Text.Wrap
                            opacity: 0.7
                            text: ctl.systemMicMuted < 0
                                ? "PipeWire не найден: переключается только индикатор на клавише."
                                : "Глушит микрофон в системе и зажигает индикатор на клавише. Fn+F4 — то же с клавиатуры."
                        }
                        Item { Layout.fillHeight: true }
                    }

                    // --- Система ---
                    ColumnLayout {
                        spacing: 8

                        Label {
                            text: "Сведения о системе"
                            font.pixelSize: 16
                            font.bold: true
                        }
                        TextArea {
                            Layout.fillWidth: true
                            readOnly: true
                            selectByMouse: true
                            wrapMode: Text.Wrap
                            text: ctl.deviceInfo + (ctl.connected ? "\nБэкенд: " + ctl.backend : "")
                        }
                        Item { Layout.fillHeight: true }
                    }

                    // --- О программе ---
                    ScrollView {
                        clip: true

                        Label {
                            width: parent.width
                            wrapMode: Text.Wrap
                            textFormat: Text.MarkdownText
                            onLinkActivated: link => Qt.openUrlExternally(link)
                            text: "## MI Control для Linux\n\n"
                                + "Управление функциями ноутбуков Xiaomi и RedmiBook: режимы производительности, "
                                + "защита батареи, микрофон.\n\n"
                                + "Порт [MI Control Utility](https://github.com/loginsinex/micontrol) (© loginsin, 2025) "
                                + "на Rust и Qt. Значения лимитов заряда сверены с ACPI-таблицами RedmiBook 2025 "
                                + "(метод WMAA) и [Arch Wiki](https://wiki.archlinux.org/title/Xiaomi_RedmiBook_Pro_16_2025).\n\n"
                                + "**Внимание:** авторы не связаны с производителем и не несут ответственности "
                                + "за возможные неполадки. Используйте на свой страх и риск.\n\n"
                                + "Лицензия: GNU GPL v3 или новее."
                        }
                    }
                }
            }
        }
    }
}
