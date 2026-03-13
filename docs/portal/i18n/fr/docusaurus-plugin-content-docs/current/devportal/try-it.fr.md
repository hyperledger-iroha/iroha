---
lang: fr
direction: ltr
source: docs/portal/docs/devportal/try-it.fr.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# Bac à sable Essayez-le

Le développeur de portail fournit une console optionnelle "Try it" pour appeler des endpoints Torii sans quitter la documentation. La console relaye les requêtes via le proxy embarque pour que les navigateurs contournent les limites CORS tout en appliquant le rate limiting et l'authentification.

## Prérequis

- Node.js 18.18 ou plus récent (correspond aux exigences de build du portail)
- Accès au réseau à un environnement Torii de staging
- Un Bearer Token capable d'appeler les routes Torii que vous voulez tester

Toute la configuration du proxy passe par des variables d'environnement. Le tableau ci-dessous liste les boutons les plus importants :| Variables | Objectif | Par défaut |
| --- | --- | --- |
| `TRYIT_PROXY_TARGET` | URL Torii de base vers laquelle le proxy relais les requêtes | **Obligatoire** |
| `TRYIT_PROXY_LISTEN` | Adresse d'écoute pour le développement local (format `host:port` ou `[ipv6]:port`) | `127.0.0.1:8787` |
| `TRYIT_PROXY_ALLOWED_ORIGINS` | Liste séparée par des virgules des origines autorisées à appeler le proxy | `http://localhost:3000` |
| `TRYIT_PROXY_CLIENT_ID` | Identifiant place dans `X-TryIt-Client` pour chaque demande en amont | `docs-portal` |
| `TRYIT_PROXY_BEARER` | Bearer token par défaut relais vers Torii | _vide_ |
| `TRYIT_PROXY_ALLOW_CLIENT_AUTH` | Permet aux utilisateurs finaux de fournir leur propre token via `X-TryIt-Auth` | `0` |
| `TRYIT_PROXY_MAX_BODY` | Taille maximale du corps de requête (octets) | `1048576` |
| `TRYIT_PROXY_TIMEOUT_MS` | Timeout en amont en millisecondes | `10000` |
| `TRYIT_PROXY_RATE_LIMIT` | Demandes autorisées par fenêtre de taux par client IP | `60` |
| `TRYIT_PROXY_RATE_WINDOW_MS` | Fenêtre glissante pour limitation de débit (ms) | `60000` |
| `TRYIT_PROXY_METRICS_LISTEN` | Adresse d'écoute optionnelle pour le point final de métriques style Prometheus (`host:port` ou `[ipv6]:port`) | _vide (désactivé)_ |
| `TRYIT_PROXY_METRICS_PATH` | Chemin HTTP servi par l'endpoint de métriques | `/metrics` |

Le proxy expose également `GET /healthz`, renvoie des erreurs structurées JSON, et masque les tokens du porteur dans les logs.Activez `TRYIT_PROXY_ALLOW_CLIENT_AUTH=1` lorsque vous exposez le proxy aux utilisateurs docs pour que les panneaux Swagger et RapiDoc puissent relayer les Bearer Tokens fournis par l'utilisateur. Le proxy applique toujours les limites de taux, masque les identifiants, et enregistre si une demande d'utilisation du token par défaut ou un supplément par demande. Configurez `TRYIT_PROXY_CLIENT_ID` avec le libelle que vous voulez envoyer comme `X-TryIt-Client`
(par défaut `docs-portal`). Le proxy tronque et valide les valeurs `X-TryIt-Client` fournies par l'appelant, puis retombe sur ce par défaut afin que les passerelles de staging puissent auditer la provenance sans correler les métadonnées du navigateur.

## Démarrer le proxy en local

Installez les dépendances lors de la première configuration du portail :

```bash
cd docs/portal
npm install
```

Lancez le proxy et pointez-le vers votre instance Torii :

```bash
export TRYIT_PROXY_TARGET="https://torii.devnet.sora.example"
export TRYIT_PROXY_ALLOWED_ORIGINS="http://localhost:3000"
# Optional: preset a bearer token for the Swagger / RapiDoc panels
export TRYIT_PROXY_BEARER="Bearer eyJhbGciOi..."
npm run tryit-proxy
```

Le script logue l'adresse liée et relais les requêtes depuis `/proxy/*` vers l'origine Torii configurée.

Avant de binder le socket le script valide que
`static/openapi/torii.json` correspond au digest enregistré dans
`static/openapi/manifest.json`. Si les fichiers divergent, la commande fait écho avec une
erreur et vous demandez d'exécuter l'exécuteur `npm run sync-openapi -- --latest`. Exportez
`TRYIT_PROXY_ALLOW_STALE_SPEC=1` uniquement pour des dérogations d'urgence; le proxy logue
un avertissement et continue pour vous permettre de récupérer pendant les fenêtres de maintenance.## Cablage des widgets du portail

