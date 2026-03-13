---
lang: fr
direction: ltr
source: docs/portal/docs/devportal/try-it.ur.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# Essayez-le maintenant

"Essayez-le" Points de terminaison Torii en cours de réalisation Le système d'exploitation des ressources humaines pour le CORS est destiné à la gestion des ressources humaines. سکیں جبکہ ریٹ لمٹس اور آتھنٹیکیشن نافذ رہیں۔

## شرائط

- Node.js 18.18 pour Windows (version build pour les utilisateurs)
- Mise en scène Torii pour votre projet
- Il s'agit d'un jeton de porteur et d'un itinéraire Torii, ainsi que d'un jeton de porteur.

Il existe plusieurs variables d'environnement et des variables d'environnement. Voici les boutons et les boutons :| Variables | مقصد | Par défaut |
| --- | --- | --- |
| `TRYIT_PROXY_TARGET` | Torii L'URL de base est celle de votre site Web | **Obligatoire** |
| `TRYIT_PROXY_LISTEN` | لوکل ڈیولپمنٹ کے لئے écouter l'adresse (`host:port` ou `[ipv6]:port`) | `127.0.0.1:8787` |
| `TRYIT_PROXY_ALLOWED_ORIGINS` | origines فہرست جو پروکسی کو کال کر سکتے ہیں | `http://localhost:3000` |
| `TRYIT_PROXY_CLIENT_ID` | Demande en amont `X-TryIt-Client` pour l'identifiant | `docs-portal` |
| `TRYIT_PROXY_BEARER` | Jeton au porteur et Torii pour le jeton de porteur | _vide_ |
| `TRYIT_PROXY_ALLOW_CLIENT_AUTH` | Jeton `X-TryIt-Auth` pour le jeton d'achat | `0` |
| `TRYIT_PROXY_MAX_BODY` | corps de la requête کا زیادہ سے زیادہ سائز (octets) | `1048576` |
| `TRYIT_PROXY_TIMEOUT_MS` | délai d'attente en amont (millisecondes) | `10000` |
| `TRYIT_PROXY_RATE_LIMIT` | L'IP du client et la fenêtre des requêtes | `60` |
| `TRYIT_PROXY_RATE_WINDOW_MS` | limitation de débit à fenêtre coulissante (ms) | `60000` |
| `TRYIT_PROXY_METRICS_LISTEN` | Prometheus point de terminaison de métriques et adresse d'écoute (`host:port` ou `[ipv6]:port`) | _vide (désactivé)_ |
| `TRYIT_PROXY_METRICS_PATH` | point de terminaison des métriques et chemin HTTP | `/metrics` |

Le `GET /healthz` contient des erreurs JSON structurées et des jetons de porteur, ainsi que la sortie du journal et la rédaction des jetons.`TRYIT_PROXY_ALLOW_CLIENT_AUTH=1` Les utilisateurs de documents exposent les jetons de porteur fournis par l'utilisateur des panneaux RapiDoc de Swagger et RapiDoc. سکیں۔ Les limites de débit sont définies comme les informations d'identification et la rédaction du jeton par défaut pour la demande de jeton par défaut. یا remplacement par demande۔ `TRYIT_PROXY_CLIENT_ID` est une étiquette pour l'étiquette et `X-TryIt-Client` est une étiquette pour le produit `TRYIT_PROXY_CLIENT_ID`.
(ڈیفالٹ `docs-portal`)۔ Définir les valeurs `X-TryIt-Client` fournies par l'appelant et ajuster et valider les métadonnées du navigateur des passerelles de transfert et corréler les métadonnées du navigateur. بغیر audit de provenance کر سکیں۔

## لوکل طور پر پروکسی چلائیں

Il y a plusieurs dépendances à prendre en compte :

```bash
cd docs/portal
npm install
```

L'instance Torii est actuellement en cours de création :

```bash
export TRYIT_PROXY_TARGET="https://torii.devnet.sora.example"
export TRYIT_PROXY_ALLOWED_ORIGINS="http://localhost:3000"
# Optional: preset a bearer token for the Swagger / RapiDoc panels
export TRYIT_PROXY_BEARER="Bearer eyJhbGciOi..."
npm run tryit-proxy
```

Adresse liée à l'origine `/proxy/*` pour les requêtes et l'origine Torii configurée pour transférer l'adresse

ساکٹ lier کرنے سے پہلے اسکرپٹ vérifier کرتا ہے کہ
`static/openapi/torii.json` pour le résumé `static/openapi/manifest.json` pour le résumé du résumé Il s'agit d'une dérive et d'une erreur d'erreur et d'une sortie de sortie pour `npm run sync-openapi -- --latest`. ہے۔ `TRYIT_PROXY_ALLOW_STALE_SPEC=1` صرف ہنگامی override کے لئے استعمال کریں؛ avertissement d'avertissement pour les fenêtres de maintenance et les fenêtres de maintenance

