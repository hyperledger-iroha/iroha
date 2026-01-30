---
lang: he
direction: rtl
source: docs/portal/docs/sorafs/chunker-registry-charter.fr.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
---

---
id: chunker-registry-charter
title: Charte du registre chunker SoraFS
sidebar_label: Charte registre chunker
description: Charte de gouvernance pour les soumissions et approbations de profils chunker.
---

:::note Source canonique
Cette page reflète `docs/source/sorafs/chunker_registry_charter.md`. Gardez les deux copies synchronisées jusqu'à la retraite complète du set Sphinx hérité.
:::

# Charte de gouvernance du registre chunker SoraFS

> **Ratifiée :** 2025-10-29 par le Sora Parliament Infrastructure Panel (voir
> `docs/source/sorafs/council_minutes_2025-10-29.md`). Tout amendement exige un
> vote de gouvernance formel ; les équipes d'implémentation doivent traiter ce document comme
> normatif jusqu'à l'approbation d'une charte de remplacement.

Cette charte définit le processus et les rôles pour faire évoluer le registre chunker SoraFS.
Elle complète le [Guide de création des profils chunker](./chunker-profile-authoring.md) en décrivant comment de nouveaux
profils sont proposés, revus, ratifiés et finalement dépréciés.

## Portée

La charte s'applique à chaque entrée de `sorafs_manifest::chunker_registry` et
à tout tooling qui consomme le registre (manifest CLI, provider-advert CLI,
SDKs). Elle impose les invariants d'alias et de handle vérifiés par
`chunker_registry::ensure_charter_compliance()` :

- Les ID de profil sont des entiers positifs qui augmentent de façon monotone.
- Le handle canonique `namespace.name@semver` **doit** apparaître en première
- Les chaînes d'alias sont trimées, uniques et ne collisionnent pas avec les handles
  canoniques d'autres entrées.

## Rôles

- **Auteur(s)** – préparent la proposition, régénèrent les fixtures et collectent les
  preuves de déterminisme.
- **Tooling Working Group (TWG)** – valide la proposition à l'aide des checklists
  publiées et s'assure que les invariants du registre sont respectés.
- **Governance Council (GC)** – examine le rapport du TWG, signe l'enveloppe de la proposition
  et approuve les calendriers de publication/dépréciation.
- **Storage Team** – maintient l'implémentation du registre et publie
  les mises à jour de documentation.

## Flux du cycle de vie

1. **Soumission de la proposition**
   - L'auteur exécute la checklist de validation du guide d'auteur et crée
     un JSON `ChunkerProfileProposalV1` sous
     `docs/source/sorafs/proposals/`.
   - Inclure la sortie CLI de :
     ```bash
     cargo run -p sorafs_manifest --bin sorafs_manifest_chunk_store -- --list-profiles
     cargo run -p sorafs_manifest --bin sorafs_manifest_chunk_store -- \
       --promote-profile=<handle> --json-out=-
     cargo run -p sorafs_manifest --bin sorafs_manifest_stub -- \
       --chunker-profile=<handle> --json-out=-
     ```
   - Soumettre une PR contenant fixtures, proposition, rapport de déterminisme et
     mises à jour du registre.

2. **Revue tooling (TWG)**
   - Rejouer la checklist de validation (fixtures, fuzz, pipeline manifest/PoR).
   - Exécuter `cargo test -p sorafs_car --chunker-registry` et vérifier que
     `ensure_charter_compliance()` passe avec la nouvelle entrée.
   - Vérifier que le comportement du CLI (`--list-profiles`, `--promote-profile`, streaming
     `--json-out=-`) reflète les alias et handles mis à jour.
   - Produire un court rapport résumant les constats et le statut pass/fail.

3. **Approbation du conseil (GC)**
   - Examiner le rapport TWG et les métadonnées de la proposition.
   - Signer le digest de la proposition (`blake3("sorafs-chunker-profile-v1" || bytes)`)
     et ajouter les signatures à l'enveloppe du conseil maintenue avec les fixtures.
   - Consigner le résultat du vote dans les minutes de gouvernance.

4. **Publication**
   - Fusionner la PR en mettant à jour :
     - `sorafs_manifest::chunker_registry_data`.
     - Documentation (`chunker_registry.md`, guides d'auteur/conformité).
     - Fixtures et rapports de déterminisme.
   - Notifier les opérateurs et équipes SDK du nouveau profil et du rollout prévu.

5. **Dépréciation / Retrait**
   - Les propositions qui remplacent un profil existant doivent inclure une fenêtre de publication
     double (périodes de grâce) et un plan d'upgrade.
   - Après expiration de la fenêtre de grâce, marquer le profil remplacé comme déprécié
     dans le registre et mettre à jour le ledger de migration.

6. **Changements d'urgence**
   - Les suppressions ou hotfixes exigent un vote du conseil à la majorité.
   - Le TWG doit documenter les étapes de mitigation des risques et mettre à jour le journal d'incident.

## Attentes tooling

- `sorafs_manifest_chunk_store` et `sorafs_manifest_stub` exposent :
  - `--list-profiles` pour l'inspection du registre.
  - `--promote-profile=<handle>` pour générer le bloc de métadonnées canonique utilisé
    lors de la promotion d'un profil.
  - `--json-out=-` pour streamer les rapports vers stdout, permettant des logs de revue
    reproductibles.
- `ensure_charter_compliance()` est invoqué au démarrage dans les binaires concernés
  (`manifest_chunk_store`, `provider_advert_stub`). Les tests CI doivent échouer si
  de nouvelles entrées violent la charte.

## Registre

- Stocker tous les rapports de déterminisme dans `docs/source/sorafs/reports/`.
- Les minutes du conseil référant aux décisions chunker vivent sous
  `docs/source/sorafs/migration_ledger.md`.
- Mettre à jour `roadmap.md` et `status.md` après chaque changement majeur du registre.

## Références

- Guide de création : [Guide de création des profils chunker](./chunker-profile-authoring.md)
- Checklist de conformité : `docs/source/sorafs/chunker_conformance.md`
- Référence du registre : [Registre des profils chunker](./chunker-registry.md)
