---
lang: fr
direction: ltr
source: docs/portal/docs/devportal/try-it.pt.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# Sandbox, essayez-le

Le portail des utilisateurs inclut une console facultative « Essayez-le » pour que vous puissiez sélectionner les points de terminaison Torii sans consulter la documentation. La console retransmet les exigences requises par le proxy pour que les navigateurs respectent les limites de CORS lorsqu'elles appliquent les limites de débit et l'authentification.

## Prérequis

- Node.js 18.18 ou plus nouveau (combiné avec les exigences de build du portail)
- Accès à la rediffusion d'une ambiance de mise en scène par Torii
- Un jeton de porteur qui peut chamar comme rotas do Torii que vous prétendez exercer

Cette configuration du proxy s'effectue également selon les variations de l'environnement. Le tableau abaixo liste les boutons les plus importants :| Variables | Proposé | Par défaut |
| --- | --- | --- |
| `TRYIT_PROXY_TARGET` | Base d'URL Torii pour la qualité du proxy encaminha requisicos | **Obligatoire** |
| `TRYIT_PROXY_LISTEN` | Endereco de escuta para desenvolvimento local (format `host:port` ou `[ipv6]:port`) | `127.0.0.1:8787` |
| `TRYIT_PROXY_ALLOWED_ORIGINS` | Liste séparée par virgula d'origens qui peut chamar ou proxy | `http://localhost:3000` |
| `TRYIT_PROXY_CLIENT_ID` | Identificateur placé dans `X-TryIt-Client` pour chaque demande en amont | `docs-portal` |
| `TRYIT_PROXY_BEARER` | Jeton du porteur padrao encaminhado ao Torii | _vide_ |
| `TRYIT_PROXY_ALLOW_CLIENT_AUTH` | Permettez aux utilisateurs de finaliser leur caméra avec leur jeton via `X-TryIt-Auth` | `0` |
| `TRYIT_PROXY_MAX_BODY` | Tamanho maximo do corpo da requisicao (octets) | `1048576` |
| `TRYIT_PROXY_TIMEOUT_MS` | Timeout en amont dans les millisegundos | `10000` |
| `TRYIT_PROXY_RATE_LIMIT` | Exigences autorisées pour les taxons du client IP | `60` |
| `TRYIT_PROXY_RATE_WINDOW_MS` | Janela deslizante pour la limitation du taux (ms) | `60000` |
| `TRYIT_PROXY_METRICS_LISTEN` | Endereco de escuta facultative para o endpoint de metricas style Prometheus (`host:port` ou `[ipv6]:port`) | _vide (désactivé)_ |
| `TRYIT_PROXY_METRICS_PATH` | Chemin HTTP servi par le point de terminaison de métriques | `/metrics` |

Le proxy expose également `GET /healthz`, renvoie les erreurs JSON créées et les jetons du porteur de mascara dans nos journaux.Active `TRYIT_PROXY_ALLOW_CLIENT_AUTH=1` pour l'exportation du proxy pour les utilisateurs de documents afin que les paineis Swagger et RapiDoc puissent envoyer des jetons de porteur fournis à l'utilisateur. Le proxy a également appliqué des limites de taxons, des informations d'identification et un enregistrement pour une utilisation requise du jeton ou un remplacement pour la demande. Configurez `TRYIT_PROXY_CLIENT_ID` avec le rotulo que vous souhaitez envoyer comme `X-TryIt-Client`
(padrao `docs-portal`). Le proxy court et valide les valeurs `X-TryIt-Client` fournies au client, est activé par défaut pour que les passerelles de préparation puissent auditer la procédure sem- ment corrélée aux métadonnées du navigateur.

## Démarrer le proxy localement

Installez les dépendances avant de configurer le portail :

```bash
cd docs/portal
npm install
```

J'ai utilisé un mandataire et un représentant pour votre instance Torii :

```bash
export TRYIT_PROXY_TARGET="https://torii.devnet.sora.example"
export TRYIT_PROXY_ALLOWED_ORIGINS="http://localhost:3000"
# Optional: preset a bearer token for the Swagger / RapiDoc panels
export TRYIT_PROXY_BEARER="Bearer eyJhbGciOi..."
npm run tryit-proxy
```

Le script enregistre l'endereco ligado et encaminha requisicos de `/proxy/*` pour l'origem Torii configuré.

Avant de commencer, ne lie pas de socket ou de script valide
`static/openapi/torii.json` correspond au résumé enregistré
`static/openapi/manifest.json`. Si les archives divergent, la commande encerra l'erreur
instruire un exécuteur `npm run sync-openapi -- --latest`. Exporter
`TRYIT_PROXY_ALLOW_STALE_SPEC=1` est utilisé pour les remplacements d'urgence ; o proxy registra um aviso
et continue pour que votre voix puisse se récupérer pendant les travaux de maintenance.

