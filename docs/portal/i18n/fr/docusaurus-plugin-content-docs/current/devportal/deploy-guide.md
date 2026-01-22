---
id: deploy-guide
lang: fr
direction: ltr
source: docs/portal/docs/devportal/deploy-guide.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
---

## Vue d'ensemble

Ce playbook convertit les items du roadmap **DOCS-7** (publication SoraFS) et **DOCS-8**
(automatisation du pin CI/CD) en une procedure actionnable pour le portail developpeur.
Il couvre la phase build/lint, l'empaquetage SoraFS, la signature de manifestes avec Sigstore,
la promotion d'alias, la verification et les drills de rollback pour que chaque preview et release
soit reproductible et auditable.

Le flux suppose que vous avez le binaire `sorafs_cli` (compile avec `--features cli`), l'acces a
un endpoint Torii avec des permissions de pin-registry, et des credentials OIDC pour Sigstore.
Stockez les secrets longue duree (`IROHA_PRIVATE_KEY`, `SIGSTORE_ID_TOKEN`, tokens Torii) dans
votre coffre CI; les executions locales peuvent les charger via des exports du shell.

## Prerequis

- Node 18.18+ avec `npm` ou `pnpm`.
- `sorafs_cli` via `cargo run -p sorafs_car --features cli --bin sorafs_cli`.
- URL Torii qui expose `/v1/sorafs/*` plus un compte/cle privee d'autorite qui peut soumettre
  des manifestes et des alias.
- Emmeteur OIDC (GitHub Actions, GitLab, workload identity, etc.) pour emettre un `SIGSTORE_ID_TOKEN`.
- Optionnel: `examples/sorafs_cli_quickstart.sh` pour des dry runs et
  `docs/source/sorafs_ci_templates.md` pour le scaffolding des workflows GitHub/GitLab.
- Configurez les variables OAuth Try it (`DOCS_OAUTH_*`) et executez la
  [security-hardening checklist](./security-hardening.md) avant de promouvoir un build
  hors du lab. Le build du portail echoue maintenant lorsque ces variables manquent
  ou lorsque les knobs TTL/polling sortent des fenetres imposees; exportez
  `DOCS_OAUTH_ALLOW_INSECURE=1` uniquement pour des previews locales jetables. Joignez
  les preuves de pen-test au ticket de release.

## Etape 0 - Capturer un bundle du proxy Try it

Avant de promouvoir un preview vers Netlify ou la gateway, figez les sources du proxy Try it
et le digest du manifeste OpenAPI signe dans un bundle deterministe:

```bash
cd docs/portal
npm run release:tryit-proxy -- \
  --out ../../artifacts/tryit-proxy/$(date -u +%Y%m%dT%H%M%SZ) \
  --target https://torii.dev.sora \
  --label preview-2026-02-14
```

`scripts/tryit-proxy-release.mjs` copie les helpers proxy/probe/rollback, verifie la signature
OpenAPI et ecrit `release.json` plus `checksums.sha256`. Joignez ce bundle au ticket de
promotion Netlify/SoraFS gateway pour que les reviewers puissent rejouer les sources exactes
et les hints du target Torii sans reconstruire. Le bundle enregistre aussi si les bearers
fournis par le client etaient actives (`allow_client_auth`) pour garder le plan de rollout
et les regles CSP en phase.

## Etape 1 - Build et lint du portail

```bash
cd docs/portal
npm ci
npm run sync-openapi
npm run sync-norito-snippets
npm run test:norito-snippets
npm run test:widgets
npm run check:links
npm run build
```

`npm run build` execute automatiquement `scripts/write-checksums.mjs`, produisant:

- `build/checksums.sha256` - manifeste SHA256 pour `sha256sum -c`.
- `build/release.json` - metadata (`tag`, `generated_at`, `source`) fixes dans chaque CAR/manifeste.

Archivez ces fichiers avec le resume CAR pour que les reviewers puissent comparer les artefacts
preview sans reconstruire.

## Etape 2 - Empaqueter les assets statiques

Executez le packer CAR contre le repertoire de sortie Docusaurus. L'exemple ci-dessous ecrit
les artefacts sous `artifacts/devportal/`.

```bash
OUT=artifacts/devportal
mkdir -p "$OUT"

sorafs_cli car pack \
  --input build \
  --car-out "$OUT"/portal.car \
  --plan-out "$OUT"/portal.plan.json \
  --summary-out "$OUT"/portal.car.json \
  --chunker-handle sorafs.sf1@1.0.0
```

Le resume JSON capture les comptes de chunks, les digests et les hints de planification de proof
que `manifest build` et les dashboards CI reutilisent plus tard.

## Etape 2b - Empaqueter les companions OpenAPI et SBOM

DOCS-7 requiert de publier le site du portail, le snapshot OpenAPI et les payloads SBOM
comme des manifestes distincts pour que les gateways puissent agrafer les headers
`Sora-Proof`/`Sora-Content-CID` pour chaque artefact. Le helper de release
(`scripts/sorafs-pin-release.sh`) empaquete deja le repertoire OpenAPI
(`static/openapi/`) et les SBOMs emis via `syft` dans des CARs separes
`openapi.*`/`*-sbom.*` et enregistre les metadonnees dans
`artifacts/sorafs/portal.additional_assets.json`. En flux manuel,
repetez les Etapes 2-4 pour chaque payload avec ses propres prefixes et labels metadata
(par exemple `--car-out "$OUT"/openapi.car` plus
`--metadata alias_label=docs.sora.link/openapi`). Enregistrez chaque paire manifeste/alias
sur Torii (site, OpenAPI, SBOM portail, SBOM OpenAPI) avant de basculer le DNS pour que
la gateway serve des preuves agrafees pour tous les artefacts publies.

## Etape 3 - Construire le manifeste

```bash
sorafs_cli manifest build \
  --summary "$OUT"/portal.car.json \
  --manifest-out "$OUT"/portal.manifest.to \
  --manifest-json-out "$OUT"/portal.manifest.json \
  --pin-min-replicas 5 \
  --pin-storage-class warm \
  --pin-retention-epoch 14 \
  --metadata alias_label=docs.sora.link
```

Ajustez les flags de pin-policy selon votre fenetre de release (par exemple, `--pin-storage-class
hot` pour les canaries). La variante JSON est optionnelle mais pratique pour le code review.

## Etape 4 - Signer avec Sigstore

