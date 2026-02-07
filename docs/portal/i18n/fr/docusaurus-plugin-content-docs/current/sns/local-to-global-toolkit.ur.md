---
lang: fr
direction: ltr
source: docs/portal/docs/sns/local-to-global-toolkit.ur.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# Local -> Global ایڈریس ٹول کٹ

یہ صفحہ `docs/source/sns/local_to_global_toolkit.md` کا عکاس ہے۔ Feuille de route pour **ADDR-5c** pour les assistants CLI et les runbooks pour les utilisateurs

## جائزہ

- `scripts/address_local_toolkit.sh` `iroha` CLI et envelopper les éléments suivants :
  - `audit.json` -- `iroha tools address audit --format json` pour sortie structurée
  - `normalized.txt` -- ہر Sélecteur de domaine local کے لیے IH58 (ترجیحی) / compressé (`sora`, deuxième meilleur) littéraux۔
- Tableau de bord d'acquisition d'adresses (`dashboards/grafana/address_ingest.json`)
  Dans les règles d'Alertmanager (`dashboards/alerts/address_ingest_rules.yml`) pour les règles d'Alertmanager
  Basculement Local-8 / Local-12 Local-8 et Local-12 panneaux de collision ici
  Alertes `AddressLocal8Resurgence`, `AddressLocal12Collision`, et `AddressInvalidRatioSlo`
  کو manifeste تبدیلیاں promouvoir کرنے سے پہلے دیکھیں۔
- UX et réponse aux incidents par [Directives pour l'affichage des adresses] (address-display-guidelines.md)
  [runbook du manifeste d'adresse] (../../../source/runbooks/address_manifest_ops.md)

## استعمال

```bash
scripts/address_local_toolkit.sh       --input fixtures/address/local_digest_examples.txt       --output-dir artifacts/address_migration       --network-prefix 753       --format ih58
```

اختیارات:

- `--format compressed` IH58 pour sortie `sora...` pour sortie
- `--no-append-domain` تاکہ littéraux nus نکلیں۔
- Étape de conversion `--audit-only`
- `--allow-errors` lignes mal formées lors de l'analyse des lignes (comportement CLI ici)

اسکرپٹ رن کے آخر میں chemins d'artefacts لکھتا ہے۔ دونوں فائلیں
ticket de gestion du changement
>=30 pour les détections Local-8 et les collisions Local-12 pour

## CI انضمام1. Un travail dédié et un travail dédié aux sorties et aux sorties
2. Le `audit.json` Sélecteurs locaux رپورٹ کرے (`domain.kind = local12`) et fusionne les sélecteurs locaux
   default `true` par défaut (pour les régressions de développement/test par défaut et `false` par défaut)
   `iroha tools address normalize --fail-on-warning --only-local` et CI میں شامل کریں تاکہ
   production de régressions تک پہنچنے سے پہلے فیل ہوں۔

Vous trouverez ci-dessous des listes de contrôle des preuves et un extrait de note de version plus tard.
جسے آپ cutover کا اعلان کرتے وقت دوبارہ استعمال کر سکتے ہیں۔