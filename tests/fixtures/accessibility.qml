import QtQuick
import QtQuick.Controls
import QtQuick.Window

Window {
    visible: true
    width: 320
    height: 240
    title: "accessibility fixture"

    Column {
        spacing: 8
        TextField {
            id: field
            Accessible.name: "field"
            placeholderText: "type here"
        }
        Button {
            text: "Press"
            onClicked: field.text = "pressed"
        }
    }
}