```bash
sorafs_cli manifest sign \
  --manifest "$OUT"/portal.manifest.to \
  --chunk-plan "$OUT"/portal.plan.json \
  --bundle-out "$OUT"/portal.manifest.bundle.json \
  --signature-out "$OUT"/portal.manifest.sig \
  --identity-token-provider github-actions \
  --identity-token-audience sorafs-devportal
```

Le bundle enregistre le digest du manifeste, les digests de chunks et un hash BLAKE3 du token
OIDC sans persister le JWT. Gardez le bundle et la signature detachee; les promotions de
production peuvent reutiliser les memes artefacts au lieu de re-signer. Les executions locales
peuvent remplacer les flags provider par `--identity-token-env` (ou definir `SIGSTORE_ID_TOKEN`
) quand un helper OIDC externe emet le token.

## Etape 5 - Soumettre au pin registry

Soumettez le manifeste signe (et le plan de chunks) a Torii. Demandez toujours un resume pour
que l'entree/alias resulte soit auditable.

```bash
sorafs_cli manifest submit \
  --manifest "$OUT"/portal.manifest.to \
  --chunk-plan "$OUT"/portal.plan.json \
  --torii-url "$TORII_URL" \
  --authority ih58... \
  --private-key "$IROHA_PRIVATE_KEY" \
  --submitted-epoch 20260101 \
  --alias-namespace docs \
  --alias-name sora.link \
  --alias-proof "$OUT"/docs.alias.proof \
  --summary-out "$OUT"/portal.submit.json \
  --response-out "$OUT"/portal.submit.response.json
```

Lors d'un rollout de preview ou de canary (`docs-preview.sora`), repetez la soumission
avec un alias unique pour que QA puisse verifier le contenu avant la promotion production.

Le binding d'alias requiert trois champs: `--alias-namespace`, `--alias-name` et `--alias-proof`.
Governance produit le bundle de proof (base64 ou bytes Norito) quand la demande d'alias est
approuvee; stockez-le dans les secrets CI et exposez-le comme fichier avant d'invoquer
`manifest submit`. Laissez les flags d'alias vides quand vous voulez seulement pinner
le manifeste sans toucher au DNS.

## Etape 5b - Generer une proposition de governance

Chaque manifeste doit voyager avec une proposition prete pour Parliament afin que tout citoyen
Sora puisse introduire le changement sans emprunter des credentials privilegies. Apres les
etapes submit/sign, lancez:

```bash
sorafs_cli manifest proposal \
  --manifest "$OUT"/portal.manifest.to \
  --chunk-plan "$OUT"/portal.plan.json \
  --submitted-epoch 20260101 \
  --alias-hint docs.sora.link \
  --proposal-out "$OUT"/portal.pin.proposal.json
```

`portal.pin.proposal.json` capture l'instruction canonique `RegisterPinManifest`,
le digest de chunks, la policy et l'indice d'alias. Joignez-le au ticket de governance ou au
portail Parliament pour que les delegues puissent comparer le payload sans reconstruire.
Comme la commande ne touche jamais la cle d'autorite Torii, tout citoyen peut rediger la
proposition localement.

## Etape 6 - Verifier les proofs et la telemetrie

Apres le pin, executez les etapes de verification deterministe:

```bash
sorafs_cli proof verify \
  --manifest "$OUT"/portal.manifest.to \
  --car "$OUT"/portal.car \
  --summary-out "$OUT"/portal.proof.json

sorafs_cli manifest verify-signature \
  --manifest "$OUT"/portal.manifest.to \
  --bundle "$OUT"/portal.manifest.bundle.json \
  --chunk-plan "$OUT"/portal.plan.json
```

- Surveillez `torii_sorafs_gateway_refusals_total` et
  `torii_sorafs_replication_sla_total{outcome="missed"}` pour les anomalies.
- Executez `npm run probe:portal` pour exercer le proxy Try-It et les liens enregistres
  contre le contenu fraichement pinne.
- Capturez l'evidence de monitoring decrite dans
  [Publishing & Monitoring](./publishing-monitoring.md) pour que la gate d'observabilite
  DOCS-3c soit satisfaite avec les etapes de publication. Le helper accepte maintenant
  plusieurs entrees `bindings` (site, OpenAPI, SBOM portail, SBOM OpenAPI) et enforce
  `Sora-Name`/`Sora-Proof`/`Sora-Content-CID` sur l'hote cible via le guard optionnel
  `hostname`. L'invocation ci-dessous ecrit un resume JSON unique et le bundle d'evidence
  (`portal.json`, `tryit.json`, `binding.json`, `checksums.sha256`) sous le repertoire
  de release:

  ```bash
  npm run monitor:publishing -- \
    --config ../../configs/docs_monitor.json \
    --json-out ../../artifacts/sorafs/preview-2026-02-14/monitoring/summary.json \
    --evidence-dir ../../artifacts/sorafs/preview-2026-02-14/monitoring
  ```

## Etape 6a - Planifier les certificats du gateway

Derivez le plan SAN/challenge TLS avant de creer les paquets GAR pour que l'equipe gateway et
les approbateurs DNS examinent la meme evidence. Le nouveau helper reflete les inputs DG-3
en enumerant les hosts wildcard canoniques, les SANs pretty-host, les labels DNS-01 et
les challenges ACME recommandes:

```bash
cargo xtask soradns-acme-plan \
  --name docs.sora \
  --json-out artifacts/sorafs/portal.acme-plan.json
```

Commitez le JSON avec le bundle de release (ou uploadez-le avec le ticket de changement) pour
que les operateurs puissent coller les valeurs SAN dans la configuration
`torii.sorafs_gateway.acme` de Torii et que les reviewers GAR puissent confirmer les
mappings canonique/pretty sans re-executer les derivations d'hotes. Ajoutez des arguments
`--name` supplementaires pour chaque suffixe promu dans le meme release.

## Etape 6b - Deriver les mappings d'hotes canoniques

Avant de templater les payloads GAR, enregistrez le mapping d'hote deterministe pour chaque alias.
`cargo xtask soradns-hosts` hashe chaque `--name` en son label canonique
(`<base32>.gw.sora.id`), emet le wildcard requis (`*.gw.sora.id`) et derive le pretty host
(`<alias>.gw.sora.name`). Persistez la sortie dans les artefacts de release pour que les
reviewers DG-3 puissent comparer le mapping avec la soumission GAR:

```bash
cargo xtask soradns-hosts \
  --name docs.sora \
  --json-out artifacts/sorafs/portal.canonical-hosts.json
```