## Connecter les widgets du système d'exploitation au portailLorsque vous voulez créer ou servir le portail des développeurs, définissez une URL que les widgets développent
utiliser pour le proxy :

```bash
export TRYIT_PROXY_PUBLIC_URL="http://localhost:8787"
export TRYIT_PROXY_DEFAULT_BEARER="Bearer eyJhbGciOi..." # Optional
npm run start
```

Les composants ont les mêmes valeurs que `docusaurus.config.js` :

- **Swagger UI** - rendu dans `/reference/torii-swagger` ; préautorisation ou esquema
  porteur quando ha um token, marca requisicoes com `X-TryIt-Client`,
  injeta `X-TryIt-Auth`, et réenregistrer les chambres par proxy quando
  `TRYIT_PROXY_PUBLIC_URL` est configuré.
- **RapiDoc** - rendu dans `/reference/torii-rapidoc` ; espelha o campo de token,
  réutiliser vos en-têtes de tableau Swagger et les ajouter pour le proxy
  automatiquement lorsqu'une URL est configurée.
- **Try it console** - intégré sur la page de présentation de l'API ; permettre d'envoyer
  exigences personnalisées, voir les en-têtes et inspecter le corps de la réponse.

Vous devez afficher un **sélecteur d'instantanés** que le
`docs/portal/static/openapi/versions.json`. Preencha esse indice com
`npm run sync-openapi -- --version=<label> --mirror=current --latest` pour les réviseurs
Vous pouvez alterner entre les spécifications historiques, voir le résumé SHA-256 enregistré et confirmer.
un instantané de la version présente un manifeste assassiné avant d'utiliser les widgets interactifs.

Utilisez un jeton dans n'importe quel widget pour obtenir une session réelle du navigateur ; o proxy nunca persiste
nem registra o token fornecido.

## Tokens OAuth de curta duracaoPour éviter de distribuer les jetons Torii de longue durée entre les critiques, connectez-vous à la console Essayez-le aussi
votre serveur OAuth. Quando as variaveis de ambiente abaixo estao presentes, o portal renderiza
un widget de connexion avec le code de l'appareil, des jetons de porteur de courte durée et des jets d'encre
automatiquement aucun formulaire sur la console.

| Variables | Proposé | Par défaut |
| --- | --- | --- |
| `DOCS_OAUTH_DEVICE_CODE_URL` | Point de terminaison d'autorisation du périphérique OAuth (`/oauth/device/code`) | _vide (désactivé)_ |
| `DOCS_OAUTH_TOKEN_URL` | Point de terminaison du jeton utilisé `grant_type=urn:ietf:params:oauth:grant-type:device_code` | _vide_ |
| `DOCS_OAUTH_CLIENT_ID` | Identifiant du client OAuth enregistré pour l'aperçu des documents | _vide_ |
| `DOCS_OAUTH_SCOPE` | Portées séparées par espace sollicité sans connexion | `openid profile offline_access` |
| `DOCS_OAUTH_AUDIENCE` | Audience de l'API facultative pour le vin et le jeton | _vide_ |
| `DOCS_OAUTH_POLL_INTERVAL_MS` | Intervalle minimum d'interrogation pendant la garde approuvée (ms) | `5000` (valeurs < 5000 ms sao rejeitados) |
| `DOCS_OAUTH_DEVICE_CODE_TTL_SECONDS` | Janela de expiration do device code (secondes) | `600` (deve ficar entre 300 s et 900 s) |
| `DOCS_OAUTH_TOKEN_TTL_SECONDS` | Duracao do access token (secondes) | `900` (deve ficar entre 300 s et 900 s) |
| `DOCS_OAUTH_ALLOW_INSECURE` | Définir `1` pour les aperçus locaux qui nécessitent l'application OAuth intentionnellement | _unset_ |

Exemple de configuration :

```bash
export DOCS_OAUTH_DEVICE_CODE_URL="https://auth.dev.sora.example/oauth/device/code"
export DOCS_OAUTH_TOKEN_URL="https://auth.dev.sora.example/oauth/token"
export DOCS_OAUTH_CLIENT_ID="docs-preview"
export DOCS_OAUTH_SCOPE="torii openid offline_access"
# Optional audience and polling tweaks
export DOCS_OAUTH_AUDIENCE="https://torii.devnet.sora.example"
export DOCS_OAUTH_POLL_INTERVAL_MS="6000"
```Quando voce roda `npm run start` ou `npm run build`, le portail embute ses valeurs
`docusaurus.config.js`. Pendant un aperçu de la carte audio locale Essayez-le en montrant un botao
"Connectez-vous avec le code de l'appareil". Les utilisateurs utilisent le code affiché sur leur page OAuth ; quand le flux de l'appareil réussit à créer le widget :

