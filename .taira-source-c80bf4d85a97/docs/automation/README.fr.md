---
lang: fr
direction: ltr
source: docs/automation/README.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: c56bacde8ee42c2427d06038a3a6ca65035d4055c42f6e5ded7e54b33c1fe921
source_last_modified: "2025-11-15T13:38:36.954059+00:00"
translation_last_reviewed: 2026-01-21
---

# Bases d'automatisation de la documentation

Ce répertoire regroupe les surfaces d'automatisation évoquées par les éléments
AND5/AND6 (Android Developer Experience + Release Readiness) et DA-1
(automatisation du modèle de menaces de disponibilité des données) lorsqu'ils
exigent des preuves documentaires auditables. Conserver dans l'arborescence les
références de commandes et les artefacts attendus garantit que les prérequis des
revues de conformité restent disponibles même lorsque les pipelines CI ou les
tableaux de bord sont hors ligne.

## Structure du répertoire

| Chemin | Rôle |
|--------|------|
| `docs/automation/android/` | Baselines d'automatisation pour la documentation et la localisation Android (AND5), y compris les journaux de synchronisation des stubs i18n, les résumés de parité et les preuves de publication du SDK requises avant la validation AND6. |
| `docs/automation/da/` | Sorties d'automatisation du modèle de menaces Data-Availability référencées par `cargo xtask da-threat-model-report` et par la mise à jour nocturne de la documentation. |

Chaque sous-répertoire documente les commandes qui produisent les preuves ainsi
que la structure de fichiers attendue (généralement des résumés JSON, des logs ou
des manifestes). Les équipes déposent de nouveaux artefacts dans le dossier
concerné lorsqu'une exécution d'automatisation modifie de façon significative les
docs publiées, puis lient le commit depuis l'entrée pertinente du status/roadmap.

## Utilisation

1. **Exécuter l'automatisation** avec les commandes décrites dans le README du
   sous-répertoire (par exemple `ci/check_android_fixtures.sh` ou
   `cargo xtask da-threat-model-report`).
2. **Copier les artefacts JSON/log résultants** depuis `artifacts/…` vers le
   dossier `docs/automation/<program>/…` correspondant, en ajoutant un horodatage
   ISO-8601 dans le nom du fichier pour que les auditeurs puissent relier les
   preuves aux comptes rendus de gouvernance.
3. **Référencer le commit** dans `status.md`/`roadmap.md` lorsque vous fermez un
   gate du roadmap, afin que les réviseurs puissent confirmer la baseline
   d'automatisation utilisée pour cette décision.
4. **Garder les fichiers légers.** L'attendu est des métadonnées structurées,
   des manifestes ou des résumés, pas de gros blobs binaires. Les volumes lourds
   doivent rester en object storage avec la référence signée consignée ici.

En centralisant ces notes d'automatisation, nous débloquons le prérequis
« baselines docs/automation disponibles pour audit » cité par AND6 et offrons au
flux du modèle de menaces DA un point d'ancrage déterministe pour les rapports
nocturnes et les contrôles ponctuels manuels.