Utilisez `--verify-host-patterns <file>` pour echouer vite lorsqu'un GAR ou un JSON de
binding gateway omet un des hosts requis. Le helper accepte plusieurs fichiers de verification,
ce qui facilite le lint du template GAR et du `portal.gateway.binding.json` agrafe dans
la meme invocation:

```bash
cargo xtask soradns-hosts \
  --name docs.sora \
  --json-out artifacts/sorafs/portal.canonical-hosts.json \
  --verify-host-patterns artifacts/sorafs/portal.gar.json \
  --verify-host-patterns artifacts/sorafs/portal.gateway.binding.json
```

Joignez le JSON de resume et le log de verification au ticket de changement DNS/gateway pour que
les auditeurs puissent confirmer les hosts canoniques, wildcard et pretty sans re-executer les
scripts. Re-executez la commande quand de nouveaux alias sont ajoutes au bundle pour que les
updates GAR heritent de la meme evidence.

## Etape 7 - Generer le descripteur de cutover DNS

Les cutovers production requierent un paquet de changement auditable. Apres une soumission
reussie (binding d'alias), le helper emet
`artifacts/sorafs/portal.dns-cutover.json`, capturant:

- metadata du binding d'alias (namespace/name/proof, digest manifeste, URL Torii,
  epoch soumis, autorite);
- contexte de release (tag, alias label, chemins manifeste/CAR, plan de chunks, bundle Sigstore);
- pointeurs de verification (commande probe, alias + endpoint Torii); et
- champs optionnels de change-control (id ticket, fenetre cutover, contact ops,
  hostname/zone production);
- metadata de promotion de route derivee du header `Sora-Route-Binding`
  (host canonique/CID, chemins de header + binding, commandes de verification), assurant que la
  promotion GAR et les drills de fallback referencent la meme evidence;
- les artefacts de route-plan generes (`gateway.route_plan.json`,
  templates de headers et headers rollback optionnels) pour que les tickets de changement et
  hooks de lint CI puissent verifier que chaque paquet DG-3 reference les plans de
  promotion/rollback canoniques avant l'approbation;
- metadata optionnelle d'invalidation de cache (endpoint de purge, variable auth, payload JSON,
  et exemple de commande `curl`); et
- hints de rollback pointant vers le descripteur precedent (tag de release et digest manifeste)
  pour que les tickets capturent un chemin de fallback deterministe.

Quand le release requiert des purges de cache, generez un plan canonique avec le descripteur:

```bash
cargo xtask soradns-cache-plan \
  --name docs.sora \
  --path / \
  --path /gateway/manifest.json \
  --auth-header Authorization \
  --auth-env CACHE_PURGE_TOKEN \
  --json-out artifacts/sorafs/portal.cache_plan.json
```

Joignez le `portal.cache_plan.json` au paquet DG-3 pour que les operateurs aient des hosts/paths
 deterministes (et les hints auth correspondants) lorsqu'ils emettent des requetes `PURGE`.
La section cache optionnelle du descripteur peut referencer ce fichier directement, gardant
les reviewers alines sur les endpoints exactement purges durant un cutover.

Chaque paquet DG-3 a aussi besoin d'un checklist promotion + rollback. Generez-le via
`cargo xtask soradns-route-plan` pour que les reviewers puissent tracer les etapes exactes
preflight, cutover et rollback par alias:

```bash
cargo xtask soradns-route-plan \
  --name docs.sora \
  --json-out artifacts/sorafs/gateway.route_plan.json
```

Le `gateway.route_plan.json` emis capture hosts canoniques/pretty, rappels de health-check
par etape, updates de binding GAR, purges de cache et actions de rollback. Joignez-le aux
artefacts GAR/binding/cutover avant de soumettre le ticket de changement pour que Ops puisse
repeter et valider les memes etapes scriptes.

`scripts/generate-dns-cutover-plan.mjs` alimente ce descripteur et s'execute automatiquement depuis
`sorafs-pin-release.sh`. Pour le regenerer ou le personnaliser manuellement:

```bash
node scripts/generate-dns-cutover-plan.mjs \
  --pin-report artifacts/sorafs/portal.pin.report.json \
  --out artifacts/sorafs/portal.dns-cutover.json \
  --change-ticket OPS-4821 \
  --dns-hostname docs.sora.link \
  --dns-zone sora.link \
  --ops-contact docs-oncall@sora.link \
  --cache-purge-endpoint https://cache.api/purge \
  --cache-purge-auth-env CACHE_PURGE_TOKEN \
  --previous-dns-plan artifacts/sorafs/previous.dns-cutover.json
```

Renseignez les metadata optionnelles via des variables d'environnement avant d'executer le helper de pin:

| Variable | But |
| --- | --- |
| `DNS_CHANGE_TICKET` | ID de ticket stocke dans le descripteur. |
| `DNS_CUTOVER_WINDOW` | Fenetre de cutover ISO8601 (ex: `2026-03-21T15:00Z/2026-03-21T15:30Z`). |
| `DNS_HOSTNAME`, `DNS_ZONE` | Hostname production + zone autoritative. |
| `DNS_OPS_CONTACT` | Alias on-call ou contact d'escalade. |
| `DNS_CACHE_PURGE_ENDPOINT` | Endpoint purge cache enregistre dans le descripteur. |
| `DNS_CACHE_PURGE_AUTH_ENV` | Env var contenant le token purge (default: `CACHE_PURGE_TOKEN`). |
| `DNS_PREVIOUS_PLAN` | Chemin vers le descripteur precedent pour metadata rollback. |

Joignez le JSON au review de changement DNS pour que les approbateurs puissent verifier les digests
manifestes, bindings d'alias et commandes probe sans fouiller les logs CI. Les flags CLI
`--dns-change-ticket`, `--dns-cutover-window`, `--dns-hostname`,
`--dns-zone`, `--ops-contact`, `--cache-purge-endpoint`,
`--cache-purge-auth-env`, et `--previous-dns-plan` fournissent les memes overrides
quand le helper tourne hors CI.

## Etape 8 - Emettre le squelette de zonefile du resolver (optionnel)

Quand la fenetre de cutover production est connue, le script de release peut emettre
le squelette de zonefile SNS et le snippet resolver automatiquement. Passez les
records DNS desires et metadata via variables d'environnement ou options CLI; le helper
appelle `scripts/sns_zonefile_skeleton.py` immediatement apres generation du descripteur.
Fournissez au moins une valeur A/AAAA/CNAME et le digest GAR (BLAKE3-256 du payload GAR signe).
Si la zone/hostname sont connus et `--dns-zonefile-out` est omis, le helper ecrit dans
`artifacts/sns/zonefiles/<zone>/<hostname>.json` et remplit
`ops/soradns/static_zones.<hostname>.json` comme snippet resolver.

