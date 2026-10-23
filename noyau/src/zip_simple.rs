// SPDX-License-Identifier: GPL-3.0-or-later
//! Archive ZIP sans compression, juste assez pour un paysage Stellarium.

fn crc32(octets: &[u8]) -> u32 {
    let mut table = [0u32; 256];
    for (i, t) in table.iter_mut().enumerate() {
        let mut c = i as u32;
        for _ in 0..8 {
            c = if c & 1 != 0 { 0xEDB8_8320 ^ (c >> 1) } else { c >> 1 };
        }
        *t = c;
    }
    let mut crc = 0xFFFF_FFFFu32;
    for &o in octets {
        crc = table[((crc ^ o as u32) & 0xFF) as usize] ^ (crc >> 8);
    }
    !crc
}

/// Archive des fichiers (nom, contenu), stockés tels quels, noms en UTF-8.
pub fn archive(fichiers: &[(String, Vec<u8>)]) -> Vec<u8> {
    let mut sortie = Vec::new();
    let mut central = Vec::new();
    // 1er janvier 1980, minuit : date neutre, l'archive est reproductible.
    let (heure, jour) = (0u16, 0x0021u16);
    for (nom, contenu) in fichiers {
        let crc = crc32(contenu);
        let taille = contenu.len() as u32;
        let decalage = sortie.len() as u32;
        let n = nom.as_bytes();
        let entete = |s: &mut Vec<u8>, signature: u32, central: bool| {
            s.extend_from_slice(&signature.to_le_bytes());
            if central {
                s.extend_from_slice(&20u16.to_le_bytes());
            }
            s.extend_from_slice(&20u16.to_le_bytes());
            s.extend_from_slice(&0x0800u16.to_le_bytes()); // noms en UTF-8
            s.extend_from_slice(&0u16.to_le_bytes()); // stocké
            s.extend_from_slice(&heure.to_le_bytes());
            s.extend_from_slice(&jour.to_le_bytes());
            s.extend_from_slice(&crc.to_le_bytes());
            s.extend_from_slice(&taille.to_le_bytes());
            s.extend_from_slice(&taille.to_le_bytes());
            s.extend_from_slice(&(n.len() as u16).to_le_bytes());
            s.extend_from_slice(&0u16.to_le_bytes());
        };
        entete(&mut sortie, 0x0403_4b50, false);
        sortie.extend_from_slice(n);
        sortie.extend_from_slice(contenu);
        entete(&mut central, 0x0201_4b50, true);
        central.extend_from_slice(&0u16.to_le_bytes()); // commentaire
        central.extend_from_slice(&0u16.to_le_bytes()); // disque
        central.extend_from_slice(&0u16.to_le_bytes()); // attributs internes
        central.extend_from_slice(&0u32.to_le_bytes()); // attributs externes
        central.extend_from_slice(&decalage.to_le_bytes());
        central.extend_from_slice(n);
    }
    let debut_central = sortie.len() as u32;
    let taille_central = central.len() as u32;
    sortie.extend_from_slice(&central);
    sortie.extend_from_slice(&0x0605_4b50u32.to_le_bytes());
    sortie.extend_from_slice(&0u16.to_le_bytes());
    sortie.extend_from_slice(&0u16.to_le_bytes());
    sortie.extend_from_slice(&(fichiers.len() as u16).to_le_bytes());
    sortie.extend_from_slice(&(fichiers.len() as u16).to_le_bytes());
    sortie.extend_from_slice(&taille_central.to_le_bytes());
    sortie.extend_from_slice(&debut_central.to_le_bytes());
    sortie.extend_from_slice(&0u16.to_le_bytes());
    sortie
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn crc_de_reference() {
        assert_eq!(crc32(b"123456789"), 0xCBF4_3926);
    }

    #[test]
    fn structure_de_l_archive() {
        let a = archive(&[("a/b.txt".into(), b"bonjour".to_vec())]);
        assert_eq!(&a[..4], &[0x50, 0x4b, 0x03, 0x04]);
        let fin = a.len() - 22;
        assert_eq!(&a[fin..fin + 4], &[0x50, 0x4b, 0x05, 0x06]);
        assert!(a.windows(7).any(|w| w == b"bonjour"));
    }
}
