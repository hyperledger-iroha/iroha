<!-- Auto-generated stub for French (fr) translation. Replace this content with the full translation. -->

---
lang: fr
direction: ltr
source: docs/source/offline_qr_operator_runbook.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 3628c64f36c9c27ab74fab02742a0d15fd90277feb9b04ea39be374de1399ae2
source_last_modified: "2026-02-15T17:55:05.344220+00:00"
translation_last_reviewed: 2026-04-02
translator: machine-google-reviewed
---

## Runbook de l'opérateur QR hors ligne

Ce runbook définit des préréglages pratiques `ecc`/dimension/fps pour les caméras bruyantes.
environnements lors de l’utilisation du transport QR hors ligne.

### Préréglages recommandés

| Environnement | Style | CCE | Dimensions | FPS | Taille des morceaux | Groupe paritaire | Remarques |
| --- | --- | --- | --- | --- | --- | --- | --- |
| Éclairage contrôlé, courte portée | `sakura` | `M` | `360` | `12` | `360` | `0` | Débit le plus élevé, redondance minimale. |
| Bruit typique d'une caméra mobile | `sakura-storm` | `Q` | `512` | `12` | `336` | `4` | Préréglage équilibré préféré (`~3 KB/s`) pour les appareils mixtes. |
| Éblouissement élevé, flou de mouvement, caméras bas de gamme | `sakura-storm` | `H` | `640` | `8` | `280` | `6` | Débit inférieur, résilience de décodage la plus forte. |

### Liste de contrôle d'encodage/décodage

1. Encodez avec des boutons de transport explicites.
2. Validez avec la capture en boucle de scanner avant le déploiement.
3. Épinglez le même profil de style dans les assistants de lecture du SDK pour conserver la parité de l'aperçu.

Exemple :

```bash
iroha offline qr encode \
  --style sakura-storm \
  --ecc Q \
  --dimension 512 \
  --fps 12 \
  --chunk-size 336 \
  --parity-group 4 \
  --in payload.bin \
  --out out_dir
```

### Validation de la boucle du scanner (profil sakura-storm 3 Ko/s)

Utilisez le même profil de transport sur tous les chemins de capture :

-`chunk_size=336`
-`parity_group=4`
-`fps=12`
-`style=sakura-storm`

Cibles de validation :- iOS : `OfflineQrStreamCameraSession` + `OfflineQrStreamScanSession`
-Androïde : `OfflineQrStreamCameraXScanner` + `OfflineQrStream.ScanSession`
- Navigateur/JS : `scanQrStreamFrames(...)` + `OfflineQrStreamScanSession`

Acceptation :

- La reconstruction complète de la charge utile réussit avec une trame de données supprimée par groupe de parité.
- Aucune incompatibilité de somme de contrôle/de hachage de charge utile dans la boucle de capture normale.