---
lang: he
direction: rtl
source: docs/portal/docs/sorafs/chunker-registry-charter.fr.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
מזהה: chunker-registry-charter
כותרת: Charte du registre chunker SoraFS
sidebar_label: צ'נקר רישום של Charte
תיאור: Charte de Governance pour les soumissions and approbations de profils chunker.
---

:::הערה מקור קנוניק
Cette page reflète `docs/source/sorafs/chunker_registry_charter.md`. Gardez les deux copies Syncées jusqu'à la retraite complète du set Sphinx hérité.
:::

# Charte de gouvernance du registre chunker SoraFS

> **אשרור:** 2025-10-29 par le Sora Parliament Infrastructure Panel (voir
> `docs/source/sorafs/council_minutes_2025-10-29.md`). Tout תיקון exige un
> vote de governance formel; les équipes d'implementation doivent traiter ce מסמך comme
> normatif jusqu'à l'approbation d'une charte de remplacement.

Cette charte définit le processus et les rôles pour faire évoluer le registre chunker SoraFS.
Elle complète le [Guide de création des profils chunker](./chunker-profile-authoring.md) en décrivant comment de nouveaux
פרופילים של הצעות, המלצות, אישורים וסופיות.

## Portée

La charte s'applique à chaque entrée de `sorafs_manifest::chunker_registry` et
הכלי המצורף לרישום (מניפסט CLI, ספק-מודעה CLI,
ערכות SDK). Elle להטיל les invariants d'alias et de handle vérifiés par
`chunker_registry::ensure_charter_compliance()` :

- Les ID de profil sont des entiers positifs qui augmentent de façon monotone.
- Le handle canonique `namespace.name@semver` **doit** apparaître en première
- Les chaînes d'alias sont trimées, uniques et ne collisionnent pas avec les handles
  canoniques d'autres entrées.

## תפקידים

- **מחבר(ים)** - קדם להצעת ההצעה, régénèrent les fixtures et collectent les
  preuves de déterminisme.
- **קבוצת עבודה בכלים (TWG)** - valide la proposition à l'aide des checklists
  publiées et s'assure que les invariants du registre sont respectés.
- **מועצת הממשל (GC)** - בחן את ה-rapport du TWG, signne l'enveloppe de la proposition
  et approuve les calendriers de publication/dépréciation.
- **צוות אחסון** - תחזוקה ליישום הרישום והפרסום
  les mises à jour de documentation.

## Flux du cycle de vie

1. **מסירה דה לה הצעה**
   - L'auteur exécute la checklist de validation du guide d'auteur et crée
     un JSON `ChunkerProfileProposalV1` סוס
     `docs/source/sorafs/proposals/`.
   - כלול את la sortie CLI de:
     ```bash
     cargo run -p sorafs_manifest --bin sorafs_manifest_chunk_store -- --list-profiles
     cargo run -p sorafs_manifest --bin sorafs_manifest_chunk_store -- \
       --promote-profile=<handle> --json-out=-
     cargo run -p sorafs_manifest --bin sorafs_manifest_stub -- \
       --chunker-profile=<handle> --json-out=-
     ```
   - Soumettre une תוצרי יחסי ציבור גופי, הצעה, rapport de déterminisme et
     mises à jour du registre.

2. **Revue Tooling (TWG)**
   - Rejouer la checklist de validation (תווים, fuzz, pipeline manifest/PoR).
   - Exécuter `cargo test -p sorafs_car --chunker-registry` et וérifier que
     `ensure_charter_compliance()` passe avec la nouvelle entrée.
   - Vérifier que le comportement du CLI (`--list-profiles`, `--promote-profile`, סטרימינג
     `--json-out=-`) reflète les alias et handles mis à jour.
   - Produire un court rapport résumant les constats et le statut עובר/נכשל.3. **Approvation du conseil (GC)**
   - Examiner le rapport TWG et les métadonnées de la proposition.
   - Signer le digest de la proposition (`blake3("sorafs-chunker-profile-v1" || bytes)`)
     et ajouter les signatures à l'enveloppe du conseil maintenue avec les fixtures.
   - Consigner le résultat du vote dans les minutes de gouvernance.

4. **פרסום**
   - Fusionner la PR en mettant à jour :
     - `sorafs_manifest::chunker_registry_data`.
     - תיעוד (`chunker_registry.md`, guides d'auteur/conformité).
     - Fixtures et rapports de déterminisme.
   - Notifier les opérateurs et équipes SDK du nouveau profil et du rollout prévu.

5. **דיפréciation / retrait**
   - ההצעות שחלפו ללא פרופיל קיימות כוללות פרסום אחד
     כפול (תקופות דה גראס) ו- un plan d'upgrade.
   - אפרה תפוגה de la fenêtre de grâce, marquer le profil remplacé comme déprécié
     dans le registre et mettre à jour le ledger de migration.

6. **שינויים דחוף**
   - Les suppressions ou תיקונים חמים דרושים un vote du conseil à la majorité.
   - Le TWG doit documenter les étapes de mitigation des risques et mettre à jour le journal d'incident.

## כלי תשומת לב

- `sorafs_manifest_chunk_store` et `sorafs_manifest_stub` חשיפה:
  - `--list-profiles` pour l'inspection du registre.
  - `--promote-profile=<handle>` pour générer le bloc de métadonnées canonique utilisé
    lors de la promotion d'un profil.
  - `--json-out=-` pour streamer les rapports vers stdout, permettant des logs de revue
    מוצרים לשחזור.
- `ensure_charter_compliance()` est invoqué au démarrage dans les binaires concernés
  (`manifest_chunk_store`, `provider_advert_stub`). Les tests CI doivent échouer si
  de nouvelles entrées violent la charte.

## הירשם

- Stocker tous les rapports de déterminisme dans `docs/source/sorafs/reports/`.
- Les minutes du conseil référant aux décisions chunker vivent sous
  `docs/source/sorafs/migration_ledger.md`.
- Mettre à jour `roadmap.md` et `status.md` לאחר השינוי בהרשמה.

## רפרנסים

- Guide de création : [Guide de création des profils chunker](./chunker-profile-authoring.md)
- רשימת תאימות: `docs/source/sorafs/chunker_conformance.md`
- Référence du registre : [Registre des profils chunker](./chunker-registry.md)