| Variable / flag | But |
| --- | --- |
| `DNS_ZONEFILE_OUT`, `--dns-zonefile-out` | Chemin du squelette zonefile genere. |
| `DNS_ZONEFILE_RESOLVER_SNIPPET`, `--dns-zonefile-resolver-snippet` | Chemin du snippet resolver (default: `ops/soradns/static_zones.<hostname>.json` si omis). |
| `DNS_ZONEFILE_TTL`, `--dns-zonefile-ttl` | TTL applique aux records generes (default: 600 secondes). |
| `DNS_ZONEFILE_IPV4`, `--dns-zonefile-ipv4` | Adresses IPv4 (env separe par virgules ou flag CLI repetable). |
| `DNS_ZONEFILE_IPV6`, `--dns-zonefile-ipv6` | Adresses IPv6. |
| `DNS_ZONEFILE_CNAME`, `--dns-zonefile-cname` | Target CNAME optionnel. |
| `DNS_ZONEFILE_SPKI`, `--dns-zonefile-spki-pin` | Pins SPKI SHA-256 (base64). |
| `DNS_ZONEFILE_TXT`, `--dns-zonefile-txt` | Entrees TXT additionnelles (`key=value`). |
| `DNS_ZONEFILE_VERSION`, `--dns-zonefile-version` | Override du label de version zonefile calcule. |
| `DNS_ZONEFILE_EFFECTIVE_AT`, `--dns-zonefile-effective-at` | Force le timestamp `effective_at` (RFC3339) au lieu du debut de la fenetre cutover. |
| `DNS_ZONEFILE_PROOF`, `--dns-zonefile-proof` | Override du proof literal enregistre dans la metadata. |
| `DNS_ZONEFILE_CID`, `--dns-zonefile-cid` | Override du CID enregistre dans la metadata. |
| `DNS_ZONEFILE_FREEZE_STATE`, `--dns-zonefile-freeze-state` | Etat de freeze guardian (soft, hard, thawing, monitoring, emergency). |
| `DNS_ZONEFILE_FREEZE_TICKET`, `--dns-zonefile-freeze-ticket` | Reference ticket guardian/council pour freeze. |
| `DNS_ZONEFILE_FREEZE_EXPIRES_AT`, `--dns-zonefile-freeze-expires-at` | Timestamp RFC3339 pour thawing. |
| `DNS_ZONEFILE_FREEZE_NOTES`, `--dns-zonefile-freeze-note` | Notes freeze additionnelles (env separe par virgules ou flag repetable). |
| `DNS_GAR_DIGEST`, `--dns-gar-digest` | Digest BLAKE3-256 (hex) du payload GAR signe. Requis quand des bindings gateway sont presents. |

Le workflow GitHub Actions lit ces valeurs depuis les secrets du repo pour que chaque pin production
emet automatiquement les artefacts zonefile. Configurez les secrets suivants (les valeurs peuvent
contenir des listes separees par virgules pour les champs multivalues):

| Secret | But |
| --- | --- |
| `DOCS_SORAFS_DNS_HOSTNAME`, `DOCS_SORAFS_DNS_ZONE` | Hostname/zone production passes au helper. |
| `DOCS_SORAFS_DNS_OPS_CONTACT` | Alias on-call stocke dans le descripteur. |
| `DOCS_SORAFS_ZONEFILE_IPV4`, `DOCS_SORAFS_ZONEFILE_IPV6` | Records IPv4/IPv6 a publier. |
| `DOCS_SORAFS_ZONEFILE_CNAME` | Target CNAME optionnel. |
| `DOCS_SORAFS_ZONEFILE_SPKI` | Pins SPKI base64. |
| `DOCS_SORAFS_ZONEFILE_TXT` | Entrees TXT additionnelles. |
| `DOCS_SORAFS_ZONEFILE_FREEZE_STATE/TICKET/EXPIRES_AT/NOTES` | Metadata freeze enregistree dans le squelette. |
| `DOCS_SORAFS_GAR_DIGEST` | Digest BLAKE3 en hex du payload GAR signe. |

En declenchant `.github/workflows/docs-portal-sorafs-pin.yml`, fournissez les inputs
`dns_change_ticket` et `dns_cutover_window` pour que le descripteur/zonefile herite de la
bonne fenetre. Laissez-les vides uniquement pour des dry runs.

Invocation type (en ligne avec le runbook owner SN-7):

```bash
./docs/portal/scripts/sorafs-pin-release.sh \
  --dns-zonefile-out artifacts/sns/zonefiles/sora.link/20250303.docs.sora.json \
  --dns-zonefile-resolver-snippet ops/soradns/static_zones.docs.json \
  --dns-zonefile-ipv4 198.51.100.4 \
  --dns-zonefile-ttl 600 \
  --dns-zonefile-freeze-state soft \
  --dns-zonefile-freeze-ticket SNS-DF-XXXX \
  --dns-zonefile-freeze-expires-at 2025-03-10T12:00Z \
  --dns-gar-digest <gar-digest-hex> \
  ...autres flags...
```

Le helper recupere automatiquement le ticket de changement comme entree TXT et herite du debut de
la fenetre cutover comme timestamp `effective_at` sauf override. Pour le workflow complet, voir
`docs/source/sorafs_gateway_dns_owner_runbook.md`.

### Template de headers gateway

Le helper de deploy emet aussi `portal.gateway.headers.txt` et
`portal.gateway.binding.json`, deux artefacts qui satisfont l'exigence DG-3
pour gateway-content-binding:

- `portal.gateway.headers.txt` contient le bloc complet de headers HTTP (incluant
  `Sora-Name`, `Sora-Content-CID`, `Sora-Proof`, CSP, HSTS, et le descripteur
  `Sora-Route-Binding`) que les gateways edge doivent agrafer a chaque reponse.
- `portal.gateway.binding.json` enregistre la meme information en forme lisible machine
  pour que les tickets de changement et l'automatisation puissent comparer les
  bindings host/cid sans scraper la sortie shell.

Ils sont generes automatiquement via
`cargo xtask soradns-binding-template`
et capturent l'alias, le digest du manifeste et le hostname gateway fournis a
`sorafs-pin-release.sh`. Pour regenerer ou customiser le bloc de headers, lancez:

