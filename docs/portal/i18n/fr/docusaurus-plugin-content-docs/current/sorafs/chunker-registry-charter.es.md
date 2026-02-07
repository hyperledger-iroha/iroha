---
lang: fr
direction: ltr
source: docs/portal/docs/sorafs/chunker-registry-charter.es.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
identifiant : chunker-registry-charter
titre : Carta del registro de chunker de SoraFS
sidebar_label : Carte du registre du chunker
description: Carta de gobernanza para presentaciones y aprobaciones de profils de chunker.
---

:::note Source canonique
Cette page reflète `docs/source/sorafs/chunker_registry_charter.md`. Assurez-vous d'avoir des copies synchronisées jusqu'à ce que le ensemble de documents Sphinx héréditaire soit retiré.
:::

# Charte de gouvernance du registre de chunker de SoraFS

> **Ratififié :** 2025-10-29 par le Panel sur les infrastructures du Parlement de Sora (version
> `docs/source/sorafs/council_minutes_2025-10-29.md`). Cualquier enmienda requiere un
> voto de gobernanza formel; les équipes de mise en œuvre doivent traiter ce document comme
> normativo hasta que se apruebe una carta que lo substitutya.

Cette carte définit le processus et les rôles pour l'évolution du registre de chunker de SoraFS.
Complète le [Guide de l'auteur des profils de chunker](./chunker-profile-authoring.md) pour décrire comment de nouveaux
les profils sont proposés, révisés, ratifiés et éventuellement dépréciés.

## Alcance

La carte appliquée à chaque entrée en `sorafs_manifest::chunker_registry` et
un outil quelconque qui utilise le registre (CLI de manifeste, CLI de fournisseur-annonce,
SDK). Impone les invariants d’alias et de poignée vérifiés par
`chunker_registry::ensure_charter_compliance()` :- Les identifiants de profil sont positifs et augmentent la forme monotone.
- La poignée canonique `namespace.name@semver` **debe** apparaît comme la première
  entrée en `profile_aliases`. Siguen los alias heredados.
- Les chaînes d'alias se cortan, son únicas y no colisionan con handles canonicos
  de autres entrées.

## Rôles

- **Auteur(s)** – préparer la proposition, régénérer les luminaires et recopier la
  preuve de déterminisme.
- **Tooling Working Group (TWG)** – valider la proposition en utilisant les listes de contrôle
  publiées et garantissant que les invariantes du registre soient planifiées.
- **Conseil de gouvernance (GC)** – révise le rapport du TWG, confirme le rapport de la proposition
  et aprueba les lieux de publication/dépréciation.
- **Storage Team** – assurer la mise en œuvre du registre et de la publication
  mises à jour de la documentation.

## Flux du cycle de vie

1. **Présentation de la proposition**
   - L'auteur exécute la liste de contrôle de validation du guide d'autorité et de création
     un JSON `ChunkerProfileProposalV1` fr
     `docs/source/sorafs/proposals/`.
   - Inclut la sortie du CLI de :
     ```bash
     cargo run -p sorafs_manifest --bin sorafs_manifest_chunk_store -- --list-profiles
     cargo run -p sorafs_manifest --bin sorafs_manifest_chunk_store -- \
       --promote-profile=<handle> --json-out=-
     cargo run -p sorafs_manifest --bin sorafs_manifest_stub -- \
       --chunker-profile=<handle> --json-out=-
     ```
   - Envoyez un PR qui contenga matches, propuesta, reporte de déterminismo y
     Actualisations du registre.2. **Révision de l'outillage (TWG)**
   - Reproduire la checklist de validation (fixtures, fuzz, pipeline de manifest/PoR).
   - Exécutez `cargo test -p sorafs_car --chunker-registry` et assurez-vous que
     `ensure_charter_compliance()` passe par la nouvelle entrée.
   - Vérifiez le comportement de la CLI (`--list-profiles`, `--promote-profile`, streaming
     `--json-out=-`) reflète les alias et les poignées actualisés.
   - Produire un rapport succinct sur les hallazgos et l'état d'approbation/rechazo.

3. **Agrément du conseiller (GC)**
   - Réviser le rapport du TWG et les métadonnées de la proposition.
   - Firma el digest de la propuesta (`blake3("sorafs-chunker-profile-v1" || bytes)`)
     et añade les firmas al sobre del consejo mantenido junto a los luminaires.
   - Enregistrer le résultat de la votation dans les actes de gouvernement.

4. **Publication**
   - Fusiona el PR, mise à jour :
     - `sorafs_manifest::chunker_registry_data`.
     - Documentation (`chunker_registry.md`, guías de autoría/conformidad).
     - Calendriers et rapports de déterminisme.
   - Notifier les opérateurs et les équipes SDK du nouveau profil et du déploiement planifié.

5. **Dépréciation / Retrait**
   - Les propositions qui remplacent un profil existant doivent inclure une fenêtre de publication
     double (périodes de grâce) et un plan d'actualisation.
     dans le registre et actualisez le grand livre de migration.6. **Changements d'urgence**
   - Les suppressions ou correctifs nécessitent un vote du conseiller avec l'approbation de la mairie.
   - Le TWG doit documenter les étapes d'atténuation des risques et actualiser le registre des incidents.

## Attentes de l'outillage

- Exposants `sorafs_manifest_chunk_store` et `sorafs_manifest_stub` :
  - `--list-profiles` pour l'inspection du registre.
  - `--promote-profile=<handle>` pour générer le bloc de métadonnées canonique utilisé
    al promouvoir un profil.
  - `--json-out=-` pour transmettre des rapports sur la sortie standard, permettant les journaux de révision
    reproductibles.
- `ensure_charter_compliance()` est appelé au démarrage dans les binaires pertinents
  (`manifest_chunk_store`, `provider_advert_stub`). Las pruebas CI deben fallar si
  de nouvelles entrées violent la charte.

## Registre

- Gardez tous les rapports de déterminisme en `docs/source/sorafs/reports/`.
- Les actes du conseil qui font référence aux décisions de chunker viven en
  `docs/source/sorafs/migration_ledger.md`.
- Actualisation `roadmap.md` et `status.md` après chaque changement de maire du registre.

## Références

- Guide de l'auteur : [Guide de l'auteur des profils de chunker](./chunker-profile-authoring.md)
- Liste de contrôle de conformité : `docs/source/sorafs/chunker_conformance.md`
- Référence du registre : [Registro de perfiles de chunker](./chunker-registry.md)