---
lang: fr
direction: ltr
source: docs/portal/docs/devportal/try-it.es.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# Sandbox de Essayez-le

Le portail des développeurs comprend une console facultative « Try it » pour que vous puissiez appeler les points de terminaison de Torii sans accéder à la documentation. La console retransmite sollicite le passage du proxy, notamment pour que les navigateurs puissent éviter les limites CORS en suivant les limites de charge et d'authentification.

## Prérequis

- Node.js 18.18 ou plus nouveau (coïncide avec les exigences de construction du portail)
- Accès de rouge à un entorno de staging de Torii
- Un jeton de porteur qui peut appeler les routes Torii que les avions éjercitar

Aujourd'hui, la configuration du proxy réalise des variables intermédiaires. Le tableau suivant liste les boutons les plus importants :| Variables | Proposé | Par défaut |
| --- | --- | --- |
| `TRYIT_PROXY_TARGET` | URL base de Torii pour que le proxy revienne aux sollicitudes | **Obligatoire** |
| `TRYIT_PROXY_LISTEN` | Direction de l'écoute pour le téléchargement local (format `host:port` ou `[ipv6]:port`) | `127.0.0.1:8787` |
| `TRYIT_PROXY_ALLOWED_ORIGINS` | Liste séparée par comas d'origine que vous pouvez appeler au proxy | `http://localhost:3000` |
| `TRYIT_PROXY_CLIENT_ID` | Identifiant placé en `X-TryIt-Client` pour chaque demande en amont | `docs-portal` |
| `TRYIT_PROXY_BEARER` | Jeton du porteur par défaut renvoyé à Torii | _vide_ |
| `TRYIT_PROXY_ALLOW_CLIENT_AUTH` | Permettre aux utilisateurs de fournir leur jeton propio via `X-TryIt-Auth` | `0` |
| `TRYIT_PROXY_MAX_BODY` | Tamano maximo del corps de sollicitude (octets) | `1048576` |
| `TRYIT_PROXY_TIMEOUT_MS` | Timeout en amont et milisegundos | `10000` |
| `TRYIT_PROXY_RATE_LIMIT` | Sollicitudes permises pour une vente de bureau par IP de client | `60` |
| `TRYIT_PROXY_RATE_WINDOW_MS` | Ventana deslizante para rate limiting (ms) | `60000` |
| `TRYIT_PROXY_METRICS_LISTEN` | Direction de sauvegarde facultative pour le point final de métrique style Prometheus (`host:port` ou `[ipv6]:port`) | _vide (désactivé)_ |
| `TRYIT_PROXY_METRICS_PATH` | Route HTTP servie par le point de terminaison de métriques | `/metrics` |

Le proxy expose également `GET /healthz`, créant des erreurs JSON structurées et occultant les jetons du porteur de la sortie de journaux.Active `TRYIT_PROXY_ALLOW_CLIENT_AUTH=1` pour exposer le proxy aux utilisateurs de documents afin que les panneaux Swagger et RapiDoc puissent réenvoyer des jetons de porteur fournis par l'utilisateur. Le proxy a une application limitée sur la charge, les informations d'identification et l'enregistrement occultes si une demande d'utilisation du jeton par défaut ou une annulation de demande. Configurez `TRYIT_PROXY_CLIENT_ID` avec l'étiquette que vous souhaitez envoyer comme `X-TryIt-Client`
(par défaut `docs-portal`). Le proxy enregistré et validé les valeurs `X-TryIt-Client` portés par le client, est défini par défaut pour que les passerelles de préparation puissent vérifier la procédure sans correspondance des métadonnées du navigateur.

## Démarrer le proxy localement

Installez les dépendances la première fois que vous configurez le portail :

```bash
cd docs/portal
npm install
```

Exécutez le proxy et installez-le sur votre instance Torii :

```bash
export TRYIT_PROXY_TARGET="https://torii.devnet.sora.example"
export TRYIT_PROXY_ALLOWED_ORIGINS="http://localhost:3000"
# Optional: preset a bearer token for the Swagger / RapiDoc panels
export TRYIT_PROXY_BEARER="Bearer eyJhbGciOi..."
npm run tryit-proxy
```

Le script enregistre la direction enlazada et renvoie les sollicitudes depuis `/proxy/*` à l'origine Torii configuré.

Avant d'activer le socket et le script validé
`static/openapi/torii.json` coïncide avec le résumé enregistré en
`static/openapi/manifest.json`. Si les archives sont supprimées, la commande se termine avec un
erreur et vous indiquez exécuter `npm run sync-openapi -- --latest`. Exportation
`TRYIT_PROXY_ALLOW_STALE_SPEC=1` uniquement pour les remplacements d'urgence ; le proxy s'enregistre un
advertencia y continuara para que puedas recuperarte pendante ventanas de mantenimiento.

