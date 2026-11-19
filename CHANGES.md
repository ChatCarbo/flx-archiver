# FLX Archiver — correctifs

## Bugs bloquants
- **L'extraction ne démarrait jamais** : `decompress` est une fonction async, mais le futur était créé dans `spawn_blocking` puis jeté sans être exécuté. Il est maintenant attendu.
- **7z et RAR : « échec » même en cas de réussite** : ces formats n'envoyaient aucune progression, donc l'interface concluait à une erreur à la fermeture du canal. La fin de l'opération est maintenant détectée à la fermeture du canal, et un message 100 % est envoyé.
- **Les erreurs étaient invisibles** (`eprintln!` dans une application sans console) : elles sont maintenant transmises et affichées dans l'écran d'erreur.
- **Gzip : extraction « Format non supporté »** : implémentée (`.gz` et `.tar.gz`).
- **Zstd** : un `.tar.zst` était seulement décompressé en `.tar` ; il est maintenant extrait. Le contenu est reconnu par son en-tête `ustar`.

## Sécurité
- Zip Slip : RAR n'avait aucune validation, 7z ne refusait pas les lecteurs Windows (`C:`) ni les flux NTFS. Nouvelle fonction `security::safe_join` (avec tests) utilisée par 7z et RAR ; ZIP garde `enclosed_name` ; TAR est protégé par la crate `tar`. Les entrées refusées sont comptées et signalées.
- 7z : les erreurs d'écriture étaient ignorées silencieusement ; elles interrompent maintenant l'extraction avec un message.

## Données perdues / fausses
- **7z avec plusieurs fichiers sélectionnés archivait tout le dossier parent**, y compris les fichiers non choisis. Seuls les éléments sélectionnés sont maintenant archivés.
- ZIP : le niveau choisi est appliqué (il était ignoré) ; les fichiers de 4 Gio ou plus utilisent Zip64.
- **Historique faussé** : la taille d'origine était enregistrée comme taille compressée (ratio ≈ 100 % toujours), y compris après une extraction. On enregistre maintenant la vraie taille du fichier produit, et seulement pour une compression. Les anciennes mesures à ratio exactement 100 % sont ignorées.

## Recommandation
- L'analyseur classait toujours le contenu comme « texte » : il classe maintenant par extension (catégorie dominante ≥ 60 % des octets, sinon « mixte »).
- Le choix se base sur la moyenne par (format, niveau) et attend 3 mesures minimum ; la vitesse est plafonnée à 200 MB/s dans le score.
- L'historique est enregistré dans `%APPDATA%\FLX Archiver\` (et non plus dans le dossier courant). Les anciennes mesures ne sont pas reprises.

## Divers
- Format détecté par signature (ZIP, 7z, RAR, Zstd, Gzip), puis par extension.
- Fichiers orphelins non compilés supprimés (`security/zip_slip.rs` à la syntaxe cassée, `decompression/detect.rs`, `decompression/format_zstd.rs`, `compression/format_*.rs`, `utils.rs`).
- README réécrit.
