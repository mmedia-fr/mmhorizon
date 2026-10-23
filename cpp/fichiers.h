// SPDX-License-Identifier: GPL-3.0-or-later
// Accès aux fichiers et image affichée, côté Qt.
//
// La lecture et l'écriture passent par QFile et non par le noyau Rust : sous
// Android, une photo choisie dans la galerie arrive en URL « content:// », que
// seul Qt sait ouvrir. Le noyau reçoit et rend des octets.
#pragma once

#include <QtCore/QBuffer>
#include <QtCore/QByteArray>
#include <QtCore/QFile>
#include <QtCore/QMutex>
#include <QtCore/QMutexLocker>
#include <QtCore/QObject>
#include <QtCore/QString>
#include <QtCore/QUrl>
#include <QtGui/QImage>
#include <QtGui/QImageReader>
#include <QtQuick/QQuickImageProvider>

/// Panorama affiché, servi à QML sous « image://apercu/<n> ».
class Apercu : public QQuickImageProvider
{
public:
  Apercu() : QQuickImageProvider(QQuickImageProvider::Image) {}

  QImage requestImage(const QString&, QSize* taille, const QSize&) override
  {
    QMutexLocker verrou(&m_verrou);
    if (taille)
      *taille = m_image.size();
    return m_image;
  }

  void poser(const QImage& image)
  {
    QMutexLocker verrou(&m_verrou);
    m_image = image;
  }

private:
  QMutex m_verrou;
  QImage m_image;
};

class Fichiers : public QObject
{
  Q_OBJECT
public:
  using QObject::QObject;

  /// Fournisseur d'image partagé, posé par main.cpp avant le chargement du QML.
  static inline Apercu* apercu = nullptr;

  /// Largeur maximale de l'image affichée : une texture de 4 096 pixels passe
  /// sur tous les processeurs graphiques, mobiles compris, et donne encore
  /// 0,09° par pixel sur un tour complet.
  static constexpr int LargeurAffichage = 4096;

  Q_INVOKABLE QByteArray lire(const QUrl& url) const
  {
    QFile fichier(chemin(url));
    if (!fichier.open(QIODevice::ReadOnly))
      return {};
    return fichier.readAll();
  }

  Q_INVOKABLE bool ecrire(const QUrl& url, const QByteArray& octets) const
  {
    if (octets.isEmpty())
      return false;
    QFile fichier(chemin(url));
    if (!fichier.open(QIODevice::WriteOnly | QIODevice::Truncate))
      return false;
    return fichier.write(octets) == octets.size();
  }

  /// Nom de fichier lisible ; sous Android, le dernier segment d'une URL
  /// « content:// » est souvent encodé, voire un simple identifiant.
  Q_INVOKABLE QString nom(const QUrl& url) const
  {
    QString n = QUrl::fromPercentEncoding(url.fileName().toUtf8());
    const int deuxPoints = n.lastIndexOf(QLatin1Char(':'));
    if (deuxPoints >= 0)
      n = n.mid(deuxPoints + 1);
    const int barre = n.lastIndexOf(QLatin1Char('/'));
    if (barre >= 0)
      n = n.mid(barre + 1);
    return n.isEmpty() ? QStringLiteral("panorama") : n;
  }

  /// Décode la photo pour l'affichage, orientation EXIF appliquée comme dans
  /// le noyau, réduite à LargeurAffichage.
  Q_INVOKABLE bool poserApercu(const QByteArray& octets)
  {
    QBuffer tampon;
    tampon.setData(octets);
    tampon.open(QIODevice::ReadOnly);
    QImageReader lecteur(&tampon);
    lecteur.setAutoTransform(true);
    QImage image = lecteur.read();
    if (image.isNull() || !apercu)
      return false;
    if (image.width() > LargeurAffichage)
      image = image.scaledToWidth(LargeurAffichage, Qt::SmoothTransformation);
    apercu->poser(image);
    return true;
  }

private:
  static QString chemin(const QUrl& url)
  {
    return url.isLocalFile() ? url.toLocalFile() : url.toString();
  }
};