Quand vous construisez ou servez le développeur de portail, définissez l'URL que les widgets doivent
utiliser pour le proxy :

```bash
export TRYIT_PROXY_PUBLIC_URL="http://localhost:8787"
export TRYIT_PROXY_DEFAULT_BEARER="Bearer eyJhbGciOi..." # Optional
npm run start
```

Les composants suivants lisent ces valeurs depuis `docusaurus.config.js` :

- **Swagger UI** - rendu a `/reference/torii-swagger`; préautoriser le schéma porteur
  quand un token est présent, tague les requêtes avec `X-TryIt-Client`, injecte
  `X-TryIt-Auth`, et réécrire les appels via le proxy quand
  `TRYIT_PROXY_PUBLIC_URL` est défini.
- **RapiDoc** - rendu a `/reference/torii-rapidoc`; reflète le jeton du champion,
  réutiliser les memes headers que le panneau Swagger, et cibler automatiquement
  le proxy lorsque l'URL est configurée.
- **Try it console** - intégré sur la page d'aperçu de l'API ; permet d'envoyer des
  requetes personalisées, voir les en-têtes, et inspecter les corps de réponse.

Les deux panneaux affichent un **sélecteur de snapshots** qui s'allume
`docs/portal/static/openapi/versions.json`. Remplissez cet index avec
`npm run sync-openapi -- --version=<label> --mirror=current --latest` afin que les réviseurs
peut passer entre les specs historiques, voir le digest SHA-256 enregistré, et confirmer
si un snapshot de release embarque un manifeste signé avant d'utiliser les widgets interactifs.

Changer le token dans un widget ne touche que la session courante du navigateur; le proxy ne
persiste jamais et ne logue jamais le token fourni.

## Tokens OAuth courte dureePour éviter de distribuer des tokens Torii longue durée aux reviewers, reliez la console Try it a
votre serveur OAuth. Lorsque les variables d'environnement ci-dessous sont présentées, le portail
rendre un widget de login device code, emet des Bearer Tokens courte durée, et les injecter
automatiquement dans le formulaire de la console.

| Variables | Objectif | Par défaut |
| --- | --- | --- |
| `DOCS_OAUTH_DEVICE_CODE_URL` | Dispositif d'autorisation de point de terminaison OAuth (`/oauth/device/code`) | _vide (désactivé)_ |
| `DOCS_OAUTH_TOKEN_URL` | Jeton de point de terminaison qui accepte `grant_type=urn:ietf:params:oauth:grant-type:device_code` | _vide_ |
| `DOCS_OAUTH_CLIENT_ID` | Identifiant client OAuth enregistré pour la prévisualisation des documents | _vide_ |
| `DOCS_OAUTH_SCOPE` | Portées délimitées par des espaces demandés lors de la connexion | `openid profile offline_access` |
| `DOCS_OAUTH_AUDIENCE` | Audience API optionnelle pour lier le token | _vide_ |
| `DOCS_OAUTH_POLL_INTERVAL_MS` | Intervalle minimum de vote pendant l'attente d'approbation (ms) | `5000` (valeurs < 5000 ms rejetés) |
| `DOCS_OAUTH_DEVICE_CODE_TTL_SECONDS` | Code du dispositif de fenêtre d'expiration (secondes) | `600` (doit rester entre 300 s et 900 s) |
| `DOCS_OAUTH_TOKEN_TTL_SECONDS` | Jeton d'accès Durée de vie (secondes) | `900` (doit rester entre 300 s et 900 s) |
| `DOCS_OAUTH_ALLOW_INSECURE` | Mettre `1` pour des aperçus locaux qui omettent l'application OAuth intentionnellement | _unset_ |

Exemple de configuration :

```bash
export DOCS_OAUTH_DEVICE_CODE_URL="https://auth.dev.sora.example/oauth/device/code"
export DOCS_OAUTH_TOKEN_URL="https://auth.dev.sora.example/oauth/token"
export DOCS_OAUTH_CLIENT_ID="docs-preview"
export DOCS_OAUTH_SCOPE="torii openid offline_access"
# Optional audience and polling tweaks
export DOCS_OAUTH_AUDIENCE="https://torii.devnet.sora.example"
export DOCS_OAUTH_POLL_INTERVAL_MS="6000"
```Quand vous lancez `npm run start` ou `npm run build`, le portail intègre ces valeurs
dans `docusaurus.config.js`. En aperçu locale la carte Essayez-le affiche un bouton
"Connectez-vous avec le code de l'appareil". Les utilisateurs saisissent le code affiché sur votre page OAuth; une fois que le périphérique flow réutilise le widget :

