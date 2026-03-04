---
lang: fr
direction: ltr
source: docs/portal/docs/sorafs/migration-roadmap.ru.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
title : « Carte de migration SoraFS »
---

> Adapté à [`docs/source/sorafs/migration_roadmap.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sorafs/migration_roadmap.md).

# Carte de migration supplémentaire SoraFS (SF-1)

Ce document présente des recommandations opérationnelles pour la migration, en particulier dans
`docs/source/sorafs_architecture_rfc.md`. Он разворачивает livrables SF-1 в
les critères d'utilisation des véhicules, les critères d'attribution et les listes de contrôle des commandes
articles publiés sur la base SoraFS.

La carte du produit doit être nommée : il est nécessaire de choisir un produit non disponible.
articles, commandes et attestations, qui sont des projets en aval
идентичные выходы, а gouvernance сохраняла аудируемый след.

## Обзор вех

| Веха | Okno | Les sels naturels | Je dois maintenant le faire | Владельцы |
|------|------|---------------|-----------------------------|---------------|
| **M1 - Application déterministe** | Jours 7-12 | En publiant des luminaires et en créant des preuves d'alias, chaque page contient des drapeaux d'attente. | Il s'agit actuellement de vérifications, de manifestations avec la demande, de mises en scène dans le registre des alias. | Stockage, gouvernance, SDK |

Le statut est défini dans `docs/source/sorafs/migration_ledger.md`. Все изменения
Dans cette double carte, le Département gère le grand livre, la gouvernance et l'ingénierie des versions.
оставались синхронизированы.

## Pots de robot

### 2. Principe de l'épinglage| Шаг | Веха | Description | Propriétaire(s) | Выход |
|-----|------|----------|---------------|-------|
| Calendrier de répétition | M0 | Il existe des essais à sec, des résumés de fragments locaux avec `fixtures/sorafs_chunker`. Publier la réponse dans `docs/source/sorafs/reports/`. | Fournisseurs de stockage | `determinism-<date>.md` avec réussite/échec. |
| Envoyer des commentaires | M1 | `ci/check_sorafs_fixtures.sh` + `.github/workflows/sorafs-fixtures-nightly.yml` sont compatibles avec la dérive ou les manifestes. Dev annule la renonciation à la gouvernance dans les relations publiques. | GT Outillage | Лог CI, ссылка на renonciation ticket (если применимо). |
| Indicateurs d'attente | M1 | L'application `sorafs_manifest_stub` correspond à vos attentes en matière de configuration de sortie : | Documents CI | Les scripts sont associés aux drapeaux d'attente (avec le bloc de commandes ci-dessous). |
| Épinglage dans le registre en premier | M2 | `sorafs pin propose` et `sorafs pin approve` оборачивают отправку manifeste ; La CLI peut utiliser `--require-registry`. | Opérations de gouvernance | Journal d'audit CLI du registre, informations télémétriques. |
| Parité d'observabilité | M3 | Les tableaux de bord Prometheus/Grafana proposent l'inventaire des fragments et les manifestes de registre ; alert'ы подключены к ops on-call. | Observabilité | Recherchez le tableau de bord, les identifiants et les alertes, les résultats GameDay. |

#### Publications de la commande canonique

```bash
cargo run -p sorafs_manifest --bin sorafs_manifest_stub -- docs/book \
  --manifest-out artifacts/docs/book/2025-11-01/docs.manifest \
  --manifest-signatures-out artifacts/docs/book/2025-11-01/docs.manifest_signatures.json \
  --car-out artifacts/docs/book/2025-11-01/docs.car \
  --chunk-fetch-plan-out artifacts/docs/book/2025-11-01/docs.fetch_plan.json \
  --car-digest=13fa919c67e55a2e95a13ff8b0c6b40b2e51d6ef505568990f3bc7754e6cc482 \
  --car-size=429391872 \
  --root-cid=f40101... \
  --dag-codec=0x71
```

Indiquez le résumé, la définition et le CID dans les fichiers affichés sur la page d'accueil.
registre de migration pour l'art.

### 3. Nom d'alias et communication| Шаг | Веха | Description | Propriétaire(s) | Выход |
|-----|------|----------|---------------|-------|
| Preuves d'alias dans la mise en scène | M1 | Enregistrez les revendications d'alias dans la préparation du registre Pin et enregistrez les preuves Merkle dans les manifestes (`--alias`). | Gouvernance, Docs | Ensemble de preuves composé du manifeste + du grand livre de commentaires avec l'alias principal. |
| Application de la preuve | M2 | Les passerelles отклоняют manifestent les en-têtes `Sora-Proof` ; CI получает шаг `sorafs alias verify` для извлечения preuves. | Réseautage | Le patch configure la passerelle + la sortie CI de manière fiable. |

### 4. Communication et audit

- **Compte rendu du grand livre :** каждое изменение состояния (dérive des luminaires, soumission au registre,
  activation de l'alias) должно добавлять датированную заметку в
  `docs/source/sorafs/migration_ledger.md`.
- **Procès-verbaux de gouvernance :** заседания совета, утверждающие изменения pin registre ou
  alias politique, vous devez vous renseigner sur cette carte et votre grand livre.
- **Communications :** DevRel publie le statut de l'état d'avancement de chaque étape (blog +
  extrait du changelog), подчеркивая детерминированные гарантии и таймлайны alias.

## Précautions et risques| Avis | Влияние | Mitigation |
|------------|---------|---------------|
| Télécharger le registre des broches du contrat | Bloque le déploiement de la broche M2 en premier. | Подготовить contrat до M2 с replay tests; Vous pouvez utiliser le repli de l'enveloppe en cas de régression. |
| Clés pour la soviet | Traitement des enveloppes de manifeste et des approbations du registre. | Cérémonie de signature, description du `docs/source/sorafs/signing_ceremony.md` ; faites pivoter les clés pour ouvrir et inscrire dans le grand livre. |
| Cadence par rapport au SDK | Les clients doivent effectuer des preuves d'alias pour M3. | Synchronisez les SDK avec les portes de jalon ; créez des listes de contrôle de migration dans des modèles de version. |

Risques d'état et atténuation des risques dans `docs/source/sorafs_architecture_rfc.md`
et il est facile de procéder à des réferences croisées lors des modifications.

## Чеклист критериев выхода

| Веха | Critères |
|------|----------|
| M1 | - Travail de nuit sur les appareils зеленый семь дней подряд.  - Staging des preuves d'alias prouvées par CI.  - La gouvernance ratifie les attentes politiques. |

## Mise à jour des modifications

1. Avant de procéder à l'aménagement des relations publiques, veuillez consulter ce fichier ** et **
   `docs/source/sorafs/migration_ledger.md`.
2. Examinez les procès-verbaux de gouvernance et les preuves de CI dans l'analyse des relations publiques.
3. Ensuite, fusionnez le stockage + la liste de diffusion DevRel avec récupération et suppression
   действиями операторов.

Cette procédure garantit que le déploiement SoraFS permet de déterminer,
Les écouteurs et les commandes des commandes se trouvent dans le boîtier Nexus.