```bash
cargo xtask soradns-binding-template \
  --manifest artifacts/sorafs/portal.manifest.json \
  --alias docs.sora \
  --hostname docs.sora.link \
  --route-label production \
  --json-out artifacts/sorafs/portal.gateway.binding.json \
  --headers-out artifacts/sorafs/portal.gateway.headers.txt
```

Passez `--csp-template`, `--permissions-template`, ou `--hsts-template` pour override
les templates de headers par defaut quand un deploiement a besoin de directives
supplementaires; combinez-les avec les switches `--no-*` existants pour supprimer
un header completement.

Joignez le snippet de headers a la demande de changement CDN et alimentez le document JSON
au pipeline d'automatisation gateway pour que la promotion d'hote corresponde a
a l'evidence de release.

Le script de release execute automatiquement le helper de verification pour que les
tickets DG-3 incluent toujours une evidence recente. Re-executez-le manuellement si
vous modifiez le JSON de binding a la main:

```bash
cargo xtask soradns-verify-binding \
  --binding artifacts/sorafs/portal.gateway.binding.json \
  --alias docs.sora.link \
  --hostname docs.sora.link \
  --proof-status ok \
  --manifest-json artifacts/sorafs/portal.manifest.json
```

La commande decode le payload `Sora-Proof` agrafe, verifie que la metadata
`Sora-Route-Binding` correspond au CID du manifeste + hostname, et echoue vite
si un header derive. Archivez la sortie console avec les autres artefacts de
deployment quand vous lancez la commande hors CI pour que les reviewers DG-3
aient la preuve que le binding a ete valide avant le cutover.

> **Integration du descripteur DNS:** `portal.dns-cutover.json` embarque maintenant
> une section `gateway_binding` pointant vers ces artefacts (chemins, content CID,
> statut du proof et template de headers literal) **et** une strophe `route_plan`
> qui reference `gateway.route_plan.json` ainsi que les templates de headers
> principal et rollback. Incluez ces blocs dans chaque ticket DG-3 pour que les
> reviewers puissent comparer les valeurs exactes `Sora-Name/Sora-Proof/CSP` et
> confirmer que les plans promotion/rollback correspondent au bundle d'evidence
> sans ouvrir l'archive build.

## Etape 9 - Executer les monitors de publication

L'item roadmap **DOCS-3c** requiert une evidence continue que le portail, le proxy Try it et
les bindings gateway restent sains apres un release. Executez le monitor consolide
juste apres les Etapes 7-8 et branchez-le a vos probes programmes:

```bash
cd docs/portal
npm run monitor:publishing -- \
  --config ../../configs/docs_monitor.json \
  --json-out ../../artifacts/sorafs/${RELEASE_TAG}/monitoring/summary-$(date -u +%Y%m%dT%H%M%SZ).json \
  --evidence-dir ../../artifacts/sorafs/${RELEASE_TAG}/monitoring
```

- `scripts/monitor-publishing.mjs` charge le fichier config (voir
  `docs/portal/docs/devportal/publishing-monitoring.md` pour le schema) et
  execute trois checks: probes de paths portail + validation CSP/Permissions-Policy,
  probes proxy Try it (optionnellement via son endpoint `/metrics`), et le verificateur
  de binding gateway (`cargo xtask soradns-verify-binding`) qui exige maintenant la
  presence et la valeur attendue de Sora-Content-CID avec les checks alias/manifeste.
- La commande retourne non-zero quand un probe echoue pour que CI, cron jobs, ou
  operateurs de runbook puissent arreter un release avant de promouvoir des alias.
- Passer `--json-out` ecrit un resume JSON unique avec le statut par cible; `--evidence-dir`
  emet `summary.json`, `portal.json`, `tryit.json`, `binding.json`, et `checksums.sha256`
  pour que les reviewers governance puissent comparer les resultats sans relancer les monitors.
  Archivez ce repertoire sous `artifacts/sorafs/<tag>/monitoring/` avec le bundle Sigstore
  et le descripteur DNS.
- Incluez la sortie du monitor, l'export Grafana (`dashboards/grafana/docs_portal.json`),
  et l'ID de drill Alertmanager dans le ticket release pour que le SLO DOCS-3c soit
  auditable plus tard. Le playbook de monitoring publishing dedie vit a
  `docs/portal/docs/devportal/publishing-monitoring.md`.

Les probes portail requierent HTTPS et rejettent les URLs base `http://` sauf si
`allowInsecureHttp` est defini dans la config monitor; gardez les cibles production/staging
sur TLS et n'activez l'override que pour des previews locales.

Automatisez le monitor via `npm run monitor:publishing` en Buildkite/cron une fois
le portail en ligne. La meme commande, pointee sur les URLs production, alimente les checks
sante continus entre releases.

## Automatisation avec `sorafs-pin-release.sh`

`docs/portal/scripts/sorafs-pin-release.sh` encapsule les Etapes 2-6. Il:

1. archive `build/` dans un tarball deterministe,
2. execute `car pack`, `manifest build`, `manifest sign`, `manifest verify-signature`,
   et `proof verify`,