## Connectez les widgets du portailLorsque vous créez le portail des développeurs, définissez l'URL des widgets
devez utiliser le proxy :

```bash
export TRYIT_PROXY_PUBLIC_URL="http://localhost:8787"
export TRYIT_PROXY_DEFAULT_BEARER="Bearer eyJhbGciOi..." # Optional
npm run start
```

Les composants suivants ont ces valeurs depuis `docusaurus.config.js` :

- **Swagger UI** - rendu sur `/reference/torii-swagger` ; préautoriser l'esquema
  porteur cuando hay un jeton, étiquette las sollicitudes avec `X-TryIt-Client`,
  inyecta `X-TryIt-Auth`, et réécrire les llamadas a traves del proxy cuando
  `TRYIT_PROXY_PUBLIC_URL` est défini.
- **RapiDoc** - rendu sur `/reference/torii-rapidoc` ; refleja el campo de token,
  réutiliser les mêmes en-têtes que le panneau Swagger et apunta al proxy
  automatiquement lorsque l'URL est configurée.
- **Try it console** - intégré à la page de présentation de l'API ; permettre d'envoyer
  sollicitudes personnalisées, consulter les en-têtes et inspecter les corps de réponse.

Les panneaux Ambos peuvent afficher un **sélecteur d'instantanés** que vous pouvez lire
`docs/portal/static/openapi/versions.json`. Llena est cet indice con
`npm run sync-openapi -- --version=<label> --mirror=current --latest` pour les réviseurs
vous pouvez entrer les spécifications historiques, voir le résumé SHA-256 enregistré, et confirmer si un
snapshot de release est un manifeste ferme avant d'utiliser les widgets interactifs.

Changer le jeton dans n'importe quel widget n'affecte que la session actuelle du navigateur ; le proxy nunca
persiste ni enregistrer le jeton fourni.

## Tokens OAuth de courte duréePour éviter de distribuer des jetons Torii de longue durée aux évaluateurs, connectez-vous à la console Essayez-le et
votre serveur OAuth. Lorsque les variables d'entrée en bas sont présentées, le portail rend un
widget de connexion avec le code de l'appareil, génère des jetons de porteur de vie et les injections automatiquement
dans le formulaire de la console.

| Variables | Proposé | Par défaut |
| --- | --- | --- |
| `DOCS_OAUTH_DEVICE_CODE_URL` | Point de terminaison d'autorisation du périphérique OAuth (`/oauth/device/code`) | _vide (désactivé)_ |
| `DOCS_OAUTH_TOKEN_URL` | Point de terminaison du jeton acceptant `grant_type=urn:ietf:params:oauth:grant-type:device_code` | _vide_ |
| `DOCS_OAUTH_CLIENT_ID` | Identifiant du client OAuth enregistré pour l'aperçu des documents | _vide_ |
| `DOCS_OAUTH_SCOPE` | Portées séparées par les espaces sollicités lors de la connexion | `openid profile offline_access` |
| `DOCS_OAUTH_AUDIENCE` | Audience de l'API facultative pour identifier le jeton | _vide_ |
| `DOCS_OAUTH_POLL_INTERVAL_MS` | Intervalle minimum de sondage pendant l'attente de l'approbation (ms) | `5000` (valeurs < 5000 ms se rechazan) |
| `DOCS_OAUTH_DEVICE_CODE_TTL_SECONDS` | Ventana d'expiration du code de l'appareil (secondes) | `600` (doit être entretenu entre 300 s et 900 s) |
| `DOCS_OAUTH_TOKEN_TTL_SECONDS` | Durée du jeton d'accès (seconde) | `900` (doit être entretenu entre 300 s et 900 s) |
| `DOCS_OAUTH_ALLOW_INSECURE` | Pon `1` pour les aperçus locaux qui omettent intentionnellement l'application d'OAuth | _unset_ |

Exemple de configuration :

```bash
export DOCS_OAUTH_DEVICE_CODE_URL="https://auth.dev.sora.example/oauth/device/code"
export DOCS_OAUTH_TOKEN_URL="https://auth.dev.sora.example/oauth/token"
export DOCS_OAUTH_CLIENT_ID="docs-preview"
export DOCS_OAUTH_SCOPE="torii openid offline_access"
# Optional audience and polling tweaks
export DOCS_OAUTH_AUDIENCE="https://torii.devnet.sora.example"
export DOCS_OAUTH_POLL_INTERVAL_MS="6000"
```Lorsque vous exécutez `npm run start` ou `npm run build`, le portail incruste ces valeurs
en `docusaurus.config.js`. Durante un aperçu local de la tarjeta Try it muestra un
bouton "Connectez-vous avec le code de l'appareil". Les utilisateurs intègrent le code affiché sur votre page de vérification OAuth ; Une fois que le flux de l'appareil a quitté le widget :