- injeta ou jeton du porteur émis sur le champ de la console Essayez-le,
- marque requise avec les en-têtes existants `X-TryIt-Client` et `X-TryIt-Auth`,
- exibe o tempo de vida restante, e
- nettoyer automatiquement le jeton à l'expiration.

A entrada manual Bearer continua disponivel; omita comme variaveis OAuth quand je souhaite
forcar reviewers a colar um token temporario por conta propria, ou exporte
`DOCS_OAUTH_ALLOW_INSECURE=1` pour les aperçus locaux isolés avec accès anonyme et
Aceitavel. Construit avec une configuration OAuth juste avant Falham pour répondre à la porte d'entrée
feuille de route DOCS-1b.

Remarque : Réviser une [liste de contrôle de renforcement de la sécurité et de test d'intrusion](./security-hardening.md)
avant l'exportation ou le portail du laboratoire ; ela documenta le modèle de menace,
Le profil CSP/Trusted Types et les étapes de pen-test qui pourraient bloquer DOCS-1b.

## Amostras Norito-RPCConditions requises Norito-RPC pour partager le proxy et la plomberie OAuth selon la rotation JSON ;
ils ont simplement défini `Content-Type: application/x-norito` et envoyé la charge utile Norito
description pré-encodée avec la spécification NRPC
(`docs/source/torii/nrpc_spec.md`).
Le référentiel inclut les charges utiles canoniques sur `fixtures/norito_rpc/` pour les auteurs
portail, les propriétaires du SDK et les réviseurs peuvent reproduire les octets exatos que CI utilise.

### Envoyer une charge utile Norito pour la console Essayez-le

1. Escolha um luminaire como `fixtures/norito_rpc/transfer_asset.norito`. Essès
   arquivos sao enveloppes Norito brutos; **nao** faca base64.
2. Dans Swagger ou RapiDoc, localisez le point final NRPC (par exemple
   `POST /v1/pipeline/submit`) et modifier le sélecteur **Content-Type** para
   `application/x-norito`.
3. Accédez à l'éditeur de corps pour **binaire** (mode "Fichier" de Swagger ou
   sélectionnez "Binaire/Fichier" dans RapiDoc) et envoyez le fichier `.norito`. Ô widget
   transmettre les octets par proxy sans modification.
4. Envie d'une demande. Voir Torii retornar `X-Iroha-Error-Code: schema_mismatch`,
   vérifiez si vous appelez un point de terminaison qui prend en charge les binaires de charges utiles et confirmez
   que le hachage de schéma est enregistré dans `fixtures/norito_rpc/schema_hashes.json`
   correspond à la build Torii que vous utilisez.La console tient ou archive plus récemment dans la mémoire pour que vous puissiez revivre votre message
La charge utile concerne différents jetons d'autorisation ou d'hôtes Torii. Ajouter
`scripts/run_norito_rpc_fixtures.sh --note "<ticket>"` pour votre workflow produit ou bundle de
 preuves référencées dans le plan d'adoption NRPC-4 (log + résumé JSON), qui se combinent entre elles
com capturer des captures d'écran en réponse à Try It lors des critiques.

### Exemple CLI (boucle)

Mes appareils peuvent être reproduits sur les forums du portail via `curl`, ou ici
lorsque vous validez le proxy ou supprimez les réponses de la passerelle :

```bash
TORII="https://torii.devnet.sora.example"
TOKEN="Bearer $(cat ~/.config/torii/devnet.token)"
curl   -H "Content-Type: application/x-norito"   -H "Authorization: ${TOKEN}"   --data-binary @fixtures/norito_rpc/transfer_asset.norito   "${TORII}/v1/pipeline/submit"
```

Troque o luminaire por n'importe quelle entrée listada em `transaction_fixtures.manifest.json`
ou codifier votre propre charge utile avec `cargo xtask norito-rpc-fixtures`. Quand le Torii est-il
en mode Canary, vous pouvez proposer le `curl` pour l'essayer par proxy
(`https://docs.sora.example/proxy/v1/pipeline/submit`) pour exercer le message
infraestrutura usada pelos widgets do portail.

## Observabilité et opérations

Chaque demande et enregistrée une fois avec la méthode, le chemin, l'origine, le statut en amont et la source
de autenticacao (`override`, `default` ou `client`). Jetons nunca sao armazenados: tanto
os en-têtes porteur quanto os valeurs `X-TryIt-Auth` sao redigidos antes do log,
alors vous pouvez encaminhar stdout pour un collecteur central qui se préoccupe des vazamentos.

### Sondes de santé et alertes

J'ai utilisé la sonde incluse pendant le déploiement ou selon un calendrier :