3. execute optionnellement `manifest submit` (incluant le binding d'alias) quand
   les credentials Torii sont presents, et
4. ecrit `artifacts/sorafs/portal.pin.report.json`, le optionnel
  `portal.pin.proposal.json`, le descripteur DNS (apres submission),
  et le bundle de binding gateway (`portal.gateway.binding.json` plus le bloc de headers)
  pour que les equipes governance, networking et ops puissent comparer l'evidence
  sans fouiller les logs CI.

Definissez `PIN_ALIAS`, `PIN_ALIAS_NAMESPACE`, `PIN_ALIAS_NAME`, et (optionnellement)
`PIN_ALIAS_PROOF_PATH` avant d'invoquer le script. Utilisez `--skip-submit` pour des dry
runs; le workflow GitHub ci-dessous bascule ceci via l'input `perform_submit`.

## Etape 8 - Publier les specs OpenAPI et bundles SBOM

DOCS-7 requiert que le build du portail, la spec OpenAPI et les artefacts SBOM traversent
le meme pipeline deterministe. Les helpers existants couvrent les trois:

1. **Regenerer et signer la spec.**

   ```bash
   npm run sync-openapi -- --version=2025-q3 --mirror=current --latest
   cargo xtask openapi --sign docs/portal/static/openapi/manifest.json
   ```

   Fournissez un label release via `--version=<label>` quand vous voulez preserver un snapshot
   historique (par exemple `2025-q3`). Le helper ecrit le snapshot dans
   `static/openapi/versions/<label>/torii.json`, le miroir dans
   `versions/current`, et enregistre la metadata (SHA-256, statut du manifeste, timestamp
   mis a jour) dans `static/openapi/versions.json`. Le portail developpeur lit cet index
   pour que les panels Swagger/RapiDoc presentent un picker de version et affichent le
   digest/la signature associee inline. Omettre `--version` conserve les labels du release
   precedent et ne rafraichit que les pointeurs `current` + `latest`.

   Le manifeste capture les digests SHA-256/BLAKE3 pour que la gateway puisse agrafer des
   headers `Sora-Proof` pour `/reference/torii-swagger`.

2. **Emettre les SBOMs CycloneDX.** Le pipeline release attend deja des SBOMs syft
   selon `docs/source/sorafs_release_pipeline_plan.md`. Gardez la sortie
   pres des artefacts de build:

   ```bash
   syft dir:build -o json > "$OUT"/portal.sbom.json
   syft file:docs/portal/static/openapi/torii.json -o json > "$OUT"/openapi.sbom.json
   ```

3. **Empaqueter chaque payload en CAR.**

   ```bash
   sorafs_cli car pack \
     --input docs/portal/static/openapi \
     --car-out "$OUT"/openapi.car \
     --plan-out "$OUT"/openapi.plan.json \
     --summary-out "$OUT"/openapi.car.json

   sorafs_cli car pack \
     --input "$OUT"/portal.sbom.json \
     --car-out "$OUT"/portal.sbom.car \
     --plan-out "$OUT"/portal.sbom.plan.json \
     --summary-out "$OUT"/portal.sbom.car.json
   ```

   Suivez les memes etapes `manifest build` / `manifest sign` que le site principal,
   en ajustant les aliases par artefact (par exemple, `docs-openapi.sora` pour la spec et
   `docs-sbom.sora` pour le bundle SBOM signe). Garder des aliases distincts maintient
   SoraDNS, GARs et tickets rollback limites au payload exact.

4. **Submit et bind.** Reutilisez l'autorite existante et le bundle Sigstore, mais enregistrez
   le tuple d'alias dans le checklist release pour que les auditeurs puissent suivre quel
   nom Sora mappe vers quel digest manifeste.

Archiver les manifestes spec/SBOM avec le build portail assure que chaque ticket release
contient le set complet d'artefacts sans re-executer le packer.

### Helper d'automatisation (CI/package script)

`./ci/package_docs_portal_sorafs.sh` codifie les Etapes 1-8 pour que l'item roadmap
**DOCS-7** soit executable avec une seule commande. Le helper:

- execute la preparation requise du portail (`npm ci`, sync OpenAPI/norito, tests widgets);
- emet les CARs et paires manifeste du portail, OpenAPI et SBOM via `sorafs_cli`;
- execute optionnellement `sorafs_cli proof verify` (`--proof`) et la signature Sigstore
  (`--sign`, `--sigstore-provider`, `--sigstore-audience`);
- depose tous les artefacts sous `artifacts/devportal/sorafs/<timestamp>/` et
  ecrit `package_summary.json` pour que CI/outillage release puisse ingerer le bundle; et
- rafraichit `artifacts/devportal/sorafs/latest` pour pointer vers la derniere execution.

Exemple (pipeline complet avec Sigstore + PoR):

```bash
./ci/package_docs_portal_sorafs.sh \
  --proof \
  --sign \
  --sigstore-provider=github-actions \
  --sigstore-audience=sorafs-devportal
```

Flags utiles:

- `--out <dir>` - override du root des artefacts (default conserve les dossiers timestamp).
- `--skip-build` - reutilise un `docs/portal/build` existant (utile quand CI ne peut pas
  reconstruire a cause de mirrors offline).
- `--skip-sync-openapi` - ignore `npm run sync-openapi` quand `cargo xtask openapi`
  ne peut pas joindre crates.io.
- `--skip-sbom` - evite d'appeler `syft` quand le binaire n'est pas installe (le script logge
  un avertissement a la place).
- `--proof` - execute `sorafs_cli proof verify` pour chaque paire CAR/manifeste. Les payloads
  multi-fichiers exigent encore le support chunk-plan dans le CLI, donc laissez ce flag
  desactive si vous rencontrez des erreurs `plan chunk count` et verifiez manuellement
  une fois le gate upstream livre.
- `--sign` - invoque `sorafs_cli manifest sign`. Fournissez un token via
  `SIGSTORE_ID_TOKEN` (ou `--sigstore-token-env`) ou laissez le CLI le recuperer via
  `--sigstore-provider/--sigstore-audience`.

Pour les artefacts production, utilisez `docs/portal/scripts/sorafs-pin-release.sh`.
Il empaquete maintenant le portail, OpenAPI et SBOM, signe chaque manifeste, et enregistre
la metadata assets supplementaire dans `portal.additional_assets.json`. Le helper
comprend les memes knobs optionnels que le packager CI plus les nouveaux switches
`--openapi-*`, `--portal-sbom-*`, et `--openapi-sbom-*` pour assigner des tuples d'alias
par artefact, override de la source SBOM via `--openapi-sbom-source`, sauter certains
payloads (`--skip-openapi`/`--skip-sbom`), et pointer vers un binaire `syft` non default
avec `--syft-bin`.

Le script affiche chaque commande executee; copiez le log dans le ticket release
avec `package_summary.json` pour que les reviewers puissent comparer les digests CAR,
les metadata de plan, et les hashes du bundle Sigstore sans parcourir une sortie shell ad hoc.

## Etape 9 - Verification gateway + SoraDNS

Avant d'annoncer un cutover, prouvez que le nouvel alias resol via SoraDNS et que les gateways
agraffent des proofs frais:

1. **Executer le gate probe.** `ci/check_sorafs_gateway_probe.sh` exerce
   `cargo xtask sorafs-gateway-probe` contre les fixtures demo dans
   `fixtures/sorafs_gateway/probe_demo/`. Pour des deploiements reels, pointez le probe
   vers le hostname cible:

   ```bash
   ./ci/check_sorafs_gateway_probe.sh -- \
     --gateway "https://docs.sora/.well-known/sorafs/manifest" \
     --header "Accept: application/json" \
     --gar fixtures/sorafs_gateway/probe_demo/demo.gar.jws \
     --gar-key "demo-gar=$(<fixtures/sorafs_gateway/probe_demo/gar_pub.hex>)" \
     --host "docs.sora" \
     --report-json artifacts/sorafs_gateway_probe/ci/docs.json
   ```

   Le probe decode `Sora-Name`, `Sora-Proof`, et `Sora-Proof-Status` selon
   `docs/source/sorafs_alias_policy.md` et echoue quand le digest manifeste,
   les TTLs ou les bindings GAR derivent.

   For lightweight spot checks (for example, when only the binding bundle
   changed), run `cargo xtask soradns-verify-binding --binding <portal.gateway.binding.json> --alias "<alias>" --hostname "<gateway-host>" --proof-status ok --manifest-json <portal.manifest.json>`.
   The helper validates the captured binding bundle and is handy for release
   tickets that only need binding confirmation instead of a full probe drill.

2. **Capturer l'evidence de drill.** Pour des drills operateur ou des dry runs PagerDuty,
   enveloppez le probe avec `scripts/telemetry/run_sorafs_gateway_probe.sh --scenario
   devportal-rollout -- ...`. Le wrapper stocke headers/logs sous
   `artifacts/sorafs_gateway_probe/<stamp>/`, met a jour `ops/drill-log.md`, et
   (optionnellement) declenche des hooks rollback ou des payloads PagerDuty. Definissez
   `--host docs.sora` pour valider le chemin SoraDNS au lieu d'une IP hard-codee.

3. **Verifier les bindings DNS.** Quand governance publie le proof alias, enregistrez
   le fichier GAR reference par le probe (`--gar`) et joignez-le a l'evidence release.
   Les owners resolver peuvent rejouer le meme input via `tools/soradns-resolver` pour
   s'assurer que les entrees en cache honorent le nouveau manifeste. Avant de joindre le JSON,
   executez
   `cargo xtask soradns-verify-gar --gar <path> --name <alias> [--manifest-cid <cid>] [--telemetry-label <label>]`
   pour valider offline le mapping host deterministe, la metadata manifeste et les labels
   telemetrie. Le helper peut emettre un resume `--json-out` avec le GAR signe pour que les
   reviewers aient une evidence verifiable sans ouvrir le binaire.
  Quand vous redigez un nouveau GAR, preferez
  `cargo xtask soradns-gar-template --name <alias> --manifest <portal.manifest.json> --telemetry-label <label> ...`
  (revenez a `--manifest-cid <cid>` uniquement quand le fichier manifeste n'est pas
  disponible). Le helper derive maintenant le CID **et** le digest BLAKE3 directement
  depuis le manifest JSON, coupe les espaces, deduplique les flags `--telemetry-label`,
  trie les labels, et emet les templates CSP/HSTS/Permissions-Policy par defaut avant
  d'ecrire le JSON pour que le payload reste deterministe meme si les operateurs
  capturent des labels depuis des shells differents.

4. **Surveiller les metrics alias.** Gardez `torii_sorafs_alias_cache_refresh_duration_ms`
   et `torii_sorafs_gateway_refusals_total{profile="docs"}` a l'ecran pendant le probe;
   ces series sont graphees dans `dashboards/grafana/docs_portal.json`.

## Etape 10 - Monitoring et bundle d'evidence

- **Dashboards.** Exportez `dashboards/grafana/docs_portal.json` (SLOs portail),
  `dashboards/grafana/sorafs_gateway_observability.json` (latence gateway +
  sante proof), et `dashboards/grafana/sorafs_fetch_observability.json`
  (sante orchestrator) pour chaque release. Joignez les exports JSON au ticket release
  pour que les reviewers puissent rejouer les requetes Prometheus.
- **Archives probe.** Conservez `artifacts/sorafs_gateway_probe/<stamp>/` dans git-annex
  ou votre bucket d'evidence. Incluez le resume probe, les headers, et le payload PagerDuty
  capture par le script telemetry.
- **Bundle de release.** Stockez les resumes CAR du portail/SBOM/OpenAPI, les bundles manifestes,
  signatures Sigstore, `portal.pin.report.json`, logs probe Try-It, et reports link-check
  sous un dossier timestamp (par exemple, `artifacts/sorafs/devportal/20260212T1103Z/`).
- **Drill log.** Quand les probes font partie d'un drill, laissez
  `scripts/telemetry/run_sorafs_gateway_probe.sh` ajouter des entrees a `ops/drill-log.md`
  pour que la meme evidence satisfasse le requirement chaos SNNet-5.
- **Liens de ticket.** Referencez les IDs de panel Grafana ou les exports PNG attaches dans le
  ticket de changement, avec le chemin du rapport probe, pour que les reviewers puissent
  recouper les SLOs sans acces shell.

## Etape 11 - Drill fetch multi-source et evidence scoreboard

Publier sur SoraFS requiert maintenant une evidence fetch multi-source (DOCS-7/SF-6)
avec les preuves DNS/gateway ci-dessus. Apres le pin du manifeste:

1. **Lancer `sorafs_fetch` contre le manifeste live.** Utilisez les artefacts plan/manifeste
   produits dans les Etapes 2-3 avec les credentials gateway emis pour chaque provider.
   Persistez toutes les sorties pour que les auditeurs puissent rejouer la trace de decision
   orchestrator:

   ```bash
   OUT=artifacts/sorafs/devportal
   FETCH_OUT="$OUT/fetch/$(date -u +%Y%m%dT%H%M%SZ)"
   mkdir -p "$FETCH_OUT"

   cargo run -p sorafs_car --bin sorafs_fetch -- \
     --plan "$OUT/portal.plan.json" \
     --manifest-json "$OUT/portal.manifest.json" \
     --gateway-provider name=docs-us,provider-id="$DOCS_US_PROVIDER_ID",base-url="$DOCS_US_GATEWAY",stream-token="$DOCS_US_STREAM_TOKEN" \
     --gateway-provider name=docs-eu,provider-id="$DOCS_EU_PROVIDER_ID",base-url="$DOCS_EU_GATEWAY",stream-token="$DOCS_EU_STREAM_TOKEN" \
     --scoreboard-out "$FETCH_OUT/scoreboard.json" \
     --provider-metrics-out "$FETCH_OUT/providers.ndjson" \
     --json-out "$FETCH_OUT/fetch.json" \
     --chunk-receipts-out "$FETCH_OUT/chunk_receipts.ndjson" \
     --telemetry-json artifacts/sorafs/provider_telemetry.json \
     --max-peers=3 \
     --retry-budget=4
   ```

   - Recuperez d'abord les adverts providers references par le manifeste (par exemple
     `sorafs_cli manifest describe --provider-adverts-out artifacts/sorafs/provider_adverts/`)
     et passez-les via `--provider-advert name=path` pour que le scoreboard evalue les
     fenetres de capacite de facon deterministe. Utilisez
     `--allow-implicit-provider-metadata` **uniquement** quand vous rejouez des fixtures en CI;
     les drills production doivent citer les adverts signes qui ont accompagne le pin.
   - Quand le manifeste reference des regions additionnelles, repetez la commande avec les
     tuples providers correspondants pour que chaque cache/alias ait un artefact fetch associe.

