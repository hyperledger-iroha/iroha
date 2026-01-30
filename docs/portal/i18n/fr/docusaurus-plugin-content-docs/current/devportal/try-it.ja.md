---
lang: ja
direction: ltr
source: docs/portal/i18n/fr/docusaurus-plugin-content-docs/current/devportal/try-it.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 2d257c43a643ae11cfa11031373f60ae84c3fa10fea821f546f4de47bce4d9ee
source_last_modified: "2025-11-14T04:43:20.195996+00:00"
translation_last_reviewed: 2026-01-30
---

# Bac a sable Try It

Le portail developpeur fournit une console optionnelle "Try it" pour appeler des endpoints Torii sans quitter la documentation. La console relaie les requetes via le proxy embarque pour que les navigateurs contournent les limites CORS tout en appliquant le rate limiting et l'authentification.

## Prerequis

- Node.js 18.18 ou plus recent (correspond aux exigences de build du portail)
- Acces reseau a un environnement Torii de staging
- Un bearer token capable d'appeler les routes Torii que vous voulez tester

Toute la configuration du proxy passe par des variables d'environnement. Le tableau ci-dessous liste les knobs les plus importants:

| Variable | Objectif | Defaut |
| --- | --- | --- |
| `TRYIT_PROXY_TARGET` | URL Torii de base vers laquelle le proxy relaie les requetes | **Required** |
| `TRYIT_PROXY_LISTEN` | Adresse d'ecoute pour le developpement local (format `host:port` ou `[ipv6]:port`) | `127.0.0.1:8787` |
| `TRYIT_PROXY_ALLOWED_ORIGINS` | Liste separee par des virgules des origines autorisees a appeler le proxy | `http://localhost:3000` |
| `TRYIT_PROXY_CLIENT_ID` | Identifiant place dans `X-TryIt-Client` pour chaque requete upstream | `docs-portal` |
| `TRYIT_PROXY_BEARER` | Bearer token par defaut relaie vers Torii | _empty_ |
| `TRYIT_PROXY_ALLOW_CLIENT_AUTH` | Permet aux utilisateurs finaux de fournir leur propre token via `X-TryIt-Auth` | `0` |
| `TRYIT_PROXY_MAX_BODY` | Taille maximale du corps de requete (bytes) | `1048576` |
| `TRYIT_PROXY_TIMEOUT_MS` | Timeout upstream en millisecondes | `10000` |
| `TRYIT_PROXY_RATE_LIMIT` | Requetes autorisees par fenetre de taux par IP client | `60` |
| `TRYIT_PROXY_RATE_WINDOW_MS` | Fenetre glissante pour rate limiting (ms) | `60000` |
| `TRYIT_PROXY_METRICS_LISTEN` | Adresse d'ecoute optionnelle pour l'endpoint de metriques style Prometheus (`host:port` ou `[ipv6]:port`) | _empty (disabled)_ |
| `TRYIT_PROXY_METRICS_PATH` | Chemin HTTP servi par l'endpoint de metriques | `/metrics` |

Le proxy expose aussi `GET /healthz`, renvoie des erreurs JSON structurees, et masque les bearer tokens dans les logs.

Activez `TRYIT_PROXY_ALLOW_CLIENT_AUTH=1` lorsque vous exposez le proxy aux utilisateurs docs pour que les panneaux Swagger et RapiDoc puissent relayer des bearer tokens fournis par l'utilisateur. Le proxy applique toujours les limites de taux, masque les credentiels, et enregistre si une requete a utilise le token par defaut ou une surcharge par requete. Configurez `TRYIT_PROXY_CLIENT_ID` avec le libelle que vous voulez envoyer comme `X-TryIt-Client`
(par defaut `docs-portal`). Le proxy tronque et valide les valeurs `X-TryIt-Client` fournies par l'appelant, puis retombe sur ce defaut afin que les gateways de staging puissent auditer la provenance sans correler les metadonnees du navigateur.

## Demarrer le proxy en local

Installez les dependances lors de la premiere configuration du portail:

```bash
cd docs/portal
npm install
```

Lancez le proxy et pointez-le vers votre instance Torii:

```bash
export TRYIT_PROXY_TARGET="https://torii.devnet.sora.example"
export TRYIT_PROXY_ALLOWED_ORIGINS="http://localhost:3000"
# Optional: preset a bearer token for the Swagger / RapiDoc panels
export TRYIT_PROXY_BEARER="Bearer eyJhbGciOi..."
npm run tryit-proxy
```

Le script logue l'adresse liee et relaie les requetes depuis `/proxy/*` vers l'origine Torii configuree.

