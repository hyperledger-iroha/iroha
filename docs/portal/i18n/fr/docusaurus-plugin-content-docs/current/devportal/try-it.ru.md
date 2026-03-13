---
lang: fr
direction: ltr
source: docs/portal/docs/devportal/try-it.ru.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# Песочница Essayez-le

Le portail du robot inclut la console optionnelle « Essayez-le », pour que vous puissiez sélectionner les points de terminaison Torii, sans aucune documentation. Le proxy propose des proxys officiels, des routeurs qui gèrent CORS, qui prennent généralement en compte les limites de taux et l'authentification.

## Предварительные условия

- Node.js 18.18 ou nouveau (portail de build optimisé)
- Сетевой доступ к staging окружению Torii
- jeton du porteur, qui peut être utilisé dans les marchés Torii

Le proxy est activé dès l'ouverture temporaire. Dans le tableau, il y a de nouveaux boutons différents :| Variables | Назначение | Par défaut |
| --- | --- | --- |
| `TRYIT_PROXY_TARGET` | Basez l'URL Torii, votre proxy est disponible | **Obligatoire** |
| `TRYIT_PROXY_LISTEN` | Adresse de livraison pour les ordinateurs locaux (format `host:port` ou `[ipv6]:port`) | `127.0.0.1:8787` |
| `TRYIT_PROXY_ALLOWED_ORIGINS` | Список origin-ов, которым разрешено обращаться к proxy (через запятую) | `http://localhost:3000` |
| `TRYIT_PROXY_CLIENT_ID` | Identificateur ajouté au `X-TryIt-Client` pour le passage en amont | `docs-portal` |
| `TRYIT_PROXY_BEARER` | Jeton de porteur pour l'échange, par Torii | _vide_ |
| `TRYIT_PROXY_ALLOW_CLIENT_AUTH` | Supprimer le jeton disponible ici `X-TryIt-Auth` | `0` |
| `TRYIT_PROXY_MAX_BODY` | Taille maximale du temps de chargement (octets) | `1048576` |
| `TRYIT_PROXY_TIMEOUT_MS` | Délai d'attente en amont dans plusieurs millions | `10000` |
| `TRYIT_PROXY_RATE_LIMIT` | Propositions de limitation de débit pour les clients IP | `60` |
| `TRYIT_PROXY_RATE_WINDOW_MS` | Скользящее окно limitation de débit (ms) | `60000` |
| `TRYIT_PROXY_METRICS_LISTEN` | Adresse de livraison pour la mesure Prometheus (`host:port` ou `[ipv6]:port`) | _vide (désactivé)_ |
| `TRYIT_PROXY_METRICS_PATH` | HTTP pour la mesure du point de terminaison | `/metrics` |

Le proxy expose `GET /healthz`, optimise la structure des fichiers JSON et réédite les jetons du porteur dans les journaux.Si vous utilisez le proxy `TRYIT_PROXY_ALLOW_CLIENT_AUTH=1`, vous pouvez utiliser des documents de support, les panneaux Swagger et RapiDoc peuvent utiliser des jetons de porteur, selon vos préférences. Le proxy permet de définir les limites de taux, de corriger les crédits et de supprimer, en utilisant le jeton de remplacement pour la suppression ou le remplacement des transactions. Installez `TRYIT_PROXY_CLIENT_ID` avec votre entreprise pour que vous puissiez l'utiliser comme `X-TryIt-Client`.
(pour la référence `docs-portal`). Le proxy détecte et valide `X-TryIt-Client` du client et le transmet par défaut, les passerelles de transfert peuvent auditionner la provenance sans correction. метаданными браузера.

## Ouvrir le proxy localement

Установите зависимости при первой настройке портала:

```bash
cd docs/portal
npm install
```

Installez le proxy et sélectionnez votre instance Torii :

```bash
export TRYIT_PROXY_TARGET="https://torii.devnet.sora.example"
export TRYIT_PROXY_ALLOWED_ORIGINS="http://localhost:3000"
# Optional: preset a bearer token for the Swagger / RapiDoc panels
export TRYIT_PROXY_BEARER="Bearer eyJhbGciOi..."
npm run tryit-proxy
```

Le script enregistre la liaison d'adresse et effectue des transactions avec `/proxy/*` sur l'origine Torii.

Avant la confirmation du script de liaison, c'est
`static/openapi/torii.json` соответствует digest, записанному в
`static/openapi/manifest.json`. Si c'est le cas, la commande s'occupera de vous et précédera le `npm run sync-openapi -- --latest`. Exporter
`TRYIT_PROXY_ALLOW_STALE_SPEC=1` uniquement pour les appareils ménagers ; proxy prévoit la pré-préparation et le travail du robot, ce que vous pouvez faire pendant la fenêtre de maintenance.