2. **Archiver les sorties.** Conservez `scoreboard.json`,
   `providers.ndjson`, `fetch.json`, et `chunk_receipts.ndjson` sous le dossier evidence
   du release. Ces fichiers capturent le weighting peers, le retry budget, la latence EWMA
   et les receipts par chunk que le paquet governance doit retenir pour SF-7.

3. **Mettre a jour la telemetrie.** Importez les sorties fetch dans le dashboard
   **SoraFS Fetch Observability** (`dashboards/grafana/sorafs_fetch_observability.json`),
   surveillez `torii_sorafs_fetch_duration_ms`/`_failures_total` et les panels de range provider
   pour les anomalies. Liez les snapshots Grafana au ticket release avec le chemin scoreboard.

4. **Tester les regles d'alertes.** Executez `scripts/telemetry/test_sorafs_fetch_alerts.sh`
   pour valider le bundle d'alertes Prometheus avant de fermer le release. Joignez la sortie
   promtool au ticket pour que les reviewers DOCS-7 confirment que les alertes de stall et
   slow-provider restent armees.

5. **Brancher en CI.** Le workflow de pin portail garde une etape `sorafs_fetch` derriere
   l'input `perform_fetch_probe`; activez-le pour les runs staging/production afin que
   l'evidence fetch soit produite avec le bundle manifeste sans intervention manuelle.
   Les drills locaux peuvent reutiliser le meme script en exportant les tokens gateway et
   en definissant `PIN_FETCH_PROVIDERS` a la liste de providers separee par virgules.

