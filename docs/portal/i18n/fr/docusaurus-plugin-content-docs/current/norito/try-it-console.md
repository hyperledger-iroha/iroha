<!-- Auto-generated stub for French (fr) translation. Replace this content with the full translation. -->

---
lang: fr
direction: ltr
source: docs/portal/docs/norito/try-it-console.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
title: Console Try-It de Norito
description: Utilisez le proxy du portail développeur ainsi que les widgets Swagger et RapiDoc pour envoyer de vraies requêtes Torii / Norito-RPC directement depuis le site de documentation.
---

Le portail regroupe trois surfaces interactives qui relaient le trafic vers Torii :

- **Swagger UI** à `/reference/torii-swagger` rend la spécification OpenAPI signée et réécrit automatiquement les requêtes via le proxy lorsque `TRYIT_PROXY_PUBLIC_URL` est défini.
- **RapiDoc** à `/reference/torii-rapidoc` expose le même schéma avec des téléversements de fichiers et des sélecteurs de type de contenu qui fonctionnent bien pour `application/x-norito`.
- **Try it sandbox** sur la page d'aperçu Norito fournit un formulaire léger pour des requêtes REST ad hoc et des connexions OAuth par appareil.

Les trois widgets envoient les requêtes au **proxy Try-It** local (`docs/portal/scripts/tryit-proxy.mjs`). Le proxy vérifie que `static/openapi/torii.json` correspond au digest signé dans `static/openapi/manifest.json`, applique un limiteur de débit, caviarde les en-têtes `X-TryIt-Auth` dans les logs, et tague chaque appel upstream avec `X-TryIt-Client` afin que les opérateurs Torii puissent auditer les sources de trafic.

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
- `TRYIT_PROXY_ALLOWED_ORIGINS` doit inclure chaque origine du portail (serveur local, hostname de production, URL d'aperçu) qui doit intégrer la console.
- `TRYIT_PROXY_PUBLIC_URL` est consommée par `docusaurus.config.js` et injectée dans les widgets via `customFields.tryIt`.
- `TRYIT_PROXY_BEARER` n'est chargé que lorsque `DOCS_TRYIT_ALLOW_DEFAULT_BEARER=1` ; sinon les utilisateurs doivent fournir leur propre jeton via la console ou le flux device OAuth.
- `TRYIT_PROXY_CLIENT_ID` définit le tag `X-TryIt-Client` transporté sur chaque requête.
  Fournir `X-TryIt-Client` depuis le navigateur est autorisé mais les valeurs sont tronquées
  et rejetées si elles contiennent des caractères de contrôle.

Au démarrage, le proxy exécute `verifySpecDigest` et se termine avec un indice de remédiation si le manifeste est périmé. Exécutez `npm run sync-openapi -- --latest` pour télécharger la spécification Torii la plus récente ou passez `TRYIT_PROXY_ALLOW_STALE_SPEC=1` pour des dérogations d'urgence.

Pour mettre à jour ou revenir en arrière sur la cible du proxy sans éditer les fichiers d'environnement à la main, utilisez l'outil d'aide :

```bash
npm run manage:tryit-proxy -- update --target https://new.torii.example
npm run manage:tryit-proxy -- rollback
```

## Brancher les widgets

Servez le portail après que le proxy est à l'écoute :

```bash
cd docs/portal
TRYIT_PROXY_PUBLIC_URL="http://localhost:8787" npm run start
```

`docusaurus.config.js` expose les réglages suivants :

| Variable | Rôle |
| --- | --- |
| `TRYIT_PROXY_PUBLIC_URL` | URL injectée dans Swagger, RapiDoc et le sandbox Try it. Laisser vide pour masquer les widgets lors des aperçus non autorisés. |
| `TRYIT_PROXY_DEFAULT_BEARER` | Jeton par défaut optionnel conservé en mémoire. Nécessite `DOCS_TRYIT_ALLOW_DEFAULT_BEARER=1` et le garde-fou CSP HTTPS uniquement (DOCS-1b) sauf si vous passez `DOCS_SECURITY_ALLOW_INSECURE=1` en local. |
| `DOCS_OAUTH_*` | Active le flux device OAuth (`OAuthDeviceLogin` component) afin que les relecteurs puissent émettre des jetons de courte durée sans quitter le portail. |

Lorsque les variables OAuth sont présentes, le sandbox affiche un bouton **Sign in with device code** qui passe par le serveur Auth configuré (voir `config/security-helpers.js` pour le format exact). Les jetons émis via le flux device ne sont mis en cache que dans la session du navigateur.

## Envoyer des payloads Norito-RPC

1. Construisez un payload `.norito` avec le CLI ou les extraits décrits dans le [quickstart Norito](./quickstart.md). Le proxy transfère les corps `application/x-norito` sans modification, vous pouvez donc réutiliser le même artefact que vous enverriez avec `curl`.
2. Ouvrez `/reference/torii-rapidoc` (préféré pour les payloads binaires) ou `/reference/torii-swagger`.
3. Sélectionnez le snapshot Torii souhaité dans la liste déroulante. Les snapshots sont signés ; le panneau affiche le digest du manifeste enregistré dans `static/openapi/manifest.json`.
4. Choisissez le type de contenu `application/x-norito` dans le tiroir "Try it", cliquez sur **Choose File**, et sélectionnez votre payload. Le proxy réécrit la requête vers `/proxy/v1/pipeline/submit` et la tague avec `X-TryIt-Client=docs-portal-rapidoc`.
5. Pour télécharger les réponses Norito, définissez `Accept: application/x-norito`. Swagger/RapiDoc exposent le sélecteur d'en-tête dans le même tiroir et renvoient le binaire via le proxy.

Pour les routes uniquement JSON, le sandbox Try it intégré est souvent plus rapide : saisissez le chemin (par exemple `/v1/accounts/ih58@wonderland/assets`), sélectionnez la méthode HTTP, collez un corps JSON si nécessaire, et cliquez sur **Send request** pour inspecter les en-têtes, la durée et les payloads en ligne.

## Dépannage

| Symptôme | Cause probable | Remédiation |
| --- | --- | --- |
| La console du navigateur affiche des erreurs CORS ou le sandbox avertit que l'URL du proxy manque. | Le proxy n'est pas en cours d'exécution ou l'origine n'est pas autorisée. | Lancez le proxy, assurez-vous que `TRYIT_PROXY_ALLOWED_ORIGINS` couvre l'hôte du portail, puis relancez `npm run start`. |
| `npm run tryit-proxy` se termine avec « digest mismatch ». | Le bundle OpenAPI de Torii a changé en amont. | Exécutez `npm run sync-openapi -- --latest` (ou `--version=<tag>`) puis réessayez. |
| Les widgets renvoient `401` ou `403`. | Jeton manquant, expiré ou scopes insuffisants. | Utilisez le flux device OAuth ou collez un bearer token valide dans le sandbox. Pour les jetons statiques, vous devez exporter `DOCS_TRYIT_ALLOW_DEFAULT_BEARER=1`. |
| `429 Too Many Requests` depuis le proxy. | La limite de débit par IP est dépassée. | Augmentez `TRYIT_PROXY_RATE_LIMIT`/`TRYIT_PROXY_RATE_WINDOW_MS` pour les environnements de confiance ou réduisez les scripts de test. Tous les refus pour rate limit incrémentent `tryit_proxy_rate_limited_total`. |
| Erreurs `502/504` avec `ERR_STRICT_ADDRESS_REQUIRED` dans les logs Torii. | Les requêtes sont relayées sans support d'analyse Norito IH58/compressé. | Confirmez que le build Torii cible inclut les changements ADDR-5 (voir `crates/iroha_torii/tests/address_parsing.rs`) et que vous pointez vers le bon environnement. |

## Observabilité

- `npm run probe:tryit-proxy` (wrapper autour de `scripts/tryit-proxy-probe.mjs`) appelle `/healthz`, exécute éventuellement une route d'exemple, et émet des fichiers texte Prometheus pour `probe_success` / `probe_duration_seconds`. Configurez `TRYIT_PROXY_PROBE_METRICS_FILE` pour l'intégrer à node_exporter.
- Réglez `TRYIT_PROXY_METRICS_LISTEN=127.0.0.1:9798` pour exposer des compteurs (`tryit_proxy_requests_total`, `tryit_proxy_rate_limited_total`, `tryit_proxy_upstream_failures_total`) et des histogrammes de latence. Le tableau de bord `dashboards/grafana/docs_portal.json` lit ces métriques pour appliquer les SLOs DOCS-SORA.
- Les logs d'exécution se trouvent sur stdout. Chaque entrée inclut l'id de requête, le statut upstream, la source d'authentification (`default`, `override` ou `client`), et la durée ; les secrets sont caviardés avant émission.

Si vous devez valider que les payloads `application/x-norito` atteignent Torii sans modification, exécutez la suite Jest (`npm test -- tryit-proxy`) ou inspectez les fixtures dans `docs/portal/scripts/__tests__/tryit-proxy.test.mjs`. Les tests de régression couvrent les binaires Norito compressés, les manifestes OpenAPI signés et les chemins de rétrogradation du proxy afin que les déploiements NRPC conservent une piste d'audit permanente.