## Ajout de vidéos sur le portail

Lors de la construction ou du service d'un portail de démarrage, vous devez utiliser une URL pour le proxy :

```bash
export TRYIT_PROXY_PUBLIC_URL="http://localhost:8787"
export TRYIT_PROXY_DEFAULT_BEARER="Bearer eyJhbGciOi..." # Optional
npm run start
```Les composants suivants sont indiqués pour `docusaurus.config.js` :

- **Swagger UI** - utilisé pour `/reference/torii-swagger` ; pré-autorise le système au porteur pour un jeton, en utilisant le proxy `X-TryIt-Client`, en injectant `X-TryIt-Auth` et en vous permettant d'obtenir un proxy, comme `TRYIT_PROXY_PUBLIC_URL` задан.
- **RapiDoc** - отображается на `/reference/torii-rapidoc` ; En utilisant un jeton, vous pouvez utiliser les en-têtes Swagger et créer automatiquement un proxy pour l'URL.
- **Essayez-le sur la console** - встроен в страницу обзора API ; Vous pouvez ouvrir les commandes de distribution, configurer les en-têtes et inspecter les appels d'offres.

Sur les panneaux, vous trouverez **sélecteur d'instantanés**, qui apparaît
`docs/portal/static/openapi/versions.json`. Заполните индекс командой
`npm run sync-openapi -- --version=<label> --mirror=current --latest`, les évaluateurs ont découvert les spécifications de l'histoire, ont téléchargé le résumé SHA-256 et ont publié un instantané signé par le manifeste. avant d'utiliser des vidéos interactives.

Le jeton dans l'espace vidéo se trouve juste sur le clavier de votre session ; le proxy n'est pas connecté et n'enregistre pas le jeton précédent.

## Jetons OAuth gratuits

Si vous ne pouvez pas utiliser les jetons Torii auprès des évaluateurs, veuillez cliquer sur la console Essayez-le avec le serveur OAuth. Lors de l'ouverture permanente de votre compte, le portail ouvre un widget de code d'appareil, vous permet de récupérer les jetons du porteur et de les envoyer automatiquement. sous forme de console.| Variables | Назначение | Par défaut |
| --- | --- | --- |
| `DOCS_OAUTH_DEVICE_CODE_URL` | Point de terminaison d’autorisation de périphérique OAuth (`/oauth/device/code`) | _vide (désactivé)_ |
| `DOCS_OAUTH_TOKEN_URL` | Point de terminaison de jeton, par `grant_type=urn:ietf:params:oauth:grant-type:device_code` | _vide_ |
| `DOCS_OAUTH_CLIENT_ID` | ID client OAuth, enregistré pour l'aperçu de la documentation | _vide_ |
| `DOCS_OAUTH_SCOPE` | Scopes, analyses approfondies, à effectuer pour chaque année | `openid profile offline_access` |
| `DOCS_OAUTH_AUDIENCE` | Audience API personnalisée pour les jetons token | _vide_ |
| `DOCS_OAUTH_POLL_INTERVAL_MS` | Intervalle d'interrogation minimal avant la réponse (ms) | `5000` (sortie < 5 000 ms) |
| `DOCS_OAUTH_DEVICE_CODE_TTL_SECONDS` | Code de périphérique TTL (secondes) | `600` (peut durer jusqu'à 300 s et 900 s) |
| `DOCS_OAUTH_TOKEN_TTL_SECONDS` | Jeton d'accès TTL (secondes) | `900` (peut durer jusqu'à 300 s et 900 s) |
| `DOCS_OAUTH_ALLOW_INSECURE` | Installez `1` pour l'aperçu local, qui est nommé pour OAuth | _unset_ |

Exemple de configuration :

```bash
export DOCS_OAUTH_DEVICE_CODE_URL="https://auth.dev.sora.example/oauth/device/code"
export DOCS_OAUTH_TOKEN_URL="https://auth.dev.sora.example/oauth/token"
export DOCS_OAUTH_CLIENT_ID="docs-preview"
export DOCS_OAUTH_SCOPE="torii openid offline_access"
# Optional audience and polling tweaks
export DOCS_OAUTH_AUDIENCE="https://torii.devnet.sora.example"
export DOCS_OAUTH_POLL_INTERVAL_MS="6000"
```