Avant de binder le socket le script valide que
`static/openapi/torii.json` correspond au digest enregistre dans
`static/openapi/manifest.json`. Si les fichiers divergent, la commande echoue avec une
erreur et vous demande d'executer `npm run sync-openapi -- --latest`. Exportez
`TRYIT_PROXY_ALLOW_STALE_SPEC=1` uniquement pour des overrides d'urgence; le proxy logue
un avertissement et continue pour vous permettre de recuperer pendant les fenetres de maintenance.

## Cablage des widgets du portail

Quand vous build ou servez le portail developpeur, definissez l'URL que les widgets doivent
utiliser pour le proxy:

```bash
export TRYIT_PROXY_PUBLIC_URL="http://localhost:8787"
export TRYIT_PROXY_DEFAULT_BEARER="Bearer eyJhbGciOi..." # Optional
npm run start
```

Les composants suivants lisent ces valeurs depuis `docusaurus.config.js`:

- **Swagger UI** - rendu a `/reference/torii-swagger`; preautorise le schema bearer
  quand un token est present, tague les requetes avec `X-TryIt-Client`, injecte
  `X-TryIt-Auth`, et reecrit les appels via le proxy quand
  `TRYIT_PROXY_PUBLIC_URL` est defini.
- **RapiDoc** - rendu a `/reference/torii-rapidoc`; reflete le champ token,
  reutilise les memes headers que le panneau Swagger, et cible automatiquement
  le proxy lorsque l'URL est configuree.
- **Try it console** - integree sur la page d'overview API; permet d'envoyer des
  requetes personnalisees, voir les headers, et inspecter les corps de reponse.

Les deux panneaux affichent un **selecteur de snapshots** qui lit
`docs/portal/static/openapi/versions.json`. Remplissez cet index avec
`npm run sync-openapi -- --version=<label> --mirror=current --latest` afin que les reviewers
puissent passer entre les specs historiques, voir le digest SHA-256 enregistre, et confirmer
si un snapshot de release embarque un manifest signe avant d'utiliser les widgets interactifs.

Changer le token dans un widget ne touche que la session navigateur courante; le proxy ne
persiste jamais et ne logue jamais le token fourni.

## Tokens OAuth courte duree

Pour eviter de distribuer des tokens Torii longue duree aux reviewers, reliez la console Try it a
votre serveur OAuth. Lorsque les variables d'environnement ci-dessous sont presentes, le portail
rend un widget de login device code, emet des bearer tokens courte duree, et les injecte
automatiquement dans le formulaire de la console.

| Variable | Objectif | Defaut |
| --- | --- | --- |
| `DOCS_OAUTH_DEVICE_CODE_URL` | Endpoint d'autorisation device OAuth (`/oauth/device/code`) | _empty (disabled)_ |
| `DOCS_OAUTH_TOKEN_URL` | Endpoint token qui accepte `grant_type=urn:ietf:params:oauth:grant-type:device_code` | _empty_ |
| `DOCS_OAUTH_CLIENT_ID` | Identifiant client OAuth enregistre pour la preview docs | _empty_ |
| `DOCS_OAUTH_SCOPE` | Scopes delimites par des espaces demandes lors du login | `openid profile offline_access` |
| `DOCS_OAUTH_AUDIENCE` | Audience API optionnelle pour lier le token | _empty_ |
| `DOCS_OAUTH_POLL_INTERVAL_MS` | Intervalle minimum de polling pendant l'attente d'approbation (ms) | `5000` (valeurs < 5000 ms rejetees) |
| `DOCS_OAUTH_DEVICE_CODE_TTL_SECONDS` | Fenetre d'expiration device code (secondes) | `600` (doit rester entre 300 s et 900 s) |
| `DOCS_OAUTH_TOKEN_TTL_SECONDS` | Duree de vie access token (secondes) | `900` (doit rester entre 300 s et 900 s) |
| `DOCS_OAUTH_ALLOW_INSECURE` | Mettre `1` pour des previews locales qui omettent l'enforcement OAuth intentionnellement | _unset_ |

Exemple de configuration:

```bash
export DOCS_OAUTH_DEVICE_CODE_URL="https://auth.dev.sora.example/oauth/device/code"
export DOCS_OAUTH_TOKEN_URL="https://auth.dev.sora.example/oauth/token"
export DOCS_OAUTH_CLIENT_ID="docs-preview"
export DOCS_OAUTH_SCOPE="torii openid offline_access"
# Optional audience and polling tweaks
export DOCS_OAUTH_AUDIENCE="https://torii.devnet.sora.example"
export DOCS_OAUTH_POLL_INTERVAL_MS="6000"
```

Quand vous lancez `npm run start` ou `npm run build`, le portail integre ces valeurs
dans `docusaurus.config.js`. En preview locale la carte Try it affiche un bouton
"Sign in with device code". Les utilisateurs saisissent le code affiche sur votre page OAuth; une fois que le device flow reussit le widget:

- injecte le bearer token emis dans le champ console Try it,
- tague les requetes avec les headers existants `X-TryIt-Client` et `X-TryIt-Auth`,
- affiche la duree restante, et
- efface automatiquement le token quand il expire.

L'entree manuelle Bearer reste disponible; omettez les variables OAuth lorsque vous voulez
forcer les reviewers a coller un token temporaire eux-memes, ou exportez
`DOCS_OAUTH_ALLOW_INSECURE=1` pour des previews locales isolees ou l'acces anonyme est
acceptable. Les builds sans OAuth configure echouent maintenant rapidement pour satisfaire
pour satisfaire la gate du roadmap DOCS-1b.

Note: Consultez la [Security hardening & pen-test checklist](./security-hardening.md)
avant d'exposer le portail hors du labo; elle documente le threat model,
le profil CSP/Trusted Types, et les etapes de pen-test qui bloquent maintenant DOCS-1b.

## Exemples Norito-RPC

Les requetes Norito-RPC partagent le meme proxy et le plumbing OAuth que les routes JSON;
elles posent simplement `Content-Type: application/x-norito` et envoient le payload Norito
pre-encode decrit dans la specification NRPC
(`docs/source/torii/nrpc_spec.md`).
Le depot fournit des payloads canoniques sous `fixtures/norito_rpc/` pour que les auteurs du
portail, owners SDK, et reviewers puissent rejouer les bytes exacts utilises par CI.

### Envoyer un payload Norito depuis la console Try It

1. Choisissez un fixture comme `fixtures/norito_rpc/transfer_asset.norito`. Ces
   fichiers sont des envelopes Norito bruts; **ne** les base64-encodez pas.
2. Dans Swagger ou RapiDoc, localisez l'endpoint NRPC (par exemple
   `POST /v1/pipeline/submit`) et basculez le selecteur **Content-Type** sur
   `application/x-norito`.
3. Basculez l'editeur de body en **binary** (mode "File" de Swagger ou
   selecteur "Binary/File" de RapiDoc) et chargez le fichier `.norito`. Le widget
   transmet les bytes via le proxy sans alteration.
4. Soumettez la requete. Si Torii renvoie `X-Iroha-Error-Code: schema_mismatch`,
   verifiez que vous appelez un endpoint qui accepte des payloads binaires et confirmez
   que le schema hash enregistre dans `fixtures/norito_rpc/schema_hashes.json`
   correspond au build Torii cible.

La console conserve le fichier le plus recent en memoire pour que vous puissiez renvoyer le
meme payload tout en testant differents tokens d'autorisation ou hosts Torii. Ajouter
`scripts/run_norito_rpc_fixtures.sh --note "<ticket>"` a votre workflow produit le bundle
de preuves reference dans le plan d'adoption NRPC-4 (log + resume JSON), ce qui va bien
avec la capture d'ecran de la reponse Try It lors des reviews.

### Exemple CLI (curl)

Les memes fixtures peuvent etre rejoues hors du portail via `curl`, ce qui est utile
lorsque vous validez le proxy ou deboguez les reponses gateway:

```bash
TORII="https://torii.devnet.sora.example"
TOKEN="Bearer $(cat ~/.config/torii/devnet.token)"
curl   -H "Content-Type: application/x-norito"   -H "Authorization: ${TOKEN}"   --data-binary @fixtures/norito_rpc/transfer_asset.norito   "${TORII}/v1/pipeline/submit"
```

Remplacez le fixture par n'importe quelle entree listee dans `transaction_fixtures.manifest.json`
ou encodez votre propre payload avec `cargo xtask norito-rpc-fixtures`. Quand Torii est en
mode canary vous pouvez pointer `curl` vers le proxy try-it
(`https://docs.sora.example/proxy/v1/pipeline/submit`) pour exercer la meme infrastructure
que les widgets du portail utilisent.

## Observabilite et operations

Chaque requete est loguee une fois avec methode, path, origine, statut upstream et la source
d'authentification (`override`, `default` ou `client`). Les tokens ne sont jamais stockes: les
headers bearer et les valeurs `X-TryIt-Auth` sont rediges avant log, afin que vous puissiez
relayer stdout vers un collecteur central sans craindre des fuites.

### Probes de sante et alertes

Lancez la probe incluse pendant les deploiements ou sur un schedule:

```bash
# Ensure the proxy responds to /healthz and forwards a sample request.
TRYIT_PROXY_PUBLIC_URL="https://docs.sora.example/proxy" TRYIT_PROXY_SAMPLE_PATH="/v1/status" npm run probe:tryit-proxy
```

