---
lang: fr
direction: ltr
source: docs/portal/docs/norito/try-it-console.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
title: Console Try-It de Norito
description: Utilisez le proxy du portail developpeur ainsi que les widgets Swagger et RapiDoc pour envoyer de vraies requetes Torii / Norito-RPC directement depuis le site de documentation.
---

Le portail regroupe trois surfaces interactives qui relaient le trafic vers Torii :

- **Swagger UI** a `/reference/torii-swagger` rend la specification OpenAPI signee et reecrit automatiquement les requetes via le proxy lorsque `TRYIT_PROXY_PUBLIC_URL` est defini.
- **RapiDoc** a `/reference/torii-rapidoc` expose le meme schema avec des televersements de fichiers et des selecteurs de type de contenu qui fonctionnent bien pour `application/x-norito`.
- **Try it sandbox** sur la page d'apercu Norito fournit un formulaire leger pour des requetes REST ad hoc et des connexions OAuth par appareil.

Les trois widgets envoient les requetes au **proxy Try-It** local (`docs/portal/scripts/tryit-proxy.mjs`). Le proxy verifie que `static/openapi/torii.json` correspond au digest signe dans `static/openapi/manifest.json`, applique un limiteur de debit, caviarde les en-tetes `X-TryIt-Auth` dans les logs, et tague chaque appel upstream avec `X-TryIt-Client` afin que les operateurs Torii puissent auditer les sources de trafic.

## Lancer le proxy

```bash
cd docs/portal
npm install
export TRYIT_PROXY_TARGET="https://torii.devnet.sora.example"
export TRYIT_PROXY_ALLOWED_ORIGINS="http://localhost:3000"
export TRYIT_PROXY_PUBLIC_URL="http://localhost:8787"
# Optional, use short-lived tokens only:
export TRYIT_PROXY_BEARER="Bearer eyJhbGciOi..."
export TRYIT_PROXY_CLIENT_ID="docs-portal"
export DOCS_TRYIT_ALLOW_DEFAULT_BEARER=1
npm run tryit-proxy
```

