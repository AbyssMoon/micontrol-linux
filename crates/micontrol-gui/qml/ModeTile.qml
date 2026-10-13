import QtQuick
import QtQuick.Controls

// Плитка-переключатель: картинка, подпись и (необязательно) строка под ней.
AbstractButton {
    id: tile

    property string image: ""
    property string caption: ""
    property string subtitle: ""
    property color accent: "#ff6900"
    property int imageSize: 72

    implicitWidth: 128
    implicitHeight: content.implicitHeight + 24
    checkable: true
    hoverEnabled: true

    background: Rectangle {
        radius: 14
        color: tile.checked ? Qt.tint("#1f1f1f", Qt.alpha(tile.accent, 0.14))
                            : (tile.hovered ? "#2c2c2c" : "#1f1f1f")
        border.width: tile.checked ? 2 : (tile.visualFocus ? 2 : 0)
        border.color: tile.checked ? tile.accent : palette.highlight
        opacity: tile.enabled ? 1 : 0.4

        Behavior on color { ColorAnimation { duration: 150 } }
    }

    contentItem: Column {
        id: content
        spacing: 4
        topPadding: 12

        Image {
            anchors.horizontalCenter: parent.horizontalCenter
            source: tile.image
            sourceSize.width: tile.imageSize
            sourceSize.height: tile.imageSize
            fillMode: Image.PreserveAspectFit
            scale: tile.pressed ? 0.94 : 1
            Behavior on scale { NumberAnimation { duration: 100 } }
        }
        Label {
            anchors.horizontalCenter: parent.horizontalCenter
            text: tile.caption
            color: tile.checked ? tile.accent : "white"
            font.pixelSize: 15
            font.bold: true
            visible: text.length > 0
        }
        Label {
            anchors.horizontalCenter: parent.horizontalCenter
            text: tile.subtitle
            color: "#9a9a9a"
            font.pixelSize: 11
            visible: text.length > 0
        }
    }
}
