---
lang: fr
direction: ltr
source: docs/portal/docs/sorafs/chunker-profile-authoring.ur.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
identifiant : création de profil chunker
titre : Guide de création de profils de chunker SoraFS
sidebar_label : guide de création de Chunker
description : Profils de chunker SoraFS et luminaires pour la liste de contrôle
---

:::note مستند ماخذ
:::

# SoraFS Guide de création de profil de chunker

Les profils de chunker sont publiés sous SoraFS et sont publiés sous forme de profils chunker.
Architecture RFC (SF-1) et référence de registre (SF-2a)
exigences de création concrètes, étapes de validation et modèles de proposition
Canonical مثال کے لیے دیکھیں
`docs/source/sorafs/proposals/sorafs_sf1_profile_v1.json`
اور متعلقہ journal d'exécution à sec
`docs/source/sorafs/reports/sf1_determinism.md` Français

## Aperçu

Votre profil et votre registre sont disponibles pour votre compte :

- Paramètres CDC déterministes et paramètres multihash et architectures pour les applications et les publicités
- appareils rejouables (Rust/Go/TS JSON + fuzz corpora + témoins PoR) pour les SDK en aval et les outils sur mesure pour vérifier et vérifier
- examen du conseil par suite de diff déterministe پاس کرنا۔

Voici une liste de contrôle pour votre proposition de proposition.

## Aperçu de la charte du registre

Avant-projet de charte du registre کے مطابق ہے جسے
`sorafs_manifest::chunker_registry::ensure_charter_compliance()` applique کرتا ہے :- Les identifiants de profil sont des nombres entiers et des espaces monotones ou monotones.
- alias کسی دوسرے poignée canonique سے collision نہیں کر سکتا اور ایک سے زیادہ بار ظاہر نہیں ہو سکتا۔
- Alias ​​non vides et espaces et garnitures

Aides CLI pratiques :

```bash
# تمام registered descriptors کی JSON listing (ids, handles, aliases, multihash)
cargo run -p sorafs_manifest --bin sorafs_manifest_chunk_store -- --list-profiles

# Candidate default profile کے لیے metadata emit کریں (canonical handle + aliases)
cargo run -p sorafs_manifest --bin sorafs_manifest_chunk_store -- \
  --promote-profile=sorafs.sf1@1.0.0 --json-out=-
```

یہ propositions de commandes کو charte de registre کے مطابق رکھتے ہیں اور discussions sur la gouvernance کے لیے درکار métadonnées canoniques فراہم کرتے ہیں۔

## Métadonnées obligatoires| Champ | Descriptif | Exemple (`sorafs.sf1@1.0.0`) |
|-------|-------------|------------------------------|
| `namespace` | متعلقہ profils کے لیے regroupement logique۔ | `sorafs` |
| `name` | étiquette lisible par l'homme | `sf1` |
| `semver` | jeu de paramètres کے لیے chaîne de version sémantique۔ | `1.0.0` |
| `profile_id` | Identificateur numérique monotone جو profil کے terrain ہونے پر attribuer ہوتا ہے۔ اگلا id reserve کریں مگر موجودہ نمبرز réutilisation نہ کریں۔ | `1` |
| `profile.min_size` | longueur du bloc minimum en octets | `65536` |
| `profile.target_size` | longueur du morceau کی cible حد octets میں۔ | `262144` |
| `profile.max_size` | longueur du morceau کی maximum حد octets میں۔ | `524288` |
| `profile.break_mask` | hachage roulant کے لیے masque adaptatif (hex)۔ | `0x0000ffff` |
| `profile.polynomial` | constante polynomiale d'engrenage (hex)۔ | `0x3da3358b4dc173` |
| `gear_seed` | La table d'engrenages de 64 KiB dérive des graines | `sorafs-v1-gear` |
| `chunk_multihash.code` | résumés par morceau ou code multihash | `0x1f` (BLAKE3-256) |
| `chunk_multihash.digest` | bundle de luminaires canoniques کا digest۔ | `13fa...c482` |
| `fixtures_root` | luminaires régénérés رکھنے والی répertoire relatif۔ | `fixtures/sorafs_chunker/sorafs.sf1@1.0.0/` |
| `por_seed` | échantillonnage PoR déterministe et graine (`splitmix64`) | `0xfeedbeefcafebabe` (exemple) |Métadonnées et document de proposition et luminaires générés pour créer un registre et un registre, outils CLI et automatisation de la gouvernance, références croisées manuelles et confirmation des valeurs. Il s'agit d'un magasin de blocs et de CLI de manifeste et d'un `--json-out=-` qui contient des notes d'examen des métadonnées calculées et un flux de données.

### CLI et points de contact du registre

- `sorafs_manifest_chunk_store --profile=<handle>` — paramètres proposés pour les métadonnées de fragments, le résumé du manifeste et les vérifications PoR et les métadonnées
- `sorafs_manifest_chunk_store --json-out=-` — rapport de magasin de blocs et sortie standard et flux de données pour comparaisons automatisées et autres
- `sorafs_manifest_stub --chunker-profile=<handle>` — confirmer les manifestes et les plans CAR, gérer canoniquement et les alias intégrés
- `sorafs_manifest_stub --plan=-` — `chunk_fetch_specs` et alimentation pour changer et vérifier les compensations/digests