- `TRYIT_PROXY_TARGET` est l'URL de base de Torii que vous voulez exercer.
- `TRYIT_PROXY_ALLOWED_ORIGINS` doit inclure chaque origine du portail (serveur local, hostname de production, URL d'apercu) qui doit integrer la console.
- `TRYIT_PROXY_PUBLIC_URL` est consommee par `docusaurus.config.js` et injectee dans les widgets via `customFields.tryIt`.
- `TRYIT_PROXY_BEARER` n'est charge que lorsque `DOCS_TRYIT_ALLOW_DEFAULT_BEARER=1` ; sinon les utilisateurs doivent fournir leur propre jeton via la console ou le flux device OAuth.
- `TRYIT_PROXY_CLIENT_ID` definit le tag `X-TryIt-Client` transporte sur chaque requete.
  Fournir `X-TryIt-Client` depuis le navigateur est autorise mais les valeurs sont tronquees
  et rejetees si elles contiennent des caracteres de controle.

Au demarrage, le proxy execute `verifySpecDigest` et se termine avec un indice de remediation si le manifeste est perime. Executez `npm run sync-openapi -- --latest` pour telecharger la specification Torii la plus recente ou passez `TRYIT_PROXY_ALLOW_STALE_SPEC=1` pour des derogations d'urgence.

Pour mettre a jour ou revenir en arriere sur la cible du proxy sans editer les fichiers d'environnement a la main, utilisez l'outil d'aide :

```bash
npm run manage:tryit-proxy -- update --target https://new.torii.example
npm run manage:tryit-proxy -- rollback
```

## Brancher les widgets

Servez le portail apres que le proxy est a l'ecoute :

```bash
cd docs/portal
TRYIT_PROXY_PUBLIC_URL="http://localhost:8787" npm run start
```

`docusaurus.config.js` expose les reglages suivants :

| Variable | Role |
| --- | --- |
| `TRYIT_PROXY_PUBLIC_URL` | URL injectee dans Swagger, RapiDoc et le sandbox Try it. Laisser vide pour masquer les widgets lors des apercus non autorises. |
| `TRYIT_PROXY_DEFAULT_BEARER` | Jeton par defaut optionnel conserve en memoire. Necessite `DOCS_TRYIT_ALLOW_DEFAULT_BEARER=1` et le garde-fou CSP HTTPS uniquement (DOCS-1b) sauf si vous passez `DOCS_SECURITY_ALLOW_INSECURE=1` en local. |
| `DOCS_OAUTH_*` | Active le flux device OAuth (`OAuthDeviceLogin` component) afin que les relecteurs puissent emettre des jetons de courte duree sans quitter le portail. |

Lorsque les variables OAuth sont presentes, le sandbox affiche un bouton **Sign in with device code** qui passe par le serveur Auth configure (voir `config/security-helpers.js` pour le format exact). Les jetons emis via le flux device ne sont mis en cache que dans la session du navigateur.

## Envoyer des payloads Norito-RPC

1. Construisez un payload `.norito` avec le CLI ou les extraits decrits dans le [quickstart Norito](./quickstart.md). Le proxy transfere les corps `application/x-norito` sans modification, vous pouvez donc reutiliser le meme artefact que vous enverriez avec `curl`.
2. Ouvrez `/reference/torii-rapidoc` (prefere pour les payloads binaires) ou `/reference/torii-swagger`.
3. Selectionnez le snapshot Torii souhaite dans la liste deroulante. Les snapshots sont signes ; le panneau affiche le digest du manifeste enregistre dans `static/openapi/manifest.json`.
4. Choisissez le type de contenu `application/x-norito` dans le tiroir "Try it", cliquez sur **Choose File**, et selectionnez votre payload. Le proxy reecrit la requete vers `/proxy/v1/pipeline/submit` et la tague avec `X-TryIt-Client=docs-portal-rapidoc`.
5. Pour telecharger les reponses Norito, definissez `Accept: application/x-norito`. Swagger/RapiDoc exposent le selecteur d'en-tete dans le meme tiroir et renvoient le binaire via le proxy.

Pour les routes uniquement JSON, le sandbox Try it integre est souvent plus rapide : saisissez le chemin (par exemple `/v1/accounts/<katakana-i105-account-id>/assets`), selectionnez la methode HTTP, collez un corps JSON si necessaire, et cliquez sur **Send request** pour inspecter les en-tetes, la duree et les payloads en ligne.

## Depannage

| Symptome | Cause probable | Remediation |
| --- | --- | --- |
| La console du navigateur affiche des erreurs CORS ou le sandbox avertit que l'URL du proxy manque. | Le proxy n'est pas en cours d'execution ou l'origine n'est pas autorisee. | Lancez le proxy, assurez-vous que `TRYIT_PROXY_ALLOWED_ORIGINS` couvre l'hote du portail, puis relancez `npm run start`. |
| `npm run tryit-proxy` se termine avec  digest mismatch . | Le bundle OpenAPI de Torii a change en amont. | Executez `npm run sync-openapi -- --latest` (ou `--version=<tag>`) puis reessayez. |
| Les widgets renvoient `401` ou `403`. | Jeton manquant, expire ou scopes insuffisants. | Utilisez le flux device OAuth ou collez un bearer token valide dans le sandbox. Pour les jetons statiques, vous devez exporter `DOCS_TRYIT_ALLOW_DEFAULT_BEARER=1`. |
| `429 Too Many Requests` depuis le proxy. | La limite de debit par IP est depassee. | Augmentez `TRYIT_PROXY_RATE_LIMIT`/`TRYIT_PROXY_RATE_WINDOW_MS` pour les environnements de confiance ou reduisez les scripts de test. Tous les refus pour rate limit incrementent `tryit_proxy_rate_limited_total`. |

## Observabilite

- `npm run probe:tryit-proxy` (wrapper autour de `scripts/tryit-proxy-probe.mjs`) appelle `/healthz`, execute eventuellement une route d'exemple, et emet des fichiers texte Prometheus pour `probe_success` / `probe_duration_seconds`. Configurez `TRYIT_PROXY_PROBE_METRICS_FILE` pour l'integrer a node_exporter.
- Reglez `TRYIT_PROXY_METRICS_LISTEN=127.0.0.1:9798` pour exposer des compteurs (`tryit_proxy_requests_total`, `tryit_proxy_rate_limited_total`, `tryit_proxy_upstream_failures_total`) et des histogrammes de latence. Le tableau de bord `dashboards/grafana/docs_portal.json` lit ces metriques pour appliquer les SLOs DOCS-SORA.
- Les logs d'execution se trouvent sur stdout. Chaque entree inclut l'id de requete, le statut upstream, la source d'authentification (`default`, `override` ou `client`), et la duree ; les secrets sont caviardes avant emission.

Si vous devez valider que les payloads `application/x-norito` atteignent Torii sans modification, executez la suite Jest (`npm test -- tryit-proxy`) ou inspectez les fixtures dans `docs/portal/scripts/__tests__/tryit-proxy.test.mjs`. Les tests de regression couvrent les binaires Norito compresses, les manifestes OpenAPI signes et les chemins de retrogradation du proxy afin que les deploiements NRPC conservent une piste d'audit permanente.
