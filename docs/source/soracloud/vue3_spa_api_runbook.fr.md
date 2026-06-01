<!-- Auto-generated stub for French (fr) translation. Replace this content with the full translation. -->

---
lang: fr
direction: ltr
source: docs/source/soracloud/vue3_spa_api_runbook.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 8655a6c1d1e79d6e1c468fd6e27f7266189e47398db7dde48dced6db5c6fba72
source_last_modified: "2026-03-24T18:59:46.536363+00:00"
translation_last_reviewed: 2026-04-02
translator: machine-google-reviewed
---

# Soracloud Vue3 SPA + Runbook API

Ce runbook couvre le déploiement et les opérations orientés production pour :

- un site statique Vue3 (`--template site`) ; et
- un service Vue3 SPA + API (`--template webapp`),

en utilisant les API du plan de contrôle Soracloud sur Iroha 3 avec les hypothèses SCR/IVM (non
Dépendance d'exécution WASM et aucune dépendance Docker).

## 1. Générer des projets modèles

Échafaudage de chantier statique :

```bash
iroha app soracloud init \
  --template site \
  --service-name docs_portal \
  --service-version 1.0.0 \
  --output-dir .soracloud-docs
```

Échafaudage SPA + API :

```bash
iroha app soracloud init \
  --template webapp \
  --service-name agent_console \
  --service-version 1.0.0 \
  --output-dir .soracloud-agent
```

Chaque répertoire de sortie comprend :

-`container_manifest.json`
-`service_manifest.json`
- fichiers sources du modèle sous `site/` ou `webapp/`

## 2. Créer des artefacts d'application

Site statique :

```bash
cd .soracloud-docs/site
npm install
npm run build
```

Frontend SPA + API :

```bash
cd .soracloud-agent/webapp
npm install
npm --prefix frontend install
npm --prefix frontend run build
```

## 3. Packager et publier des actifs frontend

Pour l'hébergement statique via SoraFS :

```bash
iroha app sorafs toolkit pack ./dist \
  --manifest-out ../sorafs/site_manifest.to \
  --car-out ../sorafs/site_payload.car \
  --json-out ../sorafs/site_pack_report.json
```

Pour l'interface SPA :

```bash
iroha app sorafs toolkit pack ./frontend/dist \
  --manifest-out ../sorafs/frontend_manifest.to \
  --car-out ../sorafs/frontend_payload.car \
  --json-out ../sorafs/frontend_pack_report.json
```

## 4. Déployer sur le plan de contrôle Soracloud en direct

Déployer le service de site statique :

```bash
iroha app soracloud deploy \
  --container .soracloud-docs/container_manifest.json \
  --service .soracloud-docs/service_manifest.json \
  --torii-url http://127.0.0.1:8080
```

Déployer le service SPA + API :

```bash
iroha app soracloud deploy \
  --container .soracloud-agent/container_manifest.json \
  --service .soracloud-agent/service_manifest.json \
  --torii-url http://127.0.0.1:8080
```

Validez la liaison de route et l’état de déploiement :

```bash
iroha app soracloud status --torii-url http://127.0.0.1:8080
```

Vérifications attendues du plan de contrôle :

- Ensemble `control_plane.services[].latest_revision.route_host`
- Ensemble `control_plane.services[].latest_revision.route_path_prefix` (`/` ou `/api`)
- `control_plane.services[].active_rollout` présent immédiatement après la mise à niveau

## 5. Mise à niveau avec un déploiement sécurisé

1. Bump `service_version` dans le manifeste de service.
2. Exécutez la mise à niveau :

```bash
iroha app soracloud upgrade \
  --container container_manifest.json \
  --service service_manifest.json \
  --torii-url http://127.0.0.1:8080
```

3. Promouvoir le déploiement après les contrôles de santé :

```bash
iroha app soracloud rollout \
  --service-name docs_portal \
  --rollout-handle <handle_from_upgrade_output> \
  --health healthy \
  --promote-to-percent 100 \
  --governance-tx-hash <tx_hash> \
  --torii-url http://127.0.0.1:8080
```

4. Si l'état de santé échoue, signalez-le en mauvais état :```bash
iroha app soracloud rollout \
  --service-name docs_portal \
  --rollout-handle <handle_from_upgrade_output> \
  --health unhealthy \
  --governance-tx-hash <tx_hash> \
  --torii-url http://127.0.0.1:8080
```

Lorsque les rapports défectueux atteignent le seuil de politique, Soracloud lance automatiquement
revenir à la révision de référence et enregistrer les événements d'audit de restauration.

## 6. Restauration manuelle et réponse aux incidents

Retour à la version précédente :

```bash
iroha app soracloud rollback \
  --service-name docs_portal \
  --torii-url http://127.0.0.1:8080
```

Utilisez la sortie d'état pour confirmer :

- `current_version` inversé
- `audit_event_count` incrémenté
- `active_rollout` effacé
- `last_rollout.stage` est `RolledBack` pour les restaurations automatiques

## 7. Liste de contrôle des opérations

- Conservez les manifestes générés par le modèle sous contrôle de version.
- Enregistrez `governance_tx_hash` pour chaque étape de déploiement afin de préserver la traçabilité.
- Traiter `service_health`, `routing`, `resource_pressure`, et
  `failed_admissions` comme entrées de porte de déploiement.
- Utiliser des pourcentages canaris et une promotion explicite plutôt que des coupes complètes directes
  mises à niveau pour les services destinés aux utilisateurs.
- Valider le comportement de session/authentification et de vérification de signature dans
  `webapp/api/server.mjs` avant production.