## Promotion, observabilite et rollback

1. **Promotion:** gardez des alias distincts pour staging et production. Promouvez en
   re-executant `manifest submit` avec le meme manifeste/bundle, en changeant
   `--alias-namespace/--alias-name` pour pointer vers l'alias production. Cela evite
   de reconstruire ou re-signer une fois que QA approuve le pin staging.
2. **Monitoring:** importez le dashboard pin-registry
   (`docs/source/grafana_sorafs_pin_registry.json`) plus les probes portail specifques
   (voir `docs/portal/docs/devportal/observability.md`). Alertez sur drift de checksums,
   probes echoues ou pics de retry proof.
3. **Rollback:** pour revenir en arriere, resoumettez le manifeste precedent (ou retirez
   l'alias courant) avec `sorafs_cli manifest submit --alias ... --retire`.
   Gardez toujours le dernier bundle connu comme bon et le resume CAR pour que les
   proofs rollback puissent etre recrees si les logs CI tournent.

## Modele de workflow CI

Au minimum, votre pipeline doit:

1. Build + lint (`npm ci`, `npm run build`, generation des checksums).
2. Empaqueter (`car pack`) et calculer les manifestes.
3. Signer via le token OIDC du job (`manifest sign`).
4. Uploader les artefacts (CAR, manifeste, bundle, plan, resumes) pour audit.
5. Soumettre au pin registry:
   - Pull requests -> `docs-preview.sora`.
   - Tags / branches protegees -> promotion alias production.
6. Executer les probes + gates de verification proof avant de terminer.

`.github/workflows/docs-portal-sorafs-pin.yml` connecte toutes ces etapes pour les releases manuelles.
Le workflow:

- build/teste le portail,
- empaquete le build via `scripts/sorafs-pin-release.sh`,
- signe/verifie le bundle manifeste via GitHub OIDC,
- upload le CAR/manifeste/bundle/plan/resumes comme artifacts, et
- (optionnellement) soumet le manifeste + alias binding quand les secrets sont presents.

Configurez les secrets/variables suivants avant de declencher le job:

| Name | But |
| --- | --- |
| `DOCS_SORAFS_TORII_URL` | Host Torii qui expose `/v1/sorafs/pin/register`. |
| `DOCS_SORAFS_SUBMITTED_EPOCH` | Identifiant d'epoch enregistre avec les submissions. |
| `DOCS_SORAFS_AUTHORITY` / `DOCS_SORAFS_PRIVATE_KEY` | Autorite de signature pour la soumission manifeste. |
| `DOCS_SORAFS_ALIAS_NAMESPACE` / `DOCS_SORAFS_ALIAS_NAME` | Tuple alias lie au manifeste quand `perform_submit` est `true`. |
| `DOCS_SORAFS_ALIAS_PROOF_B64` | Bundle proof alias encode en base64 (optionnel; omettre pour sauter alias binding). |
| `DOCS_ANALYTICS_*` | Endpoints analytics/probe existants reutilises par d'autres workflows. |

Declenchez le workflow via l'UI Actions:

1. Fournissez `alias_label` (ex: `docs.sora.link`), un `proposal_alias` optionnel,
   et un override `release_tag` optionnel.
2. Laissez `perform_submit` unchecked pour generer les artefacts sans toucher Torii
   (utile pour dry runs) ou activez-le pour publier directement sur l'alias configure.

`docs/source/sorafs_ci_templates.md` documente encore les helpers CI generiques pour
les projets hors repo, mais le workflow portail doit etre la voie preferee
pour les releases quotidiennes.

## Checklist

- [ ] `npm run build`, `npm run test:*`, et `npm run check:links` sont verts.
- [ ] `build/checksums.sha256` et `build/release.json` captures dans les artefacts.
- [ ] CAR, plan, manifeste, et resume generes sous `artifacts/`.
- [ ] Bundle Sigstore + signature detachee stockes avec les logs.
- [ ] `portal.manifest.submit.summary.json` et `portal.manifest.submit.response.json`
      captures quand des submissions ont lieu.
- [ ] `portal.pin.report.json` (et optionnel `portal.pin.proposal.json`)
      archive avec les artefacts CAR/manifeste.
- [ ] Logs `proof verify` et `manifest verify-signature` archives.
- [ ] Dashboards Grafana mis a jour + probes Try-It reussis.
- [ ] Notes rollback (ID manifeste precedent + digest alias) jointes au
      ticket release.