- injecte le Bearer Token Emis dans la console Champ Essayez-le,
- taguer les requêtes avec les en-têtes existants `X-TryIt-Client` et `X-TryIt-Auth`,
- affiche la durée restante, et
- effacer automatiquement le token lorsqu'il expire.

L'entrée manuelle Bearer reste disponible; omettez les variables OAuth lorsque vous le souhaitez
forcer les reviewers à coller un token temporaire eux-memes, ou exporterez
`DOCS_OAUTH_ALLOW_INSECURE=1` pour des aperçus locaux isolés ou l'accès anonyme est
acceptable. Les builds sans OAuth configurent échouent maintenant rapidement pour satisfaire
pour répondre à la porte du roadmap DOCS-1b.

Remarque : Consultez la [Liste de contrôle de renforcement de la sécurité et de test d'intrusion](./security-hardening.md)
avant d'exposer le portail hors du labo; elle documente le modèle de menace,
le profil CSP/Trusted Types, et les étapes de pen-test qui bloquent maintenant DOCS-1b.

## Exemples Norito-RPCLes requêtes Norito-RPC partagent le meme proxy et le plumbing OAuth que les routes JSON;
elles posent simplement `Content-Type: application/x-norito` et envoient le payload Norito
pré-encoder décrit dans la spécification NRPC
(`docs/source/torii/nrpc_spec.md`).
Le dépôt fournit des charges utiles canoniques sous `fixtures/norito_rpc/` pour que les auteurs du
le portail, les propriétaires du SDK et les réviseurs peuvent rejouer les octets exacts utilisés par CI.

### Envoyer un payload Norito depuis la console Essayez-le

1. Choisissez un luminaire comme `fixtures/norito_rpc/transfer_asset.norito`. Ces
   les fichiers sont des enveloppes Norito bruts; **ne** les base64-encodez pas.
2. Dans Swagger ou RapiDoc, localisez l'endpoint NRPC (par exemple
   `POST /v2/pipeline/submit`) et basculez le sélecteur **Content-Type** sur
   `application/x-norito`.
3. Basculez l'éditeur de body en **binary** (mode "File" de Swagger ou
   sélecteur "Binary/File" de RapiDoc) et chargez le fichier `.norito`. Le widget
   transmettre les octets via le proxy sans altération.
4. Soumettez la demande. Si Torii renvoie `X-Iroha-Error-Code: schema_mismatch`,
   vérifiez que vous appelez un point de terminaison qui accepte les payloads binaires et confirmez
   que le schéma de hachage est enregistré dans `fixtures/norito_rpc/schema_hashes.json`
   correspond au build Torii cible.La console conserve le fichier le plus récent en mémoire pour que vous puissiez renvoyer le
meme payload tout en testant différents tokens d'autorisation ou hôtes Torii. Ajouter
`scripts/run_norito_rpc_fixtures.sh --note "<ticket>"` a votre workflow produit le bundle
de preuves référence dans le plan d'adoption NRPC-4 (log + CV JSON), ce qui va bien
avec la capture d'écran de la réponse Try It lors des reviews.

### Exemple CLI (boucle)

Les memes luminaires peuvent être rejoués hors du portail via `curl`, ce qui est utile
lorsque vous validez le proxy ou déboguez les réponses gateway :

```bash
TORII="https://torii.devnet.sora.example"
TOKEN="Bearer $(cat ~/.config/torii/devnet.token)"
curl   -H "Content-Type: application/x-norito"   -H "Authorization: ${TOKEN}"   --data-binary @fixtures/norito_rpc/transfer_asset.norito   "${TORII}/v2/pipeline/submit"
```

Remplacez le luminaire par n'importe quelle entrée répertoriée dans `transaction_fixtures.manifest.json`
ou encodez votre propre payload avec `cargo xtask norito-rpc-fixtures`. Quand Torii est en
mode canary vous pouvez pointer `curl` vers le proxy try-it
(`https://docs.sora.example/proxy/v2/pipeline/submit`) pour exécuter la même infrastructure
que les widgets du portail utilisent.

## Observabilité et opérations

Chaque demande est loguee une fois avec méthode, chemin, origine, statut en amont et la source
d'authentification (`override`, `default` ou `client`). Les tokens ne sont jamais stockés : les
headers Bearer et les valeurs `X-TryIt-Auth` sont rediges avant log, afin que vous puissiez
relayer stdout vers un collecteur central sans craindre des fuites.

### Sondes de santé et alertes

