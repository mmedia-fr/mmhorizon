// SPDX-License-Identifier: GPL-3.0-or-later
#include <QtCore/QCoreApplication>
#include <QtCore/QFileInfo>
#include <QtCore/QString>
#include <QtCore/QStringList>
#include <QtCore/QTimer>
#include <QtCore/QUrl>
#include <QtCore/QVariant>
#include <QtGui/QGuiApplication>
#include <QtGui/QIcon>
#include <QtGui/QImage>
#include <QtQml/QQmlApplicationEngine>
#include <QtQml/QQmlContext>
#include <QtQml/qqml.h>
#include <QtQuick/QQuickWindow>
#include <QtQuickControls2/QQuickStyle>

#include "fichiers.h"

#ifdef Q_OS_WIN
#  include <windows.h>  // GetCommandLineW / CommandLineToArgvW
#  include <shellapi.h>
#endif

#include <cstdio>

/// Arguments du programme, en Unicode et sans perte.
///
/// Sous Windows, argv arrive dans la page de code ANSI, qui remplace par « ? »
/// tout ce qu'elle ne sait pas représenter : on demande la ligne de commande
/// Unicode à Windows lui-même (défaut relevé sur MMdedit, même socle).
static QStringList argumentsUnicode()
{
#ifdef Q_OS_WIN
  int nombre = 0;
  LPWSTR* natifs = CommandLineToArgvW(GetCommandLineW(), &nombre);
  if (!natifs)
    return QCoreApplication::arguments();
  QStringList liste;
  liste.reserve(nombre);
  for (int i = 0; i < nombre; ++i)
    liste.append(QString::fromWCharArray(natifs[i]));
  LocalFree(natifs);
  return liste;
#else
  return QCoreApplication::arguments();
#endif
}

int main(int argc, char* argv[])
{
  QGuiApplication app(argc, argv);
  QCoreApplication::setApplicationName(QStringLiteral("MMhorizon"));
  QCoreApplication::setOrganizationName(QStringLiteral("M-Media"));
  QCoreApplication::setOrganizationDomain(QStringLiteral("mmedia.fr"));
  QCoreApplication::setApplicationVersion(QStringLiteral(MMHORIZON_VERSION));
#ifndef Q_OS_ANDROID
  QGuiApplication::setWindowIcon(QIcon(QStringLiteral(":/assets/mmhorizon.png")));
#endif

  // --smoke : contrôle de fabrication, sans écran (plateforme « offscreen »).
  bool smoke = false;
  // --capture FICHIER.png : rend la fenêtre dans un PNG puis sort — sert à juger
  // l'interface depuis une machine sans écran.
  QString capture;
  QString fichier;
  const QStringList arguments = argumentsUnicode();
  for (int i = 1; i < arguments.size(); ++i) {
    const QString argument = arguments.at(i);
    if (argument == QStringLiteral("--smoke"))
      smoke = true;
    else if (argument == QStringLiteral("--capture") && i + 1 < arguments.size())
      capture = arguments.at(++i);
    else if (fichier.isEmpty())
      fichier = argument;
  }

  qmlRegisterType<Fichiers>("fr.mmedia.mmhorizon.natif", 1, 0, "Fichiers");

  // Fusion : le seul style qui rende pareil sur les quatre cibles.
  QQuickStyle::setStyle(QStringLiteral("Fusion"));

  QQmlApplicationEngine engine;
  // Le moteur prend possession du fournisseur d'image.
  Fichiers::apercu = new Apercu;
  engine.addImageProvider(QStringLiteral("apercu"), Fichiers::apercu);

  QUrl fichierInitial;
  if (!fichier.isEmpty())
    fichierInitial = QUrl::fromLocalFile(QFileInfo(fichier).absoluteFilePath());
  engine.rootContext()->setContextProperty(QStringLiteral("fichierInitial"), fichierInitial);

  // Chargement par URL qrc plutôt que loadFromModule(), absent de Qt 6.4 (Debian 12).
  const QUrl url(QStringLiteral("qrc:/qt/qml/fr/mmedia/mmhorizon/qml/Main.qml"));
  QObject::connect(
    &engine, &QQmlApplicationEngine::objectCreated, &app,
    [url](QObject* obj, const QUrl& objUrl) {
      if (!obj && url == objUrl)
        QCoreApplication::exit(1);
    },
    Qt::QueuedConnection);
  engine.load(url);

  if (smoke) {
    QTimer::singleShot(0, &app, [&engine, fichierInitial, fichier] {
      if (engine.rootObjects().isEmpty()) {
        std::fprintf(stderr, "smoke: aucune fenetre creee\n");
        QCoreApplication::exit(1);
        return;
      }
      QObject* racine = engine.rootObjects().first();
      const QString noyau = racine->property("noyau").toString();
      QVariant resultat;
      const bool appel =
        QMetaObject::invokeMethod(racine, "controle", Q_RETURN_ARG(QVariant, resultat));
      std::printf("smoke: %s | %s\n", noyau.toUtf8().constData(),
                  resultat.toString().toUtf8().constData());
      std::fflush(stdout);
      if (!noyau.startsWith(QStringLiteral("mmhorizon_core"))) {
        QCoreApplication::exit(2);
        return;
      }
      if (!appel || !resultat.toString().startsWith(QStringLiteral("ok"))) {
        QCoreApplication::exit(3);
        return;
      }
      // Une photo donnée en argument doit être arrivée jusqu'au noyau : lecture
      // par Qt, décodage, détection.
      if (!fichierInitial.isEmpty() && !racine->property("photoChargee").toBool()) {
        std::fprintf(stderr, "smoke: la photo donnee en argument n'a pas ete chargee : %s\n",
                     fichier.toUtf8().constData());
        QCoreApplication::exit(4);
        return;
      }
      QCoreApplication::exit(0);
    });
  } else if (!capture.isEmpty()) {
    QTimer::singleShot(1500, &app, [&engine, capture] {
      auto* fenetre = engine.rootObjects().isEmpty()
                        ? nullptr
                        : qobject_cast<QQuickWindow*>(engine.rootObjects().first());
      const bool ok = fenetre && fenetre->grabWindow().save(capture);
      std::printf("capture: %s %s\n", capture.toUtf8().constData(), ok ? "ecrite" : "en echec");
      QCoreApplication::exit(ok ? 0 : 5);
    });
  }

  return app.exec();
}