- inyecta le jeton du porteur émis dans le camp de la console Essayez-le,
- étiquette des sollicitudes avec les en-têtes existants `X-TryIt-Client` et `X-TryIt-Auth`,
- montre le temps de vie restante, y
- Borra automatiquement le jeton lorsqu'il expire.

La entrée manuelle du porteur est disponible ; omettre les variables OAuth lorsque vous voulez
forcer les critiques à obtenir un jeton temporel pour eux, ou exporter
`DOCS_OAUTH_ALLOW_INSECURE=1` pour les aperçus locaux faciles où l'accès est anonyme
acceptable. Les builds sans OAuth configurés maintenant tombent rapidement pour satisfaire la porte
de la feuille de route DOCS-1b.

Remarque : Révision de la [Liste de contrôle de renforcement de la sécurité et de test d'intrusion](./security-hardening.md)
avant d'exposer le portail hors du laboratoire ; documenta le modèle de menace,
le profil CSP/Trusted Types, et les étapes de pen-test qui bloquent désormais DOCS-1b.

## Musiques Norito-RPCLes demandes Norito-RPC partagent le même proxy et plomberie OAuth que les routes JSON ;
Configurez `Content-Type: application/x-norito` et envoyez simplement la charge utile Norito.
pré-encodé décrit dans la spécification NRPC
(`docs/source/torii/nrpc_spec.md`).
Le référentiel comprend des charges utiles canoniques sous `fixtures/norito_rpc/` pour les auteurs du portail,
les propriétaires du SDK et les réviseurs peuvent reproduire exactement les octets utilisés par CI.

### Envoyer une charge utile Norito depuis la console Essayez-le

1. Choisissez un luminaire comme `fixtures/norito_rpc/transfer_asset.norito`. Estos
   archivos fils enveloppes Norito en bruto; **non** les codificateurs en base64.
2. Dans Swagger ou RapiDoc, localisez le point final NRPC (par exemple
   `POST /v1/pipeline/submit`) et changer le sélecteur **Content-Type** a
   `application/x-norito`.
3. Changer l'éditeur du corps de demande en **binaire** (mode "Fichier" de Swagger ou
   sélecteur "Binary/File" de RapiDoc) et chargez l'archive `.norito`. Le widget
   transmettre les octets à travers le proxy sans les modifier.
4. Envoyez la sollicitude. Si Torii développe `X-Iroha-Error-Code: schema_mismatch`,
   vérifiez que vous appelez un point de terminaison qui accepte les binaires de charges utiles et confirmez
   que le hachage de schéma est enregistré dans `fixtures/norito_rpc/schema_hashes.json`
   coïncide avec la construction de Torii que vous utilisez.La console conserve les archives plus récentes en mémoire pour que vous puissiez réactualiser le tout
La charge utile permet d'évaluer différents jetons d'autorisation ou hôtes de Torii. Agréger
`scripts/run_norito_rpc_fixtures.sh --note "<ticket>"` pour que votre workflow produise le bundle de
preuve référencée dans le plan d'adoption NRPC-4 (log + reprise JSON), quelle combinaison
bien avec capturer des captures d'écran de la réponse Try It lors des révisions.

### Exemple CLI (curl)

Les autres appareils peuvent être reproduits à l'extérieur du portail via `curl`, ce qui est utile
Lorsque vous validez le proxy ou les réponses de la passerelle :

```bash
TORII="https://torii.devnet.sora.example"
TOKEN="Bearer $(cat ~/.config/torii/devnet.token)"
curl   -H "Content-Type: application/x-norito"   -H "Authorization: ${TOKEN}"   --data-binary @fixtures/norito_rpc/transfer_asset.norito   "${TORII}/v1/pipeline/submit"
```

Changer le luminaire par n'importe quelle entrée répertoriée en `transaction_fixtures.manifest.json`
ou codifiez votre propre charge utile avec `cargo xtask norito-rpc-fixtures`. Lorsque Torii est là
modo Canary peut identifier `curl` au proxy essayer
(`https://docs.sora.example/proxy/v1/pipeline/submit`) pour extraire la même chose
infrastructure qui utilise les widgets du portail.

## Observabilité et fonctionnement

Chaque personne vous demande d'enregistrer une fois avec sa méthode, son chemin, son origine, son état en amont et sa source.
d'authentification (`override`, `default` ou `client`). Los tokens nunca se almacenan: tanto
los headers Bearer como los valores `X-TryIt-Auth` se redaccionan antes de registrar,
donc vous pouvez renvoyer la sortie standard vers un collecteur central sans vous soucier des filtrations.

### Sondes de santé et alertes

