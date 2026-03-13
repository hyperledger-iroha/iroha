---
lang: fr
direction: ltr
source: docs/devportal/try-it.md
status: complete
translator: manual
source_hash: 791d88296d9e52d3272ce3ac324e498fa3c622c323edc8c988302efe5092f0b4
source_last_modified: "2026-02-03T00:00:00Z"
translation_last_reviewed: 2026-02-03
---

<!-- Traduction française de docs/devportal/try-it.md (Try It Sandbox Guide) -->

---
title: Guide du sandbox « Try It »
summary: Comment lancer le proxy Torii de staging et le sandbox du portail développeur.
---

Le portail développeur propose une console « Try it » pour l’API REST Torii. Ce guide
explique comment lancer le proxy associé et connecter la console à une passerelle de
staging sans exposer de credentials.

## Prérequis

- Checkout du dépôt Iroha (racine du workspace).
- Node.js 18.18+ (aligné sur le baseline du portail).
- Endpoint Torii accessible depuis votre machine (staging ou local).

## 1. Générer le snapshot OpenAPI (optionnel)

La console réutilise le même payload OpenAPI que les pages de référence du portail. Si
vous avez modifié des routes Torii, régénérez le snapshot :

```bash
cargo xtask openapi
```

La tâche écrit `docs/portal/static/openapi/torii.json`.

## 2. Démarrer le proxy Try It

Depuis la racine du dépôt :

```bash
cd docs/portal

export TRYIT_PROXY_TARGET="https://torii.staging.sora"
export TRYIT_PROXY_ALLOWED_ORIGINS="http://localhost:3000"
# Defaults optionnels
export TRYIT_PROXY_BEARER="sora-dev-token"
export TRYIT_PROXY_LISTEN="127.0.0.1:8787"

npm run tryit-proxy
```

### Variables d’environnement

| Variable | Description |
|----------|-------------|
| `TRYIT_PROXY_TARGET` | URL de base Torii (obligatoire). |
| `TRYIT_PROXY_ALLOWED_ORIGINS` | Liste d’origines (séparées par des virgules) autorisées à utiliser le proxy (par défaut `http://localhost:3000`). |
| `TRYIT_PROXY_BEARER` | Bearer token par défaut optionnel appliqué à toutes les requêtes proxied. |
| `TRYIT_PROXY_ALLOW_CLIENT_AUTH` | Mettre à `1` pour relayer tel quel l’en‑tête `Authorization` du client. |
| `TRYIT_PROXY_RATE_LIMIT` / `TRYIT_PROXY_RATE_WINDOW_MS` | Paramètres du rate‑limiter en mémoire (par défaut 60 requêtes par 60 s). |
| `TRYIT_PROXY_MAX_BODY` | Taille maximale de payload acceptée (en octets, par défaut 1 MiB). |
| `TRYIT_PROXY_TIMEOUT_MS` | Timeout upstream pour les requêtes Torii (par défaut 10 000 ms). |

Le proxy expose :

- `GET /healthz` — vérification de readiness.
- `/proxy/*` — requêtes proxied, conservant path et query string.

## 3. Lancer le portail

Dans un autre terminal :

```bash
cd docs/portal
export TRYIT_PROXY_PUBLIC_URL="http://localhost:8787"
npm run start
```

Rendez‑vous sur `http://localhost:3000/api/overview` et utilisez la console Try It. Les
mêmes variables d’environnement configurent les embeds Swagger UI et RapiDoc.

## 4. Exécuter les tests unitaires

Le proxy fournit une suite de tests rapide basée sur Node :

```bash
npm run test:tryit-proxy
```

Les tests couvrent le parsing d’adresse, la gestion des origines, le rate limiting et
l’injection de bearer token.

## 5. Automatisation des probes et métriques

Utilisez le probe fourni pour vérifier `/healthz` et une route d’exemple :

```bash
TRYIT_PROXY_PUBLIC_URL="https://docs.sora.example/proxy" \
TRYIT_PROXY_SAMPLE_PATH="/v2/status" \
npm run probe:tryit-proxy
```

Variables utiles :

- `TRYIT_PROXY_SAMPLE_PATH` — route Torii optionnelle (sans `/proxy`) à exercer.
- `TRYIT_PROXY_SAMPLE_METHOD` — `GET` par défaut ; basculez sur `POST` pour les routes d’écriture.
- `TRYIT_PROXY_PROBE_TOKEN` — injecte un bearer token temporaire pour l’appel d’exemple.
- `TRYIT_PROXY_PROBE_TIMEOUT_MS` — remplace le timeout par défaut de 5 s.
- `TRYIT_PROXY_PROBE_METRICS_FILE` — destination textfile Prometheus pour `probe_success`/`probe_duration_seconds`.
- `TRYIT_PROXY_PROBE_LABELS` — liste `clé=valeur` séparée par des virgules ajoutée aux labels (par défaut `job=tryit-proxy` et `instance=<URL du proxy>`).

Lorsque `TRYIT_PROXY_PROBE_METRICS_FILE` est défini, le script réécrit le fichier de façon
atomique afin que votre collecteur (node_exporter/textfile) lise toujours un payload complet. Exemple :

```bash
TRYIT_PROXY_PUBLIC_URL="https://docs.sora.example/proxy" \
TRYIT_PROXY_PROBE_METRICS_FILE="/var/lib/node_exporter/textfile_collector/tryit.prom" \
TRYIT_PROXY_PROBE_LABELS="job=tryit-proxy,cluster=staging" \
npm run probe:tryit-proxy
```

Publiez ensuite les métriques vers Prometheus et réutilisez la règle d’alerte fournie pour pager
quand `probe_success` retombe à `0`.

## 6. Checklist de durcissement pour la production

Avant de publier le proxy au‑delà du développement local :

- Terminer TLS en amont du proxy (reverse‑proxy ou passerelle managée).
- Configurer des logs structurés et les envoyer vers vos pipelines d’observabilité.
- Faire tourner régulièrement les bearer tokens et les stocker dans un gestionnaire de
  secrets.
- Surveiller l’endpoint `/healthz` du proxy et agréger des métriques de latence.
- Aligner les limites de rate sur les quotas de staging Torii ; ajuster le comportement de
  `Retry-After` pour communiquer clairement le throttling aux clients.