## پورٹل widgets کو جوڑیںIl s'agit d'un portail de développeurs et d'un portail de développement qui sert des liens et des URL pour les widgets ci-dessous :

```bash
export TRYIT_PROXY_PUBLIC_URL="http://localhost:8787"
export TRYIT_PROXY_DEFAULT_BEARER="Bearer eyJhbGciOi..." # Optional
npm run start
```

یہ composants `docusaurus.config.js` et valeurs پڑھتے ہیں :

- **Swagger UI** - `/reference/torii-swagger` pour le rendu jeton est utilisé pour le schéma au porteur et la pré-autorisation pour les requêtes `X-TryIt-Client` et la balise `X-TryIt-Auth` pour injecter le système `TRYIT_PROXY_PUBLIC_URL` سیٹ ہونے پر appelle کو پروکسی کے ذریعے réécriture کرتا ہے۔
- **RapiDoc** - `/reference/torii-rapidoc` pour le rendu ici jeton et miroir et panneau Swagger, réutilisation des en-têtes et URL configurer et proxy et cible et proxy.
- **Essayez-le sur la console** - Page de présentation de l'API avec version intégrée demandes personnalisées pour les en-têtes et les organismes de réponse inspectent les formulaires

Panneaux d'affichage ** **sélecteur d'instantanés** Panneaux d'affichage **
`docs/portal/static/openapi/versions.json` پڑھتا ہے۔ اس index کو
`npm run sync-openapi -- --version=<label> --mirror=current --latest` est un réviseurs spécifications historiques plus de détails sur le résumé SHA-256 et widgets interactifs Il s'agit d'un instantané de publication signé, d'un manifeste et d'un manifeste

Il s'agit d'un widget et d'un jeton pour une session de navigateur gratuite. le proxy est un jeton persistant et un journal de connexion

## مختصر مدت والے Jetons OAuthJ'ai trouvé les réviseurs de jetons Torii et j'ai déjà essayé la console Essayez-le avec le serveur OAuth. Il existe plusieurs variables d'environnement et un widget de connexion au code de l'appareil, ainsi que des jetons de porteur pour les jetons de porteur. انہیں console form میں خودکار طور پر inject کرتا ہے۔

| Variables | مقصد | Par défaut |
| --- | --- | --- |
| `DOCS_OAUTH_DEVICE_CODE_URL` | Point de terminaison d’autorisation de périphérique OAuth (`/oauth/device/code`) | _vide (désactivé)_ |
| `DOCS_OAUTH_TOKEN_URL` | Point de terminaison du jeton et `grant_type=urn:ietf:params:oauth:grant-type:device_code` pour le client | _vide_ |
| `DOCS_OAUTH_CLIENT_ID` | Identifiant client OAuth et aperçu de la documentation ici | _vide_ |
| `DOCS_OAUTH_SCOPE` | connexion dans les domaines d'application délimités par des espaces | `openid profile offline_access` |
| `DOCS_OAUTH_AUDIENCE` | token کو باندھنے کے لئے اختیاری Audience API | _vide_ |
| `DOCS_OAUTH_POLL_INTERVAL_MS` | Intervalle d'interrogation (ms) | `5000` (longueur < 5 000 ms par seconde) |
| `DOCS_OAUTH_DEVICE_CODE_TTL_SECONDS` | code de l'appareil et fenêtre d'expiration de repli (secondes) | `600` (300 s à 900 s pendant la durée de vie) |
| `DOCS_OAUTH_TOKEN_TTL_SECONDS` | jeton d'accès et durée de vie de repli (secondes) | `900` (300 s à 900 s pendant la durée de vie) |
| `DOCS_OAUTH_ALLOW_INSECURE` | Aperçus des versions `1` et application d'OAuth pour les applications | _unset_ |

Exemple de configuration :

```bash
export DOCS_OAUTH_DEVICE_CODE_URL="https://auth.dev.sora.example/oauth/device/code"
export DOCS_OAUTH_TOKEN_URL="https://auth.dev.sora.example/oauth/token"
export DOCS_OAUTH_CLIENT_ID="docs-preview"
export DOCS_OAUTH_SCOPE="torii openid offline_access"
# Optional audience and polling tweaks
export DOCS_OAUTH_AUDIENCE="https://torii.devnet.sora.example"
export DOCS_OAUTH_POLL_INTERVAL_MS="6000"
```Il s'agit d'`npm run start` et `npm run build` qui intègrent les valeurs `docusaurus.config.js`. لوکل aperçu کے دوران Carte d'essai "Connectez-vous avec le code de l'appareil" بٹن دکھاتا ہے۔ Vérifiez votre code pour la page de vérification OAuth en cliquant sur le lien ci-dessous. flux de l'appareil et widget:

- Utilisez un jeton de porteur et essayez-le dans la console pour injecter un jeton.
- Les en-têtes `X-TryIt-Client` et `X-TryIt-Auth` et les balises de requêtes sont disponibles.
- باقی ماندہ مدت دکھاتا ہے، اور
- jeton ختم ہونے پر خودکار طور پر صاف کر دیتا ہے۔

entrée manuelle du porteur - Les variables OAuth sont prises en charge par les réviseurs et le jeton est collé. ہیں، یا `DOCS_OAUTH_ALLOW_INSECURE=1` export کریں تاکہ aperçus isolés ویں accès anonyme قابل قبول ہو۔ OAuth est en train de construire une porte de feuille de route DOCS-1b et il y a un échec en cas d'échec

Remarque : Vous devez exposer les éléments de sécurité [Liste de contrôle de renforcement de la sécurité et de test d'intrusion] (./security-hardening.md) دیکھیں؛ Il s'agit d'un modèle de menace, d'un CSP/Types de confiance et d'étapes de test d'intrusion pour la porte DOCS-1b.

## Norito-RPC FrançaisNorito-RPC demande un proxy pour la plomberie OAuth et des routes JSON La charge utile `Content-Type: application/x-norito` est incluse dans la spécification NRPC et la charge utile Norito précodée est disponible.
(`docs/source/torii/nrpc_spec.md`)۔ Il s'agit du `fixtures/norito_rpc/` pour les charges utiles canoniques et pour les auteurs du portail, les propriétaires du SDK, les réviseurs et la relecture d'octets pour les CI. کرتا ہے۔

### Console Try It avec charge utile Norito en cours

1. `fixtures/norito_rpc/transfer_asset.norito` luminaire pour luminaire یہ فائلیں raw Norito enveloppes ہیں؛ انہیں **base64 encode نہ کریں**۔
2. Swagger et RapiDoc pour le point de terminaison NRPC sont disponibles (avec `POST /v2/pipeline/submit`) et le sélecteur **Content-Type** et `application/x-norito` pour le sélecteur de type de contenu.
3. demander l'éditeur de corps comme **binaire** comme sélecteur (Swagger comme "Fichier" comme RapiDoc comme sélecteur "Binaire/Fichier") ou `.norito` comme tel. widget octets et proxy pour le flux de données
4. demande بھیجیں۔ Le Torii `X-Iroha-Error-Code: schema_mismatch` comprend un point de terminaison pour les charges utiles binaires Il s'agit d'un hachage de schéma pour `fixtures/norito_rpc/schema_hashes.json` build et d'un hachage de schéma pour Torii build.Vous avez besoin de jetons d'autorisation pour les hôtes Torii et vous avez besoin de jetons d'autorisation. charge utile دوبارہ بھیج سکیں۔ `scripts/run_norito_rpc_fixtures.sh --note "<ticket>"` Un flux de travail est disponible pour le plan d'adoption du NRPC-4 et un ensemble de preuves (journal + résumé JSON) et des critiques دوران Réponse Try It avec capture d'écran ci-dessous

### CLI مثال (boucle)

et les appareils `curl` sont chargés de vérifier la relecture du proxy et de valider les réponses de la passerelle de débogage کرنے میں مددگار ہے:

```bash
TORII="https://torii.devnet.sora.example"
TOKEN="Bearer $(cat ~/.config/torii/devnet.token)"
curl   -H "Content-Type: application/x-norito"   -H "Authorization: ${TOKEN}"   --data-binary @fixtures/norito_rpc/transfer_asset.norito   "${TORII}/v2/pipeline/submit"
```

`transaction_fixtures.manifest.json` est utilisé pour l'entrée du luminaire et `cargo xtask norito-rpc-fixtures` pour l'encodage de la charge utile. Le mode Canary Torii est en cours de réalisation avec `curl` et proxy d'essai (`https://docs.sora.example/proxy/v2/pipeline/submit`) pour obtenir un proxy d'essai (`https://docs.sora.example/proxy/v2/pipeline/submit`). et le test d'infrastructure et les widgets du portail.

## Observabilité et opérations

Demande de méthode, chemin d'accès, origine, statut en amont, source d'authentification (`override`, `default` ou `client`) et journal de connexion tokens store store - en-têtes de porteur `X-TryIt-Auth` journalisation des valeurs et collecteur central forward Il y a une fuite de secrets en cours

### Sondes de santé et alerte

déploiements et calendrier des sondes groupées :

```bash
# Ensure the proxy responds to /healthz and forwards a sample request.
TRYIT_PROXY_PUBLIC_URL="https://docs.sora.example/proxy" TRYIT_PROXY_SAMPLE_PATH="/v2/status" npm run probe:tryit-proxy
```

Boutons d'environnement :- `TRYIT_PROXY_SAMPLE_PATH` - Itinéraire Torii (par `/proxy`) en cours de route
- `TRYIT_PROXY_SAMPLE_METHOD` - `GET`؛ écrire des itinéraires pour `POST` en cours
- `TRYIT_PROXY_PROBE_TOKEN` - exemple d'appel pour injecter un jeton au porteur
- `TRYIT_PROXY_PROBE_TIMEOUT_MS` - Délai d'attente de 5 s et remplacement du délai d'attente
- `TRYIT_PROXY_PROBE_METRICS_FILE` - `probe_success`/`probe_duration_seconds` Destination du fichier texte Prometheus
- `TRYIT_PROXY_PROBE_LABELS` - `key=value` sont des métriques séparées par des virgules et des métriques séparées par des virgules (`job=tryit-proxy` et `instance=<proxy URL>`).
- `TRYIT_PROXY_PROBE_METRICS_URL` - URL de point de terminaison de métriques (avec `http://localhost:9798/metrics`) et `TRYIT_PROXY_METRICS_LISTEN` pour le client.

Il existe un collecteur de fichiers texte et une sonde de chemin d'accès en écriture.
(`/var/lib/node_exporter/textfile_collector/tryit.prom`) Les étiquettes personnalisées sont disponibles :

```bash
TRYIT_PROXY_PUBLIC_URL="https://docs.sora.example/proxy" TRYIT_PROXY_PROBE_METRICS_FILE="/var/lib/node_exporter/textfile_collector/tryit.prom" TRYIT_PROXY_PROBE_LABELS="job=tryit-proxy,cluster=prod" npm run probe:tryit-proxy
```

Les métriques sont utilisées pour réécrire atomiquement le collecteur et la charge utile.

Pour configurer `TRYIT_PROXY_METRICS_LISTEN` et pour `TRYIT_PROXY_PROBE_METRICS_URL`, le point de terminaison des métriques est en cours de réalisation de la sonde en cas d'échec ou pour gratter la surface. جائے (entrée mal configurée et règles de pare-feu manquantes)۔ ایک contexte de production typique ہے
`TRYIT_PROXY_PROBE_METRICS_URL="http://127.0.0.1:9798/metrics"`۔

Pour les alertes et les sondes et pour la pile de surveillance. Prometheus contient des pannes sur la page suivante :

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

### Points de terminaison de métriques et tableaux de bord`TRYIT_PROXY_METRICS_LISTEN=127.0.0.1:9798` (hôte/port) pour le démarrage du proxy et le point de terminaison des métriques au format Prometheus exposent chemin d'accès `/metrics` et `TRYIT_PROXY_METRICS_PATH=/custom` en ligne Pour récupérer les totaux par méthode, les rejets de limite de débit, les erreurs/délais d'attente en amont, les résultats du proxy, et les résumés de latence et les détails :

```bash
export TRYIT_PROXY_METRICS_LISTEN="127.0.0.1:9798"
npm run tryit-proxy &
curl http://127.0.0.1:9798/metrics | head -n 5
# HELP tryit_proxy_requests_total Requests handled by method
tryit_proxy_requests_total{method="GET"} 12
tryit_proxy_rate_limited_total 1
```

Utiliser les collecteurs Prometheus/OTLP et le point final de métriques pour les latences de queue SRE et les pics de rejet Les journaux analysent les problèmes proxy خودکار طور پر `tryit_proxy_start_timestamp_ms` publier des messages et des opérateurs redémarre détecter کر سکیں۔

### Automatisation du rollback

aide à la gestion en utilisant l'URL cible Torii pour mettre à jour et restaurer l'URL cible La configuration de la configuration est `.env.tryit-proxy.bak` et la restauration est terminée.

```bash
# Update TRYIT_PROXY_TARGET and back up the previous config.
npm run manage:tryit-proxy -- update --target https://torii.devnet.sora.example

# Roll back to the previously backed-up target.
npm run manage:tryit-proxy -- rollback
```

Il s'agit de la configuration de déploiement et du remplacement de l'environnement par `--env` ou `TRYIT_PROXY_ENV` env pour le remplacement de la configuration. کریں۔