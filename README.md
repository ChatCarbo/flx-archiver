# FLX Archiver

Outil d'archivage Windows en Rust (`egui`), avec glisser-déposer, recommandation automatique du format et tableau de bord des performances. Licence MIT.

## Fonctionnalités

- **Compression** : ZIP, 7z, Zstd, Gzip. Niveaux Rapide / Équilibré / Maximum (ZIP : Deflate 1 / 6 / 9 ; Zstd : 1 / 3 / 19 ; Gzip : rapide / défaut / meilleur). Le niveau n'est pas encore pris en compte pour le 7z.
- **Extraction** : ZIP, 7z, RAR, Zstd, Gzip, y compris `.tar.zst` et `.tar.gz` (dossiers). Le format est reconnu par la signature du fichier, puis par l'extension.
- **Analyse intelligente** : le contenu glissé est classé (texte et code, médias déjà compressés, archives, binaires…) et l'outil propose le format et le niveau adaptés.
- **Apprentissage** : chaque compression enregistre taille, durée et vitesse ; une fois 3 mesures d'une même catégorie réunies, la recommandation s'appuie sur la moyenne de vos propres résultats.
- **Explorateur d'archives** et onglet **Statistics** (ratios, tailles, vitesses de vos compressions).
- **Sécurité** : les chemins contenus dans les archives (`../`, chemins absolus, lecteurs, flux NTFS) sont refusés à l'extraction (protection « Zip Slip »).

## Utilisation

1. Glissez des fichiers, des dossiers ou une archive dans la fenêtre.
2. Pour compresser : vérifiez le format et le niveau proposés, puis lancez.
3. Pour extraire : choisissez le dossier de destination.

L'historique des mesures est enregistré dans `%APPDATA%\FLX Archiver\compression_history.json`.

## Compilation

Prérequis : [Rust](https://www.rust-lang.org/) (édition 2021).

```bash
git clone https://github.com/ChatCarbo/flx-archiver.git
cd flx-archiver
cargo test
cargo run --release
```

## Pile technique

Rust, `eframe` / `egui` 0.28, `tokio`, `zip`, `sevenz-rust`, `unrar`, `zstd`, `flate2`, `tar`.

## Limites connues

- Windows uniquement.
- Le RAR se décompresse mais ne se crée pas.
- Une sélection multiple en 7z est d'abord copiée dans un dossier temporaire : prévoyez l'espace disque.
- Pas d'annulation d'une opération en cours.