Lorsque vous achetez `npm run start` ou `npm run build`, le portail vous permet de télécharger cette page sur `docusaurus.config.js`. Во время локального aperçu de la carte Essayez-le en cliquant sur le bouton "Connectez-vous avec le code de l'appareil". Vous pouvez trouver le code pour votre zone de vérification OAuth ; Après la visualisation du flux de l'appareil :- вставляет выданный Bearer Token в поле консоли Essayez-le,
- ajouter les en-têtes `X-TryIt-Client` et `X-TryIt-Auth`,
- отображает оставшееся время жизни,
- Le jeton est automatiquement retiré lors de l'installation.

Le portail Bearer vous propose la livraison - utilisez OAuth de façon permanente, si vous acceptez d'envoyer des évaluateurs lors de l'achat d'un jeton actuel ou d'une exportation. `DOCS_OAUTH_ALLOW_INSECURE=1` pour l'aperçu local isolé, puis le téléchargement anonyme. Builds без настроенного OAuth теперь быстро падают, чтобы удовлетворить la porte DOCS-1b.

Remarque : Avant la publication du portail des laboratoires précédents, vérifiez [Liste de contrôle de renforcement de la sécurité et de test d'intrusion] (./security-hardening.md) ; Dans notre étude du modèle de menace, du profil CSP/Trusted Types et des tests d'intrusion, il s'agit de la porte DOCS-1b.

## Exemples Norito-RPC

Norito-RPC utilise le proxy et la plomberie OAuth, ainsi que les routes JSON ; Je vais maintenant installer la charge utile `Content-Type: application/x-norito` et utiliser la charge utile Norito, selon les spécifications du NRPC. (`docs/source/torii/nrpc_spec.md`). Le dépôt contient des charges utiles canoniques selon `fixtures/norito_rpc/`, des auteurs de portail, des propriétaires et des réviseurs de SDK qui utilisent tous les octets, en utilisant CI.

### Déploiement de la charge utile Norito depuis la console Try It1. Sélectionnez le luminaire, par exemple `fixtures/norito_rpc/transfer_asset.norito`. Эти файлы являются сырыми Enveloppes Norito ; **Non** codé en base64.
2. Dans Swagger ou RapiDoc, sélectionnez le point de terminaison NRPC (par exemple, `POST /v2/pipeline/submit`) et sélectionnez le sélecteur **Content-Type** sur `application/x-norito`.
3. Sélectionnez le rédacteur en chef dans **binaire** (par exemple "Fichier" dans Swagger ou "Binaire/Fichier" dans RapiDoc) et tapez `.norito`. Vous pouvez supprimer les octets du proxy sans le modifier.
4. Ouvrir la session. Si Torii remplace `X-Iroha-Error-Code: schema_mismatch`, assurez-vous de sélectionner le point de terminaison, les charges utiles binaires principales et de modifier le hachage de schéma dans `fixtures/norito_rpc/schema_hashes.json` est compatible avec la build Torii.

Après avoir installé la console dans les paramètres, vous pouvez facilement activer la charge utile des jetons d'autorisation ou des hôtes Torii. La création de `scripts/run_norito_rpc_fixtures.sh --note "<ticket>"` dans le flux de travail concerne l'ensemble de preuves, effectuée dans le plan NRPC-4 (journal + résumé JSON), qui correspond à l'écran d'accueil. Essayez-le avec les critiques.

### CLI пример (boucle)

Ces appareils peuvent être programmés sur le port `curl`, ce qui permet de vérifier le proxy ou de lancer la passerelle :

```bash
TORII="https://torii.devnet.sora.example"
TOKEN="Bearer $(cat ~/.config/torii/devnet.token)"
curl   -H "Content-Type: application/x-norito"   -H "Authorization: ${TOKEN}"   --data-binary @fixtures/norito_rpc/transfer_asset.norito   "${TORII}/v2/pipeline/submit"
```Placez le luminaire sur l'entrée principale `transaction_fixtures.manifest.json` ou sélectionnez votre commande de charge utile `cargo xtask norito-rpc-fixtures`. Lorsque Torii fonctionne avec le système Canary, vous pouvez configurer `curl` sur un proxy d'essai (`https://docs.sora.example/proxy/v2/pipeline/submit`), afin de vérifier votre état. l'infrastructure qui vous permet de visualiser et de visualiser le portail.

## Observabilité et opérations

Les informations relatives à la méthode, au chemin, à l'origine, à l'état en amont et à l'authentification d'origine (`override`, `default` ou `client`) sont enregistrées. Les jetons ne sont pas connectés - les en-têtes du porteur et la spécification `X-TryIt-Auth` sont rédigés avant la logistique, il est possible d'utiliser la sortie standard dans le collecteur central Sans risque pour les secrets.

### Sondes de santé et alertes

Ouvrir la sonde interne lors du déploiement ou de l'installation :

```bash
# Ensure the proxy responds to /healthz and forwards a sample request.
TRYIT_PROXY_PUBLIC_URL="https://docs.sora.example/proxy" TRYIT_PROXY_SAMPLE_PATH="/v2/status" npm run probe:tryit-proxy
```

Fonctionnement des boutons :- `TRYIT_PROXY_SAMPLE_PATH` - Torii optionnel (sans `/proxy`) pour les vérifications.
- `TRYIT_PROXY_SAMPLE_METHOD` - pour remplacer `GET` ; задайте `POST` для écrire des marшрутов.
- `TRYIT_PROXY_PROBE_TOKEN` - добавляет временный porteur jeton к échantillon вызову.
- `TRYIT_PROXY_PROBE_TIMEOUT_MS` - dépasse le délai d'attente de 5 s.
- `TRYIT_PROXY_PROBE_METRICS_FILE` - Fichier texte Prometheus optionnel pour `probe_success`/`probe_duration_seconds`.
- `TRYIT_PROXY_PROBE_LABELS` - pour `key=value`, ajoutez des mesures (en utilisant `job=tryit-proxy` et `instance=<proxy URL>`).
- `TRYIT_PROXY_PROBE_METRICS_URL` - Point de terminaison de métriques d'URL optionnel (par exemple `http://localhost:9798/metrics`), que vous devez utiliser pour utiliser `TRYIT_PROXY_METRICS_LISTEN`.

Sélectionnez les résultats dans le collecteur de fichiers texte, en sélectionnant la sonde sur le chemin inscriptible (par exemple, `/var/lib/node_exporter/textfile_collector/tryit.prom`) et en créant des étiquettes de fichier :

```bash
TRYIT_PROXY_PUBLIC_URL="https://docs.sora.example/proxy" TRYIT_PROXY_PROBE_METRICS_FILE="/var/lib/node_exporter/textfile_collector/tryit.prom" TRYIT_PROXY_PROBE_LABELS="job=tryit-proxy,cluster=prod" npm run probe:tryit-proxy
```

Le script fournit une mesure automatique du collecteur, qui contient la charge utile la plus élevée.

Si vous utilisez `TRYIT_PROXY_METRICS_LISTEN`, indiquez-le
`TRYIT_PROXY_PROBE_METRICS_URL` sur le point de terminaison de métriques, que vous sondez en cas de problème lors de la propagation du scrape (par exemple, aucune entrée ou aucune règle de pare-feu). Type de production :
`TRYIT_PROXY_PROBE_METRICS_URL="http://127.0.0.1:9798/metrics"`.

Pour effectuer une alerte légère, connectez la sonde à votre pile de surveillance. Par exemple, Prometheus, cela se produit après deux étapes suivantes :

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

### Point de terminaison des métriques et tableaux de bordInstallez `TRYIT_PROXY_METRICS_LISTEN=127.0.0.1:9798` (ou votre hôte/port) avant d'installer un proxy pour ouvrir le point de terminaison de métriques au format Prometheus. Si vous utilisez `/metrics`, vous ne pouvez pas utiliser `TRYIT_PROXY_METRICS_PATH=/custom`. Le scrape prend en compte les méthodes, les rejets de limite de débit, les erreurs/délais d'attente en amont, les résultats du proxy et la latence récapitulative :

```bash
export TRYIT_PROXY_METRICS_LISTEN="127.0.0.1:9798"
npm run tryit-proxy &
curl http://127.0.0.1:9798/metrics | head -n 5
# HELP tryit_proxy_requests_total Requests handled by method
tryit_proxy_requests_total{method="GET"} 12
tryit_proxy_rate_limited_total 1
```

Configurez les collecteurs Prometheus/OTLP sur le point de terminaison de métriques et activez les panneaux `dashboards/grafana/docs_portal.json`, ce qui permet à SRE de désactiver la latence de queue et de résoudre les problèmes suivants парсинга logoв. Le proxy automatique publie `tryit_proxy_start_timestamp_ms`, lorsque l'opérateur effectue un redémarrage.

### Restauration automatique

Utilisez un assistant de gestion pour la mise à jour ou la gestion de l'URL Torii. Le script a été configuré avant la configuration dans `.env.tryit-proxy.bak`, et cette restauration est donc une commande.

```bash
# Update TRYIT_PROXY_TARGET and back up the previous config.
npm run manage:tryit-proxy -- update --target https://torii.devnet.sora.example

# Roll back to the previously backed-up target.
npm run manage:tryit-proxy -- rollback
```

Il est préférable de mettre en place le fichier d'environnement `--env` ou `TRYIT_PROXY_ENV`, sinon vous pourrez modifier la configuration de votre ordinateur.