Knobs d'environnement:

- `TRYIT_PROXY_SAMPLE_PATH` - route Torii optionnelle (sans `/proxy`) a exercer.
- `TRYIT_PROXY_SAMPLE_METHOD` - par defaut `GET`; definir `POST` pour les routes d'ecriture.
- `TRYIT_PROXY_PROBE_TOKEN` - injecte un bearer token temporaire pour l'appel d'exemple.
- `TRYIT_PROXY_PROBE_TIMEOUT_MS` - ecrase le timeout par defaut de 5 s.
- `TRYIT_PROXY_PROBE_METRICS_FILE` - destination texte Prometheus optionnelle pour `probe_success`/`probe_duration_seconds`.
- `TRYIT_PROXY_PROBE_LABELS` - paires `key=value` separees par des virgules ajoutees aux metriques (par defaut `job=tryit-proxy` et `instance=<proxy URL>`).
- `TRYIT_PROXY_PROBE_METRICS_URL` - URL optionnelle de l'endpoint de metriques (par exemple, `http://localhost:9798/metrics`) qui doit repondre avec succes lorsque `TRYIT_PROXY_METRICS_LISTEN` est active.

Injectez les resultats dans un textfile collector en pointant la probe vers un chemin writable
(par exemple, `/var/lib/node_exporter/textfile_collector/tryit.prom`) et en ajoutant des labels
personnalises:

```bash
TRYIT_PROXY_PUBLIC_URL="https://docs.sora.example/proxy" TRYIT_PROXY_PROBE_METRICS_FILE="/var/lib/node_exporter/textfile_collector/tryit.prom" TRYIT_PROXY_PROBE_LABELS="job=tryit-proxy,cluster=prod" npm run probe:tryit-proxy
```

Le script reecrit le fichier de metriques de facon atomique pour que votre collecteur lise
toujours un payload complet.

Quand `TRYIT_PROXY_METRICS_LISTEN` est configure, definissez
`TRYIT_PROXY_PROBE_METRICS_URL` sur l'endpoint de metriques pour que la probe echoue rapidement si
la surface de scrape disparait (par exemple ingress mal configure ou regles firewall manquantes).
Un reglage production typique est
`TRYIT_PROXY_PROBE_METRICS_URL="http://127.0.0.1:9798/metrics"`.

Pour des alertes legeres, connectez la probe a votre stack de monitoring. Exemple Prometheus
qui page apres deux echecs consecutifs:

```yaml
groups:
  - name: tryit-proxy
    rules:
      - alert: TryItProxyUnhealthy
        expr: probe_success{job="tryit-proxy"} == 0
        for: 2m
        labels:
          severity: page
        annotations:
          summary: Try It proxy is failing health checks
          description: |
            The try-it proxy at {{ $labels.instance }} is not responding to probe requests.
```

### Endpoint de metriques et dashboards

Definissez `TRYIT_PROXY_METRICS_LISTEN=127.0.0.1:9798` (ou tout couple host/port) avant de
lancer le proxy pour exposer un endpoint de metriques au format Prometheus. Le chemin
par defaut est `/metrics` mais peut etre remplace par `TRYIT_PROXY_METRICS_PATH=/custom`. Chaque
scrape renvoie des compteurs des totaux par methode, des rejets rate limit, des erreurs/timeouts
upstream, des outcomes proxy et des resumes de latence:

```bash
export TRYIT_PROXY_METRICS_LISTEN="127.0.0.1:9798"
npm run tryit-proxy &
curl http://127.0.0.1:9798/metrics | head -n 5
# HELP tryit_proxy_requests_total Requests handled by method
tryit_proxy_requests_total{method="GET"} 12
tryit_proxy_rate_limited_total 1
```

Pointez vos collecteurs Prometheus/OTLP vers l'endpoint de metriques et reutilisez les panels
existants dans `dashboards/grafana/docs_portal.json` afin que SRE observe les latences de queue
et les pics de rejet sans parser les logs. Le proxy publie automatiquement
`tryit_proxy_start_timestamp_ms` pour aider les operateurs a detecter les redemarrages.

### Automatisation du rollback

Utilisez le helper de gestion pour mettre a jour ou restaurer l'URL cible Torii. Le script
stocke la configuration precedente dans `.env.tryit-proxy.bak` afin que les rollbacks soient
une seule commande.

```bash
# Update TRYIT_PROXY_TARGET and back up the previous config.
npm run manage:tryit-proxy -- update --target https://torii.devnet.sora.example

# Roll back to the previously backed-up target.
npm run manage:tryit-proxy -- rollback
```

Surchargez le chemin du fichier env avec `--env` ou `TRYIT_PROXY_ENV` si votre deploiement
stocke la configuration ailleurs.
