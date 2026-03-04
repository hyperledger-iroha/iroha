---
lang: fr
direction: ltr
source: docs/portal/docs/sorafs/chunker-registry-charter.ur.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
identifiant : chunker-registry-charter
titre : Charte du registre des chunkers SoraFS
sidebar_label : charte du registre Chunker
description : Soumissions de profils de chunkers et approbations et charte de gouvernance
---

:::note مستند ماخذ
:::

# SoraFS charte de gouvernance du registre des chunkers

> **Ratifié :** 2025-10-29 par le Panel sur les infrastructures du Parlement Sora (voir
> `docs/source/sorafs/council_minutes_2025-10-29.md`). La gouvernance et la gouvernance
> mise en œuvre de la charte normative et de la charte nationale

La charte SoraFS du registre des chunkers permet d'évoluer vers le processus et de définir les rôles.
یہ [Chunker Profile Authoring Guide](./chunker-profile-authoring.md) کی تکمیل کرتی ہے اور یہ بیان کرتی ہے کہ نئے
profils proposer, réviser, ratifier et déprécier déprécier

## Portée

یہ charte `sorafs_manifest::chunker_registry` کی ہر entrée پر لاگو ہے اور
Il s'agit d'un outil de registre et d'un registre (CLI manifeste, CLI d'annonce de fournisseur,
SDK). یہ alias اور gérer les invariants appliquer کرتی ہے جنہیں
`chunker_registry::ensure_charter_compliance()` est le cas :

- Les identifiants de profil sont des nombres entiers entiers et monotones.
- Poignée canonique `namespace.name@semver` **لازم** ہے کہ `profile_aliases` میں پہلی
- Les chaînes d'alias coupent les entrées uniques et les poignées canoniques entrent en collision avec les entrées.

## Rôles- **Auteur(s)** – proposition pour les luminaires régénérer les preuves du déterminisme et les preuves du déterminisme
- **Groupe de travail sur l'outillage (TWG)** – listes de contrôle publiées pour la proposition de validation des invariants du registre et des invariants du registre
- **Conseil de gouvernance (GC)** – Rapport du GTT et examen de l'enveloppe de proposition et signature et délais de publication/dépréciation approuver et approuver.
- **Équipe de stockage** – la mise en œuvre du registre maintient les mises à jour de la documentation et publie les mises à jour

## Workflow du cycle de vie

1. **Soumission de la proposition**
   - Guide de création de l'auteur et liste de contrôle de validation
     `docs/source/sorafs/proposals/` est un `ChunkerProfileProposalV1` JSON disponible en ligne
   - Sortie CLI ci-dessous :
     ```bash
     cargo run -p sorafs_manifest --bin sorafs_manifest_chunk_store -- --list-profiles
     cargo run -p sorafs_manifest --bin sorafs_manifest_chunk_store -- \
       --promote-profile=<handle> --json-out=-
     cargo run -p sorafs_manifest --bin sorafs_manifest_stub -- \
       --chunker-profile=<handle> --json-out=-
     ```
   - Calendrier, proposition, rapport de déterminisme, mises à jour du registre et soumission des relations publiques.

2. **Révision des outils (TWG)**
   - Liste de contrôle de validation pour les projets (luminaires, fuzz, pipeline manifeste/PoR).
   - `cargo test -p sorafs_car --chunker-registry` pour le téléphone portable
     `ensure_charter_compliance()` entrée entrée کے ساتھ پاس ہو۔
   - Comportement CLI (`--list-profiles`, `--promote-profile`, streaming
     `--json-out=-`) Les alias mis à jour et les poignées de compte sont mis à jour
   - Résultats et statut réussite/échec3. **Approbation du Conseil (GC)**
   - Rapport du TWG sur les métadonnées de la proposition et examen des détails
   - Résumé de la proposition (`blake3("sorafs-chunker-profile-v1" || bytes)`) sous le signe کریں اور
     signatures et enveloppe du conseil et calendriers et calendriers
   - Résultat du vote et procès-verbal de gouvernance

4. **Publication**
   - PR merge کریں، اور اپڈیٹ کریں :
     -`sorafs_manifest::chunker_registry_data`.
     - Documentation (`chunker_registry.md`, guides de création/conformité).
     - Rapports sur les luminaires et le déterminisme.
   - Opérateurs et équipes SDK, profil actuel et déploiement prévu pour les clients

5. **Dépréciation / Coucher du soleil**
   - Les propositions de profil et le remplacement des fenêtres de publication par une fenêtre de double publication
     (périodes de grâce) اور plan de mise à niveau شامل ہونا چاہیے۔
     Grand livre de migration et mise à jour

6. **Modifications d'urgence**
   - Suppression des correctifs et du vote du conseil et de l'approbation du conseil
   - Document sur les étapes d'atténuation des risques du TWG et mise à jour du journal des incidents

## Attentes en matière d'outillage- `sorafs_manifest_chunk_store` et `sorafs_manifest_stub` exposent les détails :
  - Inspection du registre کے لیے `--list-profiles`.
  - Le profil favorise le bloc de métadonnées canoniques par `--promote-profile=<handle>`.
  - Rapports sur la sortie standard et le flux de données pour `--json-out=-`, journaux d'examen reproductibles en ligne
- Fichiers binaires pertinents `ensure_charter_compliance()` pour le démarrage et le démarrage
  (`manifest_chunk_store`, `provider_advert_stub`). Les tests CI échouent ou échouent
  Charte des entrées نئی کی خلاف ورزی کریں۔

## Tenue de registres

- Rapports de déterminisme de type `docs/source/sorafs/reports/` en cours de recherche
- Chunker فیصلوں کا حوالہ دینے والی procès-verbal du conseil
  `docs/source/sorafs/migration_ledger.md` میں موجود ہیں۔
- Changement de registre pour `roadmap.md` et `status.md` pour `roadmap.md` et `status.md`.

## Références

- Guide de création : [Guide de création de profils Chunker](./chunker-profile-authoring.md)
- Liste de contrôle de conformité : `docs/source/sorafs/chunker_conformance.md`
- Référence du registre : [Chunker Profile Registry](./chunker-registry.md)