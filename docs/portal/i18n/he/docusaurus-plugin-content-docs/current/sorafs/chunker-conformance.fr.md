---
lang: he
direction: rtl
source: docs/portal/docs/sorafs/chunker-conformance.fr.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: chunker-conformance
כותרת: Guide de conformité du chunker SoraFS
sidebar_label: chunker Conformité
תיאור: דרישות וזרימות עבודה מקדימות את נתח הפרופיל SF1 מוגדר במכשירים ו-SDKs.
---

:::הערה מקור קנוניק
:::

Ce guide codifie les exigences que chaque implémentation doit suivre pour rester
alignée avec le profil chunker déterministe de SoraFS (SF1). Il documente aussi
זרימת העבודה של המשטר, לה פוליטיקה דה חתימות et les étapes de vérification pour que
les consommateurs de fixtures dans les SDKs restent Syncés.

## פרופיל canonique

- Seed d'entrée (hex): `0000000000dec0ded`
- כבל קצה: 262144 בייטים (256 KiB)
- מינימום אורך: 65536 בייטים (64 KiB)
- גודל מקסימלי: 524288 בייטים (512 KiB)
- Polynôme de rolling : `0x3DA3358B4DC173`
- Seed de table gear: `sorafs-v1-gear`
- מסכת קרע: `0x0000FFFF`

יישום עזר: `sorafs_chunker::chunk_bytes_with_digests_profile`.
להגדיר את ה- SIMD להגביל את ההגבלות ולעכל זהויות.

## חבילת מתקנים

`cargo run --locked -p sorafs_chunker --bin export_vectors` régénère les
אביזרי ואביזרים מתאימים לסוס `fixtures/sorafs_chunker/` :

- `sf1_profile_v1.{json,rs,ts,go}` - מגבלות של נתחים קנוניים
  הצרכנים Rust, TypeScript et Go. Chaque fichier annonce le handle canonique
  `sorafs.sf1@1.0.0`, puis `sorafs.sf1@1.0.0`). L'ordre est imposé par
  `ensure_charter_compliance` et NE DOIT PAS être modifié.
- `manifest_blake3.json` — מניפסט ודאי BLAKE3 couvrant chaque fichier de fixtures.
- `manifest_signatures.json` — חתימות du conseil (Ed25519) sur le digest du manifest.
- `sf1_profile_v1_backpressure.json` et corpora bruts dans `fuzz/` —
  תרחישים של זרימה דטרמיניסטים שימושיים לפי בדיקות של לחץ אחורי של צ'אנקר.

### פוליטיקה דה חתימה

La régénération des fixtures **doit** כוללים חתימה אחת valide du conseil. Le generateur
rejette la sortie non signnée sauf si `--allow-unsigned` est passé explicitement (prévu
ייחודיות pour l'expérimentation locale). Les enveloppes de signature sont append-only et
sont dédupliquées par signataire.

Pour ajouter une signature du conseil :

```bash
cargo run --locked -p sorafs_chunker --bin export_vectors \
  --signing-key=<ed25519-private-key-hex> \
  --signature-out=fixtures/sorafs_chunker/manifest_signatures.json
```

## אימות

Le helper CI `ci/check_sorafs_fixtures.sh` rejoue le générateur avec
`--locked`. התקנים שונים או החתימות הנפוצות, ה- Job échoue. Utilisez
ce script ברצפי עבודה דה nuit et avant de soumettre des changements de fixtures.

Étapes de vérification manuelle:

1. Exécutez `cargo test -p sorafs_chunker`.
2. מיקום Lancez `ci/check_sorafs_fixtures.sh`.
3. Confirmez que `git status -- fixtures/sorafs_chunker` est propre.

## Playbook de mise à niveau

Lorsqu'on propose un nouveau profil chunker או qu'on met à jour SF1:

Voir aussi : [`docs/source/sorafs/chunker_profile_authoring.md`](./chunker-profile-authoring.md) pour les
דרישות של שיטות, les templates de proposition ו-les checklists de validation.1. Rédigez un `ChunkProfileUpgradeProposalV1` (voir RFC SF-1) med de nouveaux paramètres.
2. Regénérez les fixtures via `export_vectors` et consignez le nouveau digest du manifest.
3. Signez le manifest avec le quorum requis. Toutes les signatures doivent être
   נספחים à `manifest_signatures.json`.
4. Mettez à jour les fixtures SDK concernées (Rust/Go/TS) and assurez la parité cross-runtime.
5. Régénéres les corpora fuzz si les paramètres changent.
6. Mettez à jour ce guide avec le nouveau handle de profil, les seeds et le digest.
7. Soumettez la modification avec des tests et des mises à jour du מפת הדרכים.

השינויים המשפיעים על הגבולות של הנתח או העיכול ללא כל התהליך
sont invalides et ne doivent pas être fusionnés.