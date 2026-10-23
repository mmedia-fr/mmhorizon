// SPDX-License-Identifier: GPL-3.0-or-later
// Fenêtre de MMhorizon : panorama, désignation du nord, horizon, retouche, export.
// Écrit pour Qt 6.4 (machine de développement Debian 12) comme pour Qt 6.8.
import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import QtQuick.Dialogs
import fr.mmedia.mmhorizon
import fr.mmedia.mmhorizon.natif

ApplicationWindow {
    id: fenetre
    width: 1200
    height: 720
    visible: true
    title: releve.charge ? "MMhorizon — " + nomPhoto : "MMhorizon"

    // « » : simple consultation ; « nord », « horizon », « retouche » : le prochain
    // geste sur le panorama sert à cela.
    property string mode: ""
    property string nomPhoto: ""
    property int numeroApercu: 0
    property var points: []
    property real zoom: 1.0
    property string direction: ""
    // Lus par le contrôle de fabrication (cpp/main.cpp).
    readonly property string noyau: socle.noyau
    readonly property bool photoChargee: releve.charge

    Socle { id: socle }
    Fichiers { id: fichiers }
    Releve {
        id: releve
        onRevisionChanged: {
            fenetre.points = JSON.parse(releve.limite())
            trace.requestPaint()
        }
    }

    function ouvrir(url) {
        const octets = fichiers.lire(url)
        if (octets.byteLength === 0) {
            alerte.montrer("Lecture impossible :\n" + url)
            return false
        }
        if (!releve.charger(octets, fichiers.nom(url))) {
            alerte.montrer(releve.erreur)
            return false
        }
        fichiers.poserApercu(octets)
        numeroApercu += 1
        nomPhoto = fichiers.nom(url)
        champNom.text = releve.nomSite()
        zoom = 1.0
        vue.contentX = 0
        mode = releve.nord < 0 ? "nord" : ""
        return true
    }

    // Contrôle de fabrication : chaîne complète du noyau sur un panorama de synthèse.
    function controle() {
        return releve.controle()
    }

    function enregistrer(genre, url) {
        const nom = champNom.text
        const octets = genre === "horizon" ? releve.fichierHorizon(nom) : releve.exporter(genre, nom)
        if (!fichiers.ecrire(url, octets))
            alerte.montrer("Écriture impossible :\n" + url)
    }

    function demanderEnregistrement(genre) {
        if (!releve.charge)
            return
        if (releve.nord < 0) {
            alerte.montrer("Désignez d'abord le nord sur le panorama : sans lui, les azimuts ne sont que relatifs.")
            mode = "nord"
            return
        }
        const base = champNom.text.length > 0 ? champNom.text : "horizon"
        const fichiersParGenre = {
            "horizon": [base + ".horizon.json", "Relevé d'horizon (*.horizon.json)"],
            "astromosaic": [base + "-astromosaic.json", "AstroMosaic (*.json)"],
            "nina": [base + ".hrz", "N.I.N.A. (*.hrz)"],
            "stellarium": [base + "-stellarium.zip", "Paysage Stellarium (*.zip)"]
        }
        dialogueEnregistrer.genre = genre
        dialogueEnregistrer.nameFilters = [fichiersParGenre[genre][1]]
        dialogueEnregistrer.selectedFile = fichiersParGenre[genre][0]
        dialogueEnregistrer.open()
    }

    header: ToolBar {
        RowLayout {
            anchors.fill: parent
            spacing: 4
            ToolButton { text: "Ouvrir…"; onClicked: dialogueOuvrir.open() }
            ToolSeparator {}
            ToolButton {
                text: "Nord"
                checkable: true
                enabled: releve.charge
                checked: fenetre.mode === "nord"
                onClicked: fenetre.mode = checked ? "nord" : ""
                ToolTip.visible: hovered
                ToolTip.text: "Touchez le point du panorama qui est au nord"
            }
            ToolButton {
                text: "Horizon"
                checkable: true
                enabled: releve.charge
                checked: fenetre.mode === "horizon"
                onClicked: fenetre.mode = checked ? "horizon" : ""
                ToolTip.visible: hovered
                ToolTip.text: "Touchez la ligne de l'image qui est à 0° (proposée : le milieu)"
            }
            ToolButton {
                text: "Retoucher"
                checkable: true
                enabled: releve.charge
                checked: fenetre.mode === "retouche"
                onClicked: fenetre.mode = checked ? "retouche" : ""
                ToolTip.visible: hovered
                ToolTip.text: "Faites glisser le long de la vraie limite pour la corriger"
            }
            ToolSeparator {}
            ToolButton { text: "−"; enabled: releve.charge; onClicked: fenetre.zoom = Math.max(1.0, fenetre.zoom / 1.5) }
            ToolButton { text: "+"; enabled: releve.charge; onClicked: fenetre.zoom = Math.min(12.0, fenetre.zoom * 1.5) }
            ToolSeparator {}
            TextField {
                id: champNom
                Layout.fillWidth: true
                Layout.minimumWidth: 120
                placeholderText: "Nom du site"
                enabled: releve.charge
            }
            ToolButton {
                text: "Enregistrer…"
                enabled: releve.charge
                onClicked: fenetre.demanderEnregistrement("horizon")
            }
            ToolButton {
                text: "Exporter ▾"
                enabled: releve.charge
                onClicked: menuExports.popup()
                Menu {
                    id: menuExports
                    MenuItem { text: "AstroMosaic (seestar_alp)…"; onTriggered: fenetre.demanderEnregistrement("astromosaic") }
                    MenuItem { text: "N.I.N.A. (.hrz)…"; onTriggered: fenetre.demanderEnregistrement("nina") }
                    MenuItem { text: "Stellarium (paysage .zip)…"; onTriggered: fenetre.demanderEnregistrement("stellarium") }
                }
            }
        }
    }

    Label {
        anchors.centerIn: parent
        visible: !releve.charge
        horizontalAlignment: Text.AlignHCenter
        text: "Ouvrez un panorama 360° du lieu d'observation.\n\n"
            + "Puis touchez le point qui est au nord : MMhorizon en déduit,\n"
            + "pour chaque degré d'azimut, la hauteur au-dessus de laquelle le ciel est dégagé."
    }

    Flickable {
        id: vue
        anchors.fill: parent
        visible: releve.charge
        clip: true
        contentWidth: plan.width
        contentHeight: plan.height
        interactive: fenetre.mode !== "retouche"
        boundsBehavior: Flickable.StopAtBounds
        ScrollBar.horizontal: ScrollBar { policy: ScrollBar.AlwaysOn }
        ScrollBar.vertical: ScrollBar {}

        Item {
            id: plan
            height: vue.height * fenetre.zoom
            width: releve.hauteur > 0 ? height * releve.largeur / releve.hauteur : 0
            readonly property real sx: releve.largeur > 0 ? width / releve.largeur : 1
            readonly property real sy: releve.hauteur > 0 ? height / releve.hauteur : 1

            Image {
                id: photo
                anchors.fill: parent
                source: fenetre.numeroApercu > 0 ? "image://apercu/" + fenetre.numeroApercu : ""
                cache: false
                smooth: true
                fillMode: Image.Stretch
            }

            Canvas {
                id: trace
                anchors.fill: parent
                renderTarget: Canvas.FramebufferObject
                onWidthChanged: requestPaint()
                onHeightChanged: requestPaint()
                onPaint: {
                    const ctx = getContext("2d")
                    ctx.reset()
                    if (!releve.charge)
                        return
                    const sx = plan.sx, sy = plan.sy
                    // Horizon (0°)
                    ctx.strokeStyle = "#00e0e0"
                    ctx.lineWidth = 1
                    ctx.setLineDash([6, 4])
                    ctx.beginPath()
                    ctx.moveTo(0, releve.horizon * sy)
                    ctx.lineTo(width, releve.horizon * sy)
                    ctx.stroke()
                    ctx.setLineDash([])
                    // Limite ciel / obstacles
                    ctx.strokeStyle = "#ff3030"
                    ctx.lineWidth = 2
                    ctx.beginPath()
                    for (let i = 0; i < fenetre.points.length; ++i) {
                        const p = fenetre.points[i]
                        if (i === 0) ctx.moveTo(p[0] * sx, p[1] * sy)
                        else ctx.lineTo(p[0] * sx, p[1] * sy)
                    }
                    ctx.stroke()
                    // Points cardinaux, à partir du nord
                    if (releve.nord >= 0 && releve.tour > 0) {
                        const noms = ["N", "E", "S", "O"]
                        ctx.font = "bold 16px sans-serif"
                        for (let k = 0; k < 4; ++k) {
                            let x = releve.nord + k * releve.tour / 4
                            while (x - releve.tour >= 0) x -= releve.tour
                            for (; x < releve.largeur; x += releve.tour) {
                                ctx.strokeStyle = k === 0 ? "#ffd800" : "rgba(255,216,0,0.45)"
                                ctx.lineWidth = k === 0 ? 2 : 1
                                ctx.beginPath()
                                ctx.moveTo(x * sx, 0)
                                ctx.lineTo(x * sx, height)
                                ctx.stroke()
                                ctx.fillStyle = "#ffd800"
                                ctx.fillText(noms[k], x * sx + 4, 18)
                            }
                        }
                    }
                }
            }

            MouseArea {
                id: geste
                anchors.fill: parent
                hoverEnabled: true
                preventStealing: fenetre.mode === "retouche"
                property real dernierX: -1
                onPositionChanged: (souris) => {
                    fenetre.direction = releve.direction(souris.x / plan.sx, souris.y / plan.sy)
                    if (fenetre.mode === "retouche" && pressed) {
                        releve.retoucher(dernierX / plan.sx, souris.x / plan.sx, souris.y / plan.sy)
                        dernierX = souris.x
                    }
                }
                onPressed: (souris) => {
                    dernierX = souris.x
                    if (fenetre.mode === "retouche")
                        releve.retoucher(souris.x / plan.sx, souris.x / plan.sx, souris.y / plan.sy)
                }
                onClicked: (souris) => {
                    if (fenetre.mode === "nord") {
                        releve.fixerNord(souris.x / plan.sx)
                        fenetre.mode = ""
                    } else if (fenetre.mode === "horizon") {
                        releve.fixerHorizon(souris.y / plan.sy)
                        fenetre.mode = ""
                    }
                }
                onWheel: (molette) => {
                    if (molette.modifiers & Qt.ControlModifier) {
                        fenetre.zoom = molette.angleDelta.y > 0 ? Math.min(12.0, fenetre.zoom * 1.25)
                                                                : Math.max(1.0, fenetre.zoom / 1.25)
                    } else {
                        vue.flick(molette.angleDelta.y * 8, 0)
                    }
                }
            }
        }

        // Pincement à deux doigts pour Android, comme dans MMdedit : deux points
        // de contact exigés, le défilement d'un doigt n'est pas gêné.
        PinchHandler {
            target: null
            // Facteur au début du geste : « activeScale » lui est relatif.
            property real zoomDepart: 1.0
            onActiveChanged: if (active) zoomDepart = fenetre.zoom
            onActiveScaleChanged: if (active) fenetre.zoom = Math.max(1.0, Math.min(12.0, zoomDepart * activeScale))
        }
    }

    footer: ToolBar {
        RowLayout {
            anchors.fill: parent
            anchors.leftMargin: 8
            anchors.rightMargin: 8
            Label {
                Layout.fillWidth: true
                elide: Text.ElideRight
                text: fenetre.mode === "nord" ? "Touchez le point du panorama qui est au nord."
                    : fenetre.mode === "horizon" ? "Touchez la ligne de l'image qui est à 0°."
                    : fenetre.mode === "retouche" ? "Faites glisser le long de la vraie limite ciel / obstacles."
                    : releve.resume
            }
            Label { text: fenetre.direction; visible: releve.charge }
        }
    }

    FileDialog {
        id: dialogueOuvrir
        title: "Ouvrir un panorama 360°"
        fileMode: FileDialog.OpenFile
        nameFilters: ["Panoramas (*.jpg *.jpeg *.png *.JPG *.JPEG *.PNG)", "Tous les fichiers (*)"]
        onAccepted: fenetre.ouvrir(selectedFile)
    }

    FileDialog {
        id: dialogueEnregistrer
        property string genre: "horizon"
        title: "Enregistrer"
        fileMode: FileDialog.SaveFile
        onAccepted: fenetre.enregistrer(genre, selectedFile)
    }

    Dialog {
        id: alerte
        property alias texte: corps.text
        function montrer(t) { texte = t; open() }
        anchors.centerIn: parent
        modal: true
        title: "MMhorizon"
        standardButtons: Dialog.Ok
        Label { id: corps; wrapMode: Text.Wrap }
    }

    Component.onCompleted: {
        if (fichierInitial.toString().length > 0)
            ouvrir(fichierInitial)
    }
}
