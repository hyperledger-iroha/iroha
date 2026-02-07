---
lang: fr
direction: ltr
source: docs/portal/docs/sorafs/chunker-registry-charter.fr.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
identifiant : chunker-registry-charter
titre : Charte du registre chunker SoraFS
sidebar_label : Charte registre chunker
description : Charte de gouvernance pour les soumissions et approbations de profils chunker.
---

:::note Source canonique
Cette page reflète `docs/source/sorafs/chunker_registry_charter.md`. Gardez les deux copies synchronisées jusqu'à la retraite complète du set Sphinx subsister.
:::

# Charte de gouvernance du registre chunker SoraFS

> **Ratifiée :** 2025-10-29 par le Sora Parliament Infrastructure Panel (voir
> `docs/source/sorafs/council_minutes_2025-10-29.md`). Tout amendement exige un
> vote de gouvernance formelle ; les équipes d'implémentation doivent traiter ce document comme
> normatif jusqu'à l'approbation d'une charte de remplacement.

Cette charte définit le processus et les rôles pour faire évoluer le registre chunker SoraFS.
Elle complète le [Guide de création des profils chunker](./chunker-profile-authoring.md) en décrivant comment de nouveaux
les profils sont proposés, revus, ratifiés et finalement dépréciés.

## Portée

La charte s'applique à chaque entrée de `sorafs_manifest::chunker_registry` et
à tout outillage qui consomme le registre (manifest CLI, supplier-advert CLI,
SDK). Elle impose les invariants d'alias et de handle vérifiés par
`chunker_registry::ensure_charter_compliance()` :- Les ID de profil sont des entiers positifs qui augmentent de façon monotone.
- Le handle canonique `namespace.name@semver` **doit** apparaît en première
- Les chaînes d'alias sont trimées, uniques et ne collisionnent pas avec les handles
  canoniques d'autres entrées.

## Rôles

- **Auteur(s)** – préparent la proposition, régénèrent les luminaires et collectent les
  preuves de déterminisme.
- **Tooling Working Group (TWG)** – valider la proposition à l'aide des checklists
  publiées et s'assurent que les invariants du registre sont respectés.
- **Governance Council (GC)** – examiner le rapport du TWG, signer l'enveloppe de la proposition
  et approuver les calendriers de publication/dépréciation.
- **Storage Team** – maintenir l'implémentation du registre et publication
  les mises à jour de la documentation.

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
   - Soumettre une PR contenant des luminaires, proposition, rapport de déterminisme et
     mises à jour du registre.2. **Outils de revue (TWG)**
   - Rejouer la checklist de validation (fixtures, fuzz, pipeline manifest/PoR).
   - Exécuter `cargo test -p sorafs_car --chunker-registry` et vérifier que
     `ensure_charter_compliance()` passe avec la nouvelle entrée.
   - Vérifier que le comportement du CLI (`--list-profiles`, `--promote-profile`, streaming
     `--json-out=-`) reflète les alias et les handles mis à jour.
   - Produire un rapport judiciaire résumant les constats et le statut pass/fail.

3. **Approbation du conseil (GC)**
   - Examiner le rapport TWG et les métadonnées de la proposition.
   - Signer le résumé de la proposition (`blake3("sorafs-chunker-profile-v1" || bytes)`)
     et ajoutez les signatures à l'enveloppe du conseil enregistrée avec les luminaires.
   - Consigner le résultat du vote dans les minutes de gouvernance.

4. **Publication**
   - Fusionner la PR en mettant à jour :
     -`sorafs_manifest::chunker_registry_data`.
     - Documentation (`chunker_registry.md`, guides d'auteur/conformité).
     - Fixations et rapports de déterminisme.
   - Notifier les opérateurs et équipes SDK du nouveau profil et du déploiement prévu.

5. **Dépréciation / Retrait**
   - Les propositions qui remplacent un profil existant doivent inclure une fenêtre de publication
     double (périodes de grâce) et un plan d'upgrade.
   - Après expiration de la fenêtre de grâce, marquer le profil remplacé comme déprécié
     dans le registre et mettre à jour le grand livre de migration.6. **Changements d'urgence**
   - Les suppressions ou hotfixes nécessitent un vote du conseil à la majorité.
   - Le TWG doit documenter les étapes d'atténuation des risques et mettre à jour le journal d'incident.

## Attentes à l'outillage

- Exposant `sorafs_manifest_chunk_store` et `sorafs_manifest_stub` :
  - `--list-profiles` pour l'inspection du registre.
  - `--promote-profile=<handle>` pour générer le bloc de métadonnées canoniques utilisé
    lors de la promotion d'un profil.
  - `--json-out=-` pour streamer les rapports vers stdout, permettant des logs de revue
    reproductibles.
- `ensure_charter_compliance()` est étudié au démarrage dans les binaires concernés
  (`manifest_chunk_store`, `provider_advert_stub`). Les tests CI doivent échouer si
  de nouvelles entrées violentes la charte.

## Registre

- Stocker tous les rapports de déterminisme dans `docs/source/sorafs/reports/`.
- Les minutes du conseil référant aux décisions chunker vivent sous
  `docs/source/sorafs/migration_ledger.md`.
- Mettre à jour `roadmap.md` et `status.md` après chaque changement majeur du registre.

##Référence

- Guide de création : [Guide de création des profils chunker](./chunker-profile-authoring.md)
- Checklist de conformité : `docs/source/sorafs/chunker_conformance.md`
- Référence du registre : [Registre des profils chunker](./chunker-registry.md)