```bash
# Ensure the proxy responds to /healthz and forwards a sample request.
TRYIT_PROXY_PUBLIC_URL="https://docs.sora.example/proxy" TRYIT_PROXY_SAMPLE_PATH="/v1/status" npm run probe:tryit-proxy
```

Boutons d'ambiance :- `TRYIT_PROXY_SAMPLE_PATH` - rotation Torii facultative (sem `/proxy`) pour l'exercice.
- `TRYIT_PROXY_SAMPLE_METHOD` - padrao `GET` ; définir `POST` pour les rotations d'écriture.
- `TRYIT_PROXY_PROBE_TOKEN` - injecte un jeton de porteur temporairement pour la chambre d'amis.
- `TRYIT_PROXY_PROBE_TIMEOUT_MS` - indique le délai d'attente pendant 5 s.
- `TRYIT_PROXY_PROBE_METRICS_FILE` - destination du texte Prometheus en option pour `probe_success`/`probe_duration_seconds`.
- `TRYIT_PROXY_PROBE_LABELS` - pare `key=value` séparé par virgula anexados comme metricas (padrao `job=tryit-proxy` et `instance=<proxy URL>`).
- `TRYIT_PROXY_PROBE_METRICS_URL` - URL facultative du point de terminaison de métriques (par exemple, `http://localhost:9798/metrics`) qui doit répondre avec succès lorsque `TRYIT_PROXY_METRICS_LISTEN` est habilité.

Alimentez les résultats dans un collecteur de fichiers texte en utilisant ou en sonde pour un chemin grave
(par exemple, `/var/lib/node_exporter/textfile_collector/tryit.prom`) et étiquettes supplémentaires
personnalisés :

```bash
TRYIT_PROXY_PUBLIC_URL="https://docs.sora.example/proxy" TRYIT_PROXY_PROBE_METRICS_FILE="/var/lib/node_exporter/textfile_collector/tryit.prom" TRYIT_PROXY_PROBE_LABELS="job=tryit-proxy,cluster=prod" npm run probe:tryit-proxy
```

Le script réécrit ou l'archive de mesures de forme atomique pour que votre collectionneur semper leia
euh, charge utile complète.

Quand le `TRYIT_PROXY_METRICS_LISTEN` est configuré, défini
`TRYIT_PROXY_PROBE_METRICS_URL` pour le point final de métriques pour que la sonde échoue rapidement
la surface de scrape disparaît (par exemple, une entrée mal configurée ou un pare-feu activé).
Pour ajuster le type de production et
`TRYIT_PROXY_PROBE_METRICS_URL="http://127.0.0.1:9798/metrics"`.

Pour alerter les niveaux, connectez la sonde à votre pile de surveillance. Exemple Prometheus que
page après deux erreurs consécutives :

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
```### Endpoint de métriques et tableaux de bord

Définir `TRYIT_PROXY_METRICS_LISTEN=127.0.0.1:9798` (ou tout ce qui concerne l'hôte/le port) avant de
Lancez un proxy pour exporter un point de terminaison de métriques au format Prometheus. Le chemin
padrao et `/metrics` mais peut être écrit via
`TRYIT_PROXY_METRICS_PATH=/custom`. Chaque grattage retorna contadores de totais por metodo,
Rejeicos por rate limit, erreurs/timeouts en amont, résultats du proxy et résumés de latence :

```bash
export TRYIT_PROXY_METRICS_LISTEN="127.0.0.1:9798"
npm run tryit-proxy &
curl http://127.0.0.1:9798/metrics | head -n 5
# HELP tryit_proxy_requests_total Requests handled by method
tryit_proxy_requests_total{method="GET"} 12
tryit_proxy_rate_limited_total 1
```

Apontez vos collecteurs Prometheus/OTLP pour le point final de métriques et réutilisez les panneaux d'exploitation
existe dans `dashboards/grafana/docs_portal.json` pour que SRE observe les latences de la queue
et des pics de recherche pour analyser les journaux. Le proxy public automatiquement `tryit_proxy_start_timestamp_ms`
para ajudar operadores a detectar reinicios.

### Rollback automatique

Utilisez l'assistant de gestion pour actualiser ou restaurer une URL à l'aide de Torii. Ô scénario
Armazena a configuracao anterior em `.env.tryit-proxy.bak` para que rollbacks sejam um
un commando unique.

```bash
# Update TRYIT_PROXY_TARGET and back up the previous config.
npm run manage:tryit-proxy -- update --target https://torii.devnet.sora.example

# Roll back to the previously backed-up target.
npm run manage:tryit-proxy -- rollback
```

Sobrescreva o chemininho do arquivo env com `--env` ou `TRYIT_PROXY_ENV` se sua implantacao
armazenar a configuracao em outro lugar.