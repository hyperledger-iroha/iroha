---
lang: fr
direction: ltr
source: docs/portal/docs/sorafs/chunker-conformance.ur.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
identifiant : conformité du chunker
titre : Guide de conformité du chunker SoraFS
sidebar_label : conformité des fragments
description : Accessoires pour les SDK et le profil déterministe SF1 chunker et les exigences en matière de flux de travail
---

:::note مستند ماخذ
:::

Flux de travail de régénération, politique de signature et étapes de vérification du document, SDK et synchronisation des consommateurs d'appareils.

## Profil canonique

- Graine d'entrée (hex) : `0000000000dec0ded`
- Taille cible : 262 144 octets (256 Ko)
- Taille minimale : 65 536 octets (64 Ko)
- Taille maximale : 524 288 octets (512 Ko)
- Polynôme roulant : `0x3DA3358B4DC173`
- Graine de table d'engrenage : `sorafs-v1-gear`
- Masque de rupture : `0x0000FFFF`

Implémentation de référence : `sorafs_chunker::chunk_bytes_with_digests_profile`.
Il s'agit de l'accélération SIMD et des limites et des digestions de données.

## Lot de luminaires

Appareils `cargo run --locked -p sorafs_chunker --bin export_vectors` et régénération
Il s'agit d'un modèle `fixtures/sorafs_chunker/` pour votre compte :- `sf1_profile_v1.{json,rs,ts,go}` — Rust, TypeScript, et Go consumer pour les limites canoniques des morceaux ہر فائل
  آتے ہیں (مثلاً `sorafs.sf1@1.0.0`, پھر `sorafs.sf1@1.0.0`)۔ یہ commande
  `ensure_charter_compliance` کے ذریعے Ensure ہوتی ہے اور اسے تبدیل نہیں کیا جا سکتا۔
- `manifest_blake3.json` — Manifeste vérifié par BLAKE3 et support de luminaire et couverture de support
- `manifest_signatures.json` — résumé du manifeste پر signatures du conseil (Ed25519)۔
- `sf1_profile_v1_backpressure.json` et `fuzz/` pour les corpus bruts —
  scénarios de streaming déterministes et tests de contre-pression de chunker

### Politique de signature

Régénération des luminaires **لازم** طور پر signature valide du conseil شامل کرے۔ sortie non signée du générateur et rejet du générateur `--allow-unsigned` et rejet de celui-ci (صرف مقامی تجربات کے لیے)۔ Enveloppes de signature en annexe uniquement pour le signataire et pour la déduplication

Signature du Conseil شامل کرنے کے لیے :

```bash
cargo run --locked -p sorafs_chunker --bin export_vectors \
  --signing-key=<ed25519-private-key-hex> \
  --signature-out=fixtures/sorafs_chunker/manifest_signatures.json
```

## Vérification

CI helper Générateur `ci/check_sorafs_fixtures.sh` et `--locked` pour le générateur d'aide
Les matchs sont en dérive et les signatures sont manquantes ou le travail échoue. اس script کو
workflows nocturnes pour les changements de calendrier soumettre

Étapes de vérification manuelle :

1. `cargo test -p sorafs_chunker` چلائیں۔
2. `ci/check_sorafs_fixtures.sh` لوکل چلائیں۔
3. تصدیق کریں کہ `git status -- fixtures/sorafs_chunker` صاف ہے۔

## Mettre à niveau le manuel de jeu

Ce profil de chunker propose des liens vers SF1:یہ بھی دیکھیں: [`docs/source/sorafs/chunker_profile_authoring.md`](./chunker-profile-authoring.md) تاکہ
exigences en matière de métadonnées, modèles de proposition et listes de contrôle de validation

1. Paramètres des paramètres `ChunkProfileUpgradeProposalV1` (RFC SF-1) pour la version `ChunkProfileUpgradeProposalV1`
2. `export_vectors` les luminaires régénèrent le résumé du manifeste et le contenu du manifeste
3. مطلوبہ quorum du conseil کے ساتھ signe manifeste کریں۔ signatures
   `manifest_signatures.json` میں append ہونی چاہئیں۔
4. Les appareils du SDK (Rust/Go/TS) sont dotés d'une parité d'exécution croisée et d'une parité d'exécution croisée.
5. Paramètres de régénération des corps fuzz
6. Il s'agit d'une poignée de profil, de graines, et d'un résumé de la poignée de profil.
7. Les tests de test et les mises à jour de la feuille de route et les soumissions

Il s'agit d'une limite de bloc et d'un résumé d'un bloc non valide et d'une fusion non valide