Résultat de la commande (résumés, racines PoR, hachages du manifeste) et la proposition est reproduite textuellement par les réviseurs.

## Liste de contrôle du déterminisme et de la validation1. **Les luminaires se régénèrent کریں**
   ```bash
   cargo run --locked -p sorafs_chunker --bin export_vectors \
     --signature-out=fixtures/sorafs_chunker/manifest_signatures.json
   ```
2. ** Suite de parité ** — `cargo test -p sorafs_chunker` et faisceau de différences multilingues (`crates/sorafs_chunker/tests/vectors.rs`) pour les luminaires vert et vert
3. **Relecture de corpus fuzz/contre-pression** — `cargo fuzz list` pour le harnais de streaming (`fuzz/sorafs_chunker`) et les actifs régénérés pour les actifs régénérés
4. **Les témoins de preuve de récupérabilité vérifient ** — Profil proposé `sorafs_manifest_chunk_store --por-sample=<n>` pour les racines et le manifeste du luminaire et la correspondance avec les fichiers.
5. **Exécution à sec CI** — `ci/check_sorafs_fixtures.sh` لوکل چلائیں؛ script کو نئے luminaires اور موجودہ `manifest_signatures.json` کے ساتھ réussir ہونا چاہیے۔
6. **Confirmation inter-exécution** — Les liaisons Go/TS régénérées JSON consomment des limites de fragments identiques et les résumés émettent des fragments

Commandes et résumés résultants et proposition pour le projet Tooling WG pour les conjectures.

### Confirmation du manifeste/PoR

Les appareils régénèrent les métadonnées CAR et les preuves PoR cohérentes :

```bash
# نئے profile کے ساتھ chunk metadata + PoR validate کریں
cargo run -p sorafs_manifest --bin sorafs_manifest_chunk_store -- \
  --profile=sorafs.sf2@1.0.0 \
  --json-out=- --por-json-out=- fixtures/sorafs_chunker/input.bin

# manifest + CAR generate کریں اور chunk fetch specs capture کریں
cargo run -p sorafs_manifest --bin sorafs_manifest_stub -- \
  fixtures/sorafs_chunker/input.bin \
  --chunker-profile=sorafs.sf2@1.0.0 \
  --chunk-fetch-plan-out=chunk_plan.json \
  --manifest-out=sf2.manifest \
  --car-out=sf2.car \
  --json-out=sf2.report.json

# محفوظ fetch plan کے ساتھ دوبارہ چلائیں (stale offsets سے بچاتا ہے)
cargo run -p sorafs_manifest --bin sorafs_manifest_stub -- \
  fixtures/sorafs_chunker/input.bin \
  --chunker-profile=sorafs.sf2@1.0.0 \
  --plan=chunk_plan.json --json-out=-
```

Fichier d'entrée pour les appareils et pour le corpus représentatif
(avec flux déterministe de 1 Gio) et les résumés résultants et la proposition en pièce jointe

## Modèle de proposition

Propositions pour `ChunkerProfileProposalV1` Norito enregistrements pour `docs/source/sorafs/proposals/` pour l'enregistrement pour les enregistrements Un modèle JSON attendu est disponible ici (les valeurs de remplacement sont remplacées par ):Rapport Markdown correspondant (`determinism_report`) qui comprend la sortie de la commande, les résumés de fragments et la validation et les écarts.

## Workflow de gouvernance

1. **Proposition + calendriers pour la soumission des relations publiques ** Actifs générés, proposition Norito et mises à jour `chunker_registry_data.rs` pour les mises à jour
2. **Révision du groupe de travail sur les outils**** Liste de contrôle de validation des évaluateurs pour confirmer les règles du registre des propositions (réutilisation des identifiants et déterminisme satisfait)
3. **Enveloppe du Conseil** Approuver le résumé des propositions des membres du conseil (`blake3("sorafs-chunker-profile-v1" || canonical_bytes)`) et signer les signatures de l'enveloppe de profil pour ajouter les éléments et les calendriers. ساتھ رکھا جاتا ہے۔
4. **Publication du registre** Fusionner le registre, la documentation et la mise à jour des appareils ہوتے ہیں۔ Profil CLI par défaut pour la migration de la gouvernance et prêt pour l'instant

## Conseils de création

- Puissance de deux avec limites paires et comportement de regroupement des cas extrêmes
- Graines de table d'engrenages et pistes d'audit lisibles par l'homme et uniques au monde
- Artefacts d'analyse comparative (comparaisons de débit par exemple) avec `docs/source/sorafs/reports/` pour référence et référence

Déploiement et attentes opérationnelles et grand livre de migration
(`docs/source/sorafs/migration_ledger.md`)۔ Règles de conformité d'exécution
`docs/source/sorafs/chunker_conformance.md` دیکھیں۔