Exécutez la sonde incluse lors des opérations ou selon un horaire :

```bash
# Ensure the proxy responds to /healthz and forwards a sample request.
TRYIT_PROXY_PUBLIC_URL="https://docs.sora.example/proxy" TRYIT_PROXY_SAMPLE_PATH="/v1/status" npm run probe:tryit-proxy
```Boutons d'entrée :

- `TRYIT_PROXY_SAMPLE_PATH` - itinéraire Torii facultatif (sin `/proxy`) pour l'exercice.
- `TRYIT_PROXY_SAMPLE_METHOD` - par défaut `GET` ; définir `POST` pour les routes d'écriture.
- `TRYIT_PROXY_PROBE_TOKEN` - injecte un jeton de porteur temporel pour l'appel de la musique.
- `TRYIT_PROXY_PROBE_TIMEOUT_MS` - indique le délai d'attente par défaut de 5 s.
- `TRYIT_PROXY_PROBE_METRICS_FILE` - destination facultative du texte Prometheus pour `probe_success`/`probe_duration_seconds`.
- `TRYIT_PROXY_PROBE_LABELS` - pare `key=value` séparé par comas anexados a las metricas (par défaut `job=tryit-proxy` et `instance=<proxy URL>`).
- `TRYIT_PROXY_PROBE_METRICS_URL` - URL facultative du point de terminaison de métriques (par exemple, `http://localhost:9798/metrics`) qui doit répondre à la sortie lorsque `TRYIT_PROXY_METRICS_LISTEN` est habilité.

Alimentez les résultats dans un collecteur de fichiers texte indiquant la sonde dans une route écrivable
(par exemple, `/var/lib/node_exporter/textfile_collector/tryit.prom`) et ajouter des étiquettes
personnalisé:

```bash
TRYIT_PROXY_PUBLIC_URL="https://docs.sora.example/proxy" TRYIT_PROXY_PROBE_METRICS_FILE="/var/lib/node_exporter/textfile_collector/tryit.prom" TRYIT_PROXY_PROBE_LABELS="job=tryit-proxy,cluster=prod" npm run probe:tryit-proxy
```

Le script réécrit le fichier de métriques de forme atomique pour que votre collectionneur continue à le faire
une charge utile complète.

Lorsque `TRYIT_PROXY_METRICS_LISTEN` est configuré, définissez
`TRYIT_PROXY_PROBE_METRICS_URL` au point final de métriques pour que la sonde tombe rapidement si la
la surface de scrape disparaît (par exemple, entrée mal configurée ou règlement du pare-feu défectueux).
Un ajustement typique de la production
`TRYIT_PROXY_PROBE_METRICS_URL="http://127.0.0.1:9798/metrics"`.Pour alerter les habitants, connectez la sonde à votre pile de moniteur. Exemple de Prometheus que
page après les deux erreurs consécutives :

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

### Endpoint de métriques et tableaux de bord

Configurez `TRYIT_PROXY_METRICS_LISTEN=127.0.0.1:9798` (ou n'importe quel hôte/puerto) avant de
lancez le proxy pour exposer un point de terminaison de métriques au format Prometheus. La route
Par défaut, c'est `/metrics` mais vous pouvez changer avec `TRYIT_PROXY_METRICS_PATH=/custom`. Cada
scrape devuelve contadores de totals por metodo, rechazos por rate limit, erreurs/timeouts
en amont, résultats du proxy et résumés de latence :

```bash
export TRYIT_PROXY_METRICS_LISTEN="127.0.0.1:9798"
npm run tryit-proxy &
curl http://127.0.0.1:9798/metrics | head -n 5
# HELP tryit_proxy_requests_total Requests handled by method
tryit_proxy_requests_total{method="GET"} 12
tryit_proxy_rate_limited_total 1
```

Appointez vos collecteurs Prometheus/OTLP au point final de mesures et réutilisez les panneaux existants
en `dashboards/grafana/docs_portal.json` pour que SRE observe les latences de cola et les picos de
rechazo sin parsear journaux. Le proxy est automatiquement publié `tryit_proxy_start_timestamp_ms`
pour aider les opérateurs à détecter les reinicios.

### Automatisation du rollback

Utilisez l'assistant de gestion pour actualiser ou restaurer l'URL objet Torii. Le script
gardez la configuration précédente en `.env.tryit-proxy.bak` pour que les restaurations se produisent
commandant solo.

```bash
# Update TRYIT_PROXY_TARGET and back up the previous config.
npm run manage:tryit-proxy -- update --target https://torii.devnet.sora.example

# Roll back to the previously backed-up target.
npm run manage:tryit-proxy -- rollback
```

Sobrescribe la route del archivo env con `--env` or `TRYIT_PROXY_ENV` si tu despliegue
gardez la configuration dans un autre endroit.