Lancez la sonde incluse pendant les déploiements ou sur un planning :```bash
# Ensure the proxy responds to /healthz and forwards a sample request.
TRYIT_PROXY_PUBLIC_URL="https://docs.sora.example/proxy" TRYIT_PROXY_SAMPLE_PATH="/v2/status" npm run probe:tryit-proxy
```

Boutons d'environnement :

- `TRYIT_PROXY_SAMPLE_PATH` - route Torii optionnelle (sans `/proxy`) à exercer.
- `TRYIT_PROXY_SAMPLE_METHOD` - par défaut `GET` ; définir `POST` pour les routes d'écriture.
- `TRYIT_PROXY_PROBE_TOKEN` - injecte un jeton de porteur temporaire pour l'appel d'exemple.
- `TRYIT_PROXY_PROBE_TIMEOUT_MS` - efface le timeout par défaut de 5 s.
- `TRYIT_PROXY_PROBE_METRICS_FILE` - texte de destination Prometheus optionnelle pour `probe_success`/`probe_duration_seconds`.
- `TRYIT_PROXY_PROBE_LABELS` - paires `key=value` séparées par des virgules ajoutées aux métriques (par défaut `job=tryit-proxy` et `instance=<proxy URL>`).
- `TRYIT_PROXY_PROBE_METRICS_URL` - URL optionnelle du point de terminaison de métriques (par exemple, `http://localhost:9798/metrics`) qui doit répondre avec succès lorsque `TRYIT_PROXY_METRICS_LISTEN` est actif.

Injectez les résultats dans un collecteur de fichiers texte en pointant la sonde vers un chemin inscriptible
(par exemple, `/var/lib/node_exporter/textfile_collector/tryit.prom`) et en ajoutant des étiquettes
personnalise :

```bash
TRYIT_PROXY_PUBLIC_URL="https://docs.sora.example/proxy" TRYIT_PROXY_PROBE_METRICS_FILE="/var/lib/node_exporter/textfile_collector/tryit.prom" TRYIT_PROXY_PROBE_LABELS="job=tryit-proxy,cluster=prod" npm run probe:tryit-proxy
```

Le script réécrit le fichier de métriques de facon atomique pour que votre collecteur lise
toujours une charge utile complète.

Quand `TRYIT_PROXY_METRICS_LISTEN` est configuré, définissez-le
`TRYIT_PROXY_PROBE_METRICS_URL` sur l'endpoint de métriques pour que la sonde fasse écho rapidement si
la surface de scrape disparaissait (par exemple entrée mal configurée ou règles pare-feu manquantes).
Un réglage de production typique est
`TRYIT_PROXY_PROBE_METRICS_URL="http://127.0.0.1:9798/metrics"`.Pour des alertes légères, connectez la sonde à votre pile de surveillance. Exemple Prometheus
qui page après deux échecs consécutifs :

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

Définissez `TRYIT_PROXY_METRICS_LISTEN=127.0.0.1:9798` (ou tout couple hôte/port) avant de
lancer le proxy pour exposer un point final de métriques au format Prometheus. Le chemin
par défaut est `/metrics` mais peut être remplacé par `TRYIT_PROXY_METRICS_PATH=/custom`. Chaque
scrape renvoyer des compteurs des totaux par méthode, des rejets rate limit, des erreurs/timeouts
en amont, des résultats proxy et des résumés de latence :

```bash
export TRYIT_PROXY_METRICS_LISTEN="127.0.0.1:9798"
npm run tryit-proxy &
curl http://127.0.0.1:9798/metrics | head -n 5
# HELP tryit_proxy_requests_total Requests handled by method
tryit_proxy_requests_total{method="GET"} 12
tryit_proxy_rate_limited_total 1
```

Pointez vos collecteurs Prometheus/OTLP vers l'endpoint de métriques et réutilisez les panneaux
existant dans `dashboards/grafana/docs_portal.json` afin que SRE observe les latences de file d'attente
et les photos de rejet sans analyser les logs. Le proxy publie automatiquement
`tryit_proxy_start_timestamp_ms` pour aider les opérateurs à détecter les redémarrages.

### Automatisation du rollback

Utilisez l'assistant de gestion pour mettre à jour ou restaurer l'URL cible Torii. Le scénario
stockez la configuration précédente dans `.env.tryit-proxy.bak` afin que les rollbacks soient
une seule commande.

```bash
# Update TRYIT_PROXY_TARGET and back up the previous config.
npm run manage:tryit-proxy -- update --target https://torii.devnet.sora.example

# Roll back to the previously backed-up target.
npm run manage:tryit-proxy -- rollback
```

Surchargez le chemin du fichier env avec `--env` ou `TRYIT_PROXY_ENV` si votre déploiement
stockez la configuration ailleurs.