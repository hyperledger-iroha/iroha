---
lang: fr
direction: ltr
source: docs/portal/docs/sns/local-to-global-toolkit.ru.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# Paramètres des instruments Local -> Adresses globales

Cette page concerne `docs/source/sns/local_to_global_toolkit.md` en mono-repo. Lorsque vous activez les assistants CLI et les runbooks, vous pouvez consulter les cartes **ADDR-5c**.

## Обзор

- `scripts/address_local_toolkit.sh` utilise la CLI `iroha`, à savoir :
  - `audit.json` -- structuré pour `iroha tools address audit --format json`.
  - `normalized.txt` -- преобразованные IH58 (предпочтительно) / compressé (`sora`) (второй выбор) для каждого Sélecteur de domaine local.
- Utiliser le script dans les adresses d'acquisition du tableau de bord (`dashboards/grafana/address_ingest.json`)
  et j'utilise Alertmanager (`dashboards/alerts/address_ingest_rules.yml`), qui permet de télécharger le basculement Local-8 /
  Locale-12. Placez-vous dans les panneaux de collision Local-8 et Local-12 et alertes
  `AddressLocal8Resurgence`, `AddressLocal12Collision` et `AddressInvalidRatioSlo` avant
  продвижением изменений manifeste.
- Сверяйтесь с [Directives d'affichage des adresses](address-display-guidelines.md) et
  [runbook du manifeste d'adresse] (../../../source/runbooks/address_manifest_ops.md) pour l'UX et le contexte de réponse aux incidents.

## Utilisation

```bash
scripts/address_local_toolkit.sh       --input fixtures/address/local_digest_examples.txt       --output-dir artifacts/address_migration       --network-prefix 753       --format ih58
```

Commentaires :

- `--format compressed (`sora`)` pour votre `sora...` pour IH58.
- `--no-append-domain` pour les littéraux nus.
- `--audit-only` pour permettre la conversion.
- `--allow-errors` permet d'effectuer une analyse à l'aide de la CLI.Le script vous permet d'obtenir des œuvres d'art dans votre entreprise. Приложите оба файла к
capture d'écran du ticket de gestion des modifications avec Grafana, mis à jour maintenant
Local-8 детекций и ноль Local-12 коллизий miniмум за >=30 дней.

## Intégration CI

1. Enregistrez le script dans le travail précédent et enregistrez les sorties.
2. Bloquez les fusions en utilisant `audit.json` avec les sélecteurs locaux (`domain.kind = local12`).
   pour mettre à jour `true` (mettre en place `false` uniquement dans le développement/test lors de la régression du diagnostic) et
   Ajoutez `iroha tools address normalize --fail-on-warning --only-local` à CI, pour une régression rapide
   падали до production.

См. il s'agit d'un document détaillé, d'un extrait de preuve et d'un extrait de note de version, qui peut être utilisé
utilisez-le lors du basculement pour les clients.