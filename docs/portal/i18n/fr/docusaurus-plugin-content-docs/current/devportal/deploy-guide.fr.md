---
lang: fr
direction: ltr
source: docs/portal/docs/devportal/deploy-guide.fr.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

## Vue d'ensemble

Ce playbook convertit les éléments du roadmap **DOCS-7** (publication SoraFS) et **DOCS-8**
(automatisation du pin CI/CD) en une procédure actionnable pour le développeur du portail.
Il couvre la phase build/lint, l'empaquetage SoraFS, la signature de manifestes avec Sigstore,
la promotion d'alias, la vérification et les exercices de rollback pour que chaque aperçu et sortie
soit reproductible et auditable.

Le flux suppose que vous avez le binaire `sorafs_cli` (compile avec `--features cli`), l'accès à
un point de terminaison Torii avec des autorisations de pin-registry, et des informations d'identification OIDC pour Sigstore.
Stockez les secrets longue durée (`IROHA_PRIVATE_KEY`, `SIGSTORE_ID_TOKEN`, tokens Torii) dans
votre coffre CI; les exécutions locales peuvent les charger via des exports du shell.

## Prérequis- Nœud 18.18+ avec `npm` ou `pnpm`.
- `sorafs_cli` via `cargo run -p sorafs_car --features cli --bin sorafs_cli`.
- URL Torii qui expose `/v1/sorafs/*` plus un compte/cle privé d'autorité qui peut soumettre
  des manifestes et des alias.
- Emmeteur OIDC (GitHub Actions, GitLab, Workload Identity, etc.) pour émettre un `SIGSTORE_ID_TOKEN`.
- Optionnel : `examples/sorafs_cli_quickstart.sh` pour des essais à sec et
  `docs/source/sorafs_ci_templates.md` pour l'échafaudage des workflows GitHub/GitLab.
- Configurez les variables OAuth Try it (`DOCS_OAUTH_*`) et exécutez la
  [liste de contrôle de renforcement de la sécurité](./security-hardening.md) avant de promouvoir un build
  hors du laboratoire. Le build du portail fait écho maintenant lorsque ces variables manquent
  ou lorsque les boutons TTL/polling sortent des fenêtres imposées; exportez
  `DOCS_OAUTH_ALLOW_INSECURE=1` uniquement pour des avant-premières locales jetables. Joignez
  les preuves de pen-test au ticket de release.

## Etape 0 - Capturer un bundle du proxy Essayez-le

Avant de promouvoir un aperçu vers Netlify ou la gateway, figez les sources du proxy Essayez-le
et le digest du manifeste OpenAPI signé dans un bundle déterministe :

```bash
cd docs/portal
npm run release:tryit-proxy -- \
  --out ../../artifacts/tryit-proxy/$(date -u +%Y%m%dT%H%M%SZ) \
  --target https://torii.dev.sora \
  --label preview-2026-02-14
````scripts/tryit-proxy-release.mjs` copier les helpers proxy/probe/rollback, vérifier la signature
OpenAPI et écrit `release.json` plus `checksums.sha256`. Joignez ce bundle au ticket de
promotion Netlify/SoraFS gateway pour que les reviewers puissent rejouer les sources exactes
et les astuces du target Torii sans reconstruire. Le bundle enregistre aussi si les porteurs
les fournis par le client étaient actifs (`allow_client_auth`) pour garder le plan de déploiement
et les règles CSP en phase.

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

`npm run build` exécute automatiquement `scripts/write-checksums.mjs`, produisant :

- `build/checksums.sha256` - manifeste SHA256 pour `sha256sum -c`.
- `build/release.json` - métadonnées (`tag`, `generated_at`, `source`) corrigées dans chaque CAR/manifeste.

Archivez ces fichiers avec le CV CAR pour que les reviewers puissent comparer les artefacts
aperçu sans reconstruction.

## Etape 2 - Empaqueter les actifs statiques

Exécutez le packer CAR contre le répertoire de sortie Docusaurus. L'exemple ci-dessous écrit
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

Le CV JSON capture les comptes de chunks, les résumés et les conseils de planification de preuve
que `manifest build` et les tableaux de bord CI réutilisent plus tard.

## Etape 2b - Empaqueter les compagnons OpenAPI et SBOMDOCS-7 nécessite de publier le site du portail, le snapshot OpenAPI et les payloads SBOM
comme des manifestes distincts pour que les gateways puissent agrafer les headers
`Sora-Proof`/`Sora-Content-CID` pour chaque artefact. L'assistant de libération
(`scripts/sorafs-pin-release.sh`) empaquete déjà le répertoire OpenAPI
(`static/openapi/`) et les SBOMs emis via `syft` dans des CARs separes
`openapi.*`/`*-sbom.*` et enregistrer les métadonnées dans
`artifacts/sorafs/portal.additional_assets.json`. En flux manuel,
répétez les Etapes 2-4 pour chaque payload avec ses propres préfixes et labels métadonnées
(par exemple `--car-out "$OUT"/openapi.car` plus
`--metadata alias_label=docs.sora.link/openapi`). enregistrer chaque paire manifeste/alias
sur Torii (site, OpenAPI, portail SBOM, SBOM OpenAPI) avant de basculer le DNS pour que
la passerelle sert des preuves agrafees pour tous les artefacts publics.

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

Ajustez les flags de pin-policy selon votre fenêtre de release (par exemple, `--pin-storage-class
hot` pour les canaris). La variante JSON est optionnelle mais pratique pour la révision de code.

## Etape 4 - Signer avec Sigstore

```bash
sorafs_cli manifest sign \
  --manifest "$OUT"/portal.manifest.to \
  --chunk-plan "$OUT"/portal.plan.json \
  --bundle-out "$OUT"/portal.manifest.bundle.json \
  --signature-out "$OUT"/portal.manifest.sig \
  --identity-token-provider github-actions \
  --identity-token-audience sorafs-devportal
```Le bundle enregistre le digest du manifeste, les digests de chunks et un hash BLAKE3 du token
OIDC sans persister le JWT. Gardez le bundle et la signature détachée ; les promotions de
la production peut réutiliser les mèmes artefacts au lieu de re-signer. Les exécutions locales
peuvent remplacer les flags supplier par `--identity-token-env` (ou définir `SIGSTORE_ID_TOKEN`
) quand un helper OIDC externe emet le token.

## Etape 5 - Soumettre au pin registre

Soumettez le manifeste signe (et le plan de chunks) a Torii. Demandez toujours un CV pour
que l'entrée/alias resulte soit auditable.

```bash
sorafs_cli manifest submit \
  --manifest "$OUT"/portal.manifest.to \
  --chunk-plan "$OUT"/portal.plan.json \
  --torii-url "$TORII_URL" \
  --authority i105... \
  --private-key "$IROHA_PRIVATE_KEY" \
  --submitted-epoch 20260101 \
  --alias-namespace docs \
  --alias-name sora.link \
  --alias-proof "$OUT"/docs.alias.proof \
  --summary-out "$OUT"/portal.submit.json \
  --response-out "$OUT"/portal.submit.response.json
```

Lors d'un déploiement de preview ou de canary (`docs-preview.sora`), répétez la soumission
avec un alias unique pour que QA puisse vérifier le contenu avant la production de la promotion.

La liaison d'alias requiert trois champs : `--alias-namespace`, `--alias-name` et `--alias-proof`.
Governance produit le bundle de preuve (base64 ou bytes Norito) lorsque la demande d'alias est
approuver; stockez-le dans les secrets CI et exposez-le comme fichier avant d'invoquer
`manifest submit`. Laissez les flags d'alias vidéos quand vous voulez seulement pinner
le manifeste sans toucher au DNS.

## Etape 5b - Générer une proposition de gouvernanceChaque manifeste doit voyager avec une proposition prête pour le Parlement afin que tout citoyen
Sora pourra introduire le changement sans emprunter des privilèges d’identification. Après les
etapes soumettre/signer, lancez :

```bash
sorafs_cli manifest proposal \
  --manifest "$OUT"/portal.manifest.to \
  --chunk-plan "$OUT"/portal.plan.json \
  --submitted-epoch 20260101 \
  --alias-hint docs.sora.link \
  --proposal-out "$OUT"/portal.pin.proposal.json
```

`portal.pin.proposal.json` capture l'instruction canonique `RegisterPinManifest`,
le digest de chunks, la politique et l'indice d'alias. Joignez-le au ticket de gouvernance ou au
portail Parliament pour que les délégués puissent comparer le payload sans reconstruire.
Comme la commande ne touche jamais la cle d'autorité Torii, tout citoyen peut rediger la
proposition localement.

## Etape 6 - Vérifier les preuves et la télémétrie

Après le pin, exécutez les étapes de vérification déterministe :

```bash
sorafs_cli proof verify \
  --manifest "$OUT"/portal.manifest.to \
  --car "$OUT"/portal.car \
  --summary-out "$OUT"/portal.proof.json

sorafs_cli manifest verify-signature \
  --manifest "$OUT"/portal.manifest.to \
  --bundle "$OUT"/portal.manifest.bundle.json \
  --chunk-plan "$OUT"/portal.plan.json
```- Surveillez `torii_sorafs_gateway_refusals_total` et
  `torii_sorafs_replication_sla_total{outcome="missed"}` pour les anomalies.
- Executez `npm run probe:portal` pour exécuter le proxy Try-It et les liens enregistrés
  contre le contenu fraîchement pinné.
- Capturez l'évidence de surveillance décrite dans
  [Publishing & Monitoring](./publishing-monitoring.md) pour que la porte d'observabilité
  DOCS-3c soit satisfaite des étapes de publication. Le helper accepte maintenant
  plusieurs entrées `bindings` (site, OpenAPI, SBOM portail, SBOM OpenAPI) et appliquer
  `Sora-Name`/`Sora-Proof`/`Sora-Content-CID` sur l'hôte cible via le garde optionnel
  `hostname`. L'invocation ci-dessous écrit un CV JSON unique et le bundle d'evidence
  (`portal.json`, `tryit.json`, `binding.json`, `checksums.sha256`) sous le répertoire
  de libération:

  ```bash
  npm run monitor:publishing -- \
    --config ../../configs/docs_monitor.json \
    --json-out ../../artifacts/sorafs/preview-2026-02-14/monitoring/summary.json \
    --evidence-dir ../../artifacts/sorafs/preview-2026-02-14/monitoring
  ```

## Etape 6a - Planifier les certificats du gateway

Dérivez le plan SAN/challenge TLS avant de créer les paquets GAR pour que l'équipe gateway et
les approbateurs DNS examinent la même preuve. Le nouveau helper reflète les entrées DG-3
en énumérant les hôtes wildcard canoniques, les SANs Pretty-Host, les labels DNS-01 et
les challenges ACME recommande :

```bash
cargo xtask soradns-acme-plan \
  --name docs.sora \
  --json-out artifacts/sorafs/portal.acme-plan.json
```Commitez le JSON avec le bundle de release (ou téléchargez-le avec le ticket de changement) pour
que les opérateurs peuvent coller les valeurs SAN dans la configuration
`torii.sorafs_gateway.acme` de Torii et que les reviewers GAR peuvent confirmer les
mappings canonique/pretty sans re-executer les dérivations d'hôtes. Ajouter des arguments
`--name` supplémentaires pour chaque suffixe promu dans le meme release.

## Etape 6b - Dériver les mappings d'hôtes canoniques

Avant de templater les payloads GAR, enregistrez le mapping d'hôte déterministe pour chaque alias.
`cargo xtask soradns-hosts` hashe chaque `--name` en son label canonique
(`<base32>.gw.sora.id`), emet le joker requis (`*.gw.sora.id`) et dérive le joli hôte
(`<alias>.gw.sora.name`). Persistez la sortie dans les artefacts de release pour que les
les examinateurs DG-3 peuvent comparer le mapping avec la soumission GAR :

```bash
cargo xtask soradns-hosts \
  --name docs.sora \
  --json-out artifacts/sorafs/portal.canonical-hosts.json
```

Utilisez `--verify-host-patterns <file>` pour echouer vite lorsqu'un GAR ou un JSON de
la passerelle de liaison met en place les hôtes requis. Le helper accepte plusieurs fichiers de vérification,
ce qui facilite le lint du template GAR et du `portal.gateway.binding.json` agrafe dans
la mème invocation :

```bash
cargo xtask soradns-hosts \
  --name docs.sora \
  --json-out artifacts/sorafs/portal.canonical-hosts.json \
  --verify-host-patterns artifacts/sorafs/portal.gar.json \
  --verify-host-patterns artifacts/sorafs/portal.gateway.binding.json
```Joignez le JSON de CV et le log de vérification au ticket de changement DNS/gateway pour que
les auditeurs peuvent confirmer les hôtes canoniques, wildcard et jolie sans ré-exécuter les
scripts. Re-executez la commande quand de nouveaux alias sont ajoutés au bundle pour que les
met à jour GAR heritent de la meme évidence.

## Etape 7 - Générer le descripteur de cutover DNS

Les changements de production nécessitent un paquet de changement vérifiable. Après une soumission
reussie (binding d'alias), le helper emet
`artifacts/sorafs/portal.dns-cutover.json`, capturant :- métadonnées du bind d'alias (namespace/name/proof, digest manifeste, URL Torii,
  époque soumis, autorité);
- contexte de release (tag, alias label, chemins manifeste/CAR, plan de chunks, bundle Sigstore) ;
- pointeurs de vérification (commande sonde, alias + endpoint Torii) ; et
- champs optionnels de change-control (id ticket, fenetre cutover, contact ops,
  production de nom d'hôte/zone);
- métadonnées de promotion de route dérivée du header `Sora-Route-Binding`
  (hôte canonique/CID, chemins de header + liaison, commandes de vérification), assurant que la
  promotion GAR et les exercices de repli référençant la meme évidence;
- les artefacts de genres de plan de route (`gateway.route_plan.json`,
  templates de headers et headers rollback optionnels) pour que les tickets de changement et
  hooks de lint CI peut vérifier que chaque paquet DG-3 référence les plans de
  promotion/rollback canoniques avant l'approbation;
- métadonnées optionnelles d'invalidation de cache (endpoint de purge, variable auth, payload JSON,
  et exemple de commande `curl`); et
- des conseils de rollback pointant vers le descripteur précédent (tag de release et digest manifeste)
  pour que les tickets capturent un chemin de repli déterministe.

Lorsque la release requiert des purges de cache, générer un plan canonique avec le descripteur :

```bash
cargo xtask soradns-cache-plan \
  --name docs.sora \
  --path / \
  --path /gateway/manifest.json \
  --auth-header Authorization \
  --auth-env CACHE_PURGE_TOKEN \
  --json-out artifacts/sorafs/portal.cache_plan.json
```Joignez le `portal.cache_plan.json` au paquet DG-3 pour que les opérateurs fournissant des hôtes/paths
 déterministes (et les astuces auth correspondants) lorsqu'ils emettent des requêtes `PURGE`.
La section cache optionnelle du descripteur peut référencer ce fichier directement, le garder
les reviewers alines sur les endpoints purgent exactement lors d'un basculement.

Chaque paquet DG-3 a également besoin d'un checklist promotion + rollback. Générez-le via
`cargo xtask soradns-route-plan` pour que les reviewers puissent tracer les étapes exactes
preflight, cutover et rollback par alias :

```bash
cargo xtask soradns-route-plan \
  --name docs.sora \
  --json-out artifacts/sorafs/gateway.route_plan.json
```

Le `gateway.route_plan.json` emis capture hosts canoniques/pretty, rappels de health-check
par étape, mises à jour de liaison GAR, purges de cache et actions de rollback. Joignez-le-aux
artefacts GAR/binding/cutover avant de soumettre le ticket de changement pour que Ops puisse
répéter et valider les mèmes étapes scripts.

`scripts/generate-dns-cutover-plan.mjs` alimente ce descripteur et s'exécute automatiquement depuis
`sorafs-pin-release.sh`. Pour le régénérer ou le personnaliser manuellement :

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

Renseignez les métadonnées optionnelles via des variables d'environnement avant d'exécuter le helper de pin :| Variables | Mais |
| --- | --- |
| `DNS_CHANGE_TICKET` | ID du stock de billets dans le descripteur. |
| `DNS_CUTOVER_WINDOW` | Fenêtre de coupe ISO8601 (ex : `2026-03-21T15:00Z/2026-03-21T15:30Z`). |
| `DNS_HOSTNAME`, `DNS_ZONE` | Production de nom d'hôte + zone faisant autorité. |
| `DNS_OPS_CONTACT` | Alias ​​de garde ou contact d'escalade. |
| `DNS_CACHE_PURGE_ENDPOINT` | Endpoint purge cache enregistré dans le descripteur. |
| `DNS_CACHE_PURGE_AUTH_ENV` | Env var contenant le token purge (par défaut : `CACHE_PURGE_TOKEN`). |
| `DNS_PREVIOUS_PLAN` | Chemin vers le descripteur précédent pour le rollback des métadonnées. |

Joignez le JSON au review de changement DNS pour que les approbateurs puissent vérifier les digests
manifestes, liaisons d'alias et commandes sonde sans fouiller les logs CI. Les drapeaux CLI
`--dns-change-ticket`, `--dns-cutover-window`, `--dns-hostname`,
`--dns-zone`, `--ops-contact`, `--cache-purge-endpoint`,
`--cache-purge-auth-env`, et `--previous-dns-plan` fournissent les memes overrides
quand le helper tourne hors CI.

## Etape 8 - Emettre le squelette de zonefile du solver (optionnel)Quand la fenêtre de production cutover est connue, le script de release peut émettre
le squelette de zonefile SNS et le snippet solver automatiquement. Passez les
enregistre les souhaits DNS et les métadonnées via des variables d'environnement ou des options CLI; l'assistant
appelle `scripts/sns_zonefile_skeleton.py` immédiatement après génération du descripteur.
Fournissez au moins une valeur A/AAAA/CNAME et le digest GAR (BLAKE3-256 du payload GAR signe).
Si la zone/hostname est connue et `--dns-zonefile-out` est omis, le helper écrit dans
`artifacts/sns/zonefiles/<zone>/<hostname>.json` et remplit
`ops/soradns/static_zones.<hostname>.json` comme résolveur d'extrait de code.| Variable/indicateur | Mais |
| --- | --- |
| `DNS_ZONEFILE_OUT`, `--dns-zonefile-out` | Chemin du squelette zonefile genere. |
| `DNS_ZONEFILE_RESOLVER_SNIPPET`, `--dns-zonefile-resolver-snippet` | Chemin du résolveur d'extraits de code (par défaut : `ops/soradns/static_zones.<hostname>.json` si vous le souhaitez). |
| `DNS_ZONEFILE_TTL`, `--dns-zonefile-ttl` | TTL applique aux records generes (par défaut : 600 secondes). |
| `DNS_ZONEFILE_IPV4`, `--dns-zonefile-ipv4` | Adresses IPv4 (env séparés par virgules ou flag CLI répétable). |
| `DNS_ZONEFILE_IPV6`, `--dns-zonefile-ipv6` | Adresses IPv6. |
| `DNS_ZONEFILE_CNAME`, `--dns-zonefile-cname` | Ciblez l'option CNAME. |
| `DNS_ZONEFILE_SPKI`, `--dns-zonefile-spki-pin` | Broches SPKI SHA-256 (base64). |
| `DNS_ZONEFILE_TXT`, `--dns-zonefile-txt` | Entrées TXT additionnelles (`key=value`). |
| `DNS_ZONEFILE_VERSION`, `--dns-zonefile-version` | Override du label de version zonefile calculé. |
| `DNS_ZONEFILE_EFFECTIVE_AT`, `--dns-zonefile-effective-at` | Forcez le timestamp `effective_at` (RFC3339) au lieu du début de la fenêtre de basculement. |
| `DNS_ZONEFILE_PROOF`, `--dns-zonefile-proof` | Override du proof literal enregistré dans les métadonnées. |
| `DNS_ZONEFILE_CID`, `--dns-zonefile-cid` | Override du CID enregistré dans les métadonnées. |
| `DNS_ZONEFILE_FREEZE_STATE`, `--dns-zonefile-freeze-state` | Etat de gel gardien (doux, dur, décongélation, surveillance, urgence). |
| `DNS_ZONEFILE_FREEZE_TICKET`, `--dns-zonefile-freeze-ticket` | Billet de référence gardien/conseil pour geler. |
| `DNS_ZONEFILE_FREEZE_EXPIRES_AT`, `--dns-zonefile-freeze-expires-at` | Horodatage RFC3339 pour la décongélation. || `DNS_ZONEFILE_FREEZE_NOTES`, `--dns-zonefile-freeze-note` | Notes freeze additionnelles (env séparées par virgules ou flag répétable). |
| `DNS_GAR_DIGEST`, `--dns-gar-digest` | Digest BLAKE3-256 (hex) du payload GAR signé. Requis lorsque les liaisons gateway sont présentes. |

Le workflow GitHub Actions lit ces valeurs depuis les secrets du repo pour que chaque pin production
emet automatiquement les artefacts zonefile. Configurez les secrets suivants (les valeurs peuvent
contient des listes séparées par virgules pour les champs multivalues):

| Secrets | Mais |
| --- | --- |
| `DOCS_SORAFS_DNS_HOSTNAME`, `DOCS_SORAFS_DNS_ZONE` | La production du nom d'hôte/de la zone passe au helper. |
| `DOCS_SORAFS_DNS_OPS_CONTACT` | Alias ​​de garde stocke dans le descripteur. |
| `DOCS_SORAFS_ZONEFILE_IPV4`, `DOCS_SORAFS_ZONEFILE_IPV6` | Enregistre IPv4/IPv6 à publier. |
| `DOCS_SORAFS_ZONEFILE_CNAME` | Ciblez l'option CNAME. |
| `DOCS_SORAFS_ZONEFILE_SPKI` | Broches SPKI base64. |
| `DOCS_SORAFS_ZONEFILE_TXT` | Entrées TXT additionnelles. |
| `DOCS_SORAFS_ZONEFILE_FREEZE_STATE/TICKET/EXPIRES_AT/NOTES` | Metadata freeze enregistré dans le squelette. |
| `DOCS_SORAFS_GAR_DIGEST` | Digest BLAKE3 en hex du payload GAR signe. |

En déclenchant `.github/workflows/docs-portal-sorafs-pin.yml`, fournissez les entrées
`dns_change_ticket` et `dns_cutover_window` pour que le descripteur/zonefile hérite de la
bonne fenêtre. Laissez-les vides uniquement pour des essais à sec.

Type d'appel (en ligne avec le propriétaire du runbook SN-7) :

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
```L'assistant récupère automatiquement le ticket de changement comme entrée TXT et hérite du début de
la fenêtre bascule comme timestamp `effective_at` sauf override. Pour le workflow complet, voir
`docs/source/sorafs_gateway_dns_owner_runbook.md`.

### Note sur la délégation DNS publique

Le squelette de zonefile ne définit que les enregistrements autoritatifs de la
zone. Vous devez encore configurer la délégation NS/DS de la zone parente chez
votre registrar ou fournisseur DNS pour que l'internet public trouve les serveurs
de noms.

- Pour les cutovers à l'apex/TLD, utilisez ALIAS/ANAME (selon le fournisseur) ou
  publiez des enregistrements A/AAAA pointant vers les IP anycast de la passerelle.
- Pour les sous-domaines, publiez un CNAME vers le joli hôte dérive
  (`<fqdn>.gw.sora.name`).
- L'hôte canonique (`<hash>.gw.sora.id`) reste sous le domaine du gateway et
  n'est pas publié dans votre zone publique.

### Modèle de passerelle d'en-têtes

L'assistant de déploiement emet aussi `portal.gateway.headers.txt` et
`portal.gateway.binding.json`, deux artefacts qui satisfont à l'exigence DG-3
pour la liaison de contenu de passerelle :- `portal.gateway.headers.txt` contient le bloc complet de headers HTTP (incluant
  `Sora-Name`, `Sora-Content-CID`, `Sora-Proof`, CSP, HSTS et le descripteur
  `Sora-Route-Binding`) que les gateways edge doivent agrafer à chaque réponse.
- `portal.gateway.binding.json` enregistre la même information en forme lisible machine
  pour que les tickets de changement et l'automatisation puissent comparer les
  liaisons hôte/cid sans scraper la sortie shell.

Ils sont générés automatiquement via
`cargo xtask soradns-binding-template`
et capturent l'alias, le résumé du manifeste et le hostname gateway fournit un
`sorafs-pin-release.sh`. Pour régénérer ou personnaliser le bloc de headers, lancez :

```bash
cargo xtask soradns-binding-template \
  --manifest artifacts/sorafs/portal.manifest.json \
  --alias docs.sora \
  --hostname docs.sora.link \
  --route-label production \
  --json-out artifacts/sorafs/portal.gateway.binding.json \
  --headers-out artifacts/sorafs/portal.gateway.headers.txt
```

Passez `--csp-template`, `--permissions-template`, ou `--hsts-template` pour contourner
les templates de headers par défaut quand un déploiement a besoin de directives
supplémentaires; combinez-les avec les switchs `--no-*` existants pour supprimer
un en-tête complétion.

Joignez le snippet de headers à la demande de changement CDN et alimentez le document JSON
au pipeline d'automatisation gateway pour que la promotion d'hôte corresponde à
à l'évidence de libération.

Le script de release exécute automatiquement l'aide de vérification pour que les
les billets DG-3 incluent toujours une preuve récente. Ré-exécutez-le manuellement si
vous modifiez le JSON de liaison à la main :

```bash
cargo xtask soradns-verify-binding \
  --binding artifacts/sorafs/portal.gateway.binding.json \
  --alias docs.sora.link \
  --hostname docs.sora.link \
  --proof-status ok \
  --manifest-json artifacts/sorafs/portal.manifest.json
```La commande décode le payload `Sora-Proof` agrafe, vérifie que les métadonnées
`Sora-Route-Binding` correspond au CID du manifeste + hostname, et echoue vite
si un en-tête dérive. Archivez la sortie console avec les autres artefacts de
déploiement quand vous lancez la commande hors CI pour que les reviewers DG-3
avoir la preuve que la liaison a été valide avant le basculement.

> **Intégration du descripteur DNS:** `portal.dns-cutover.json` embarque maintenant
> une section `gateway_binding` pointant vers ces artefacts (chemins, contenu CID,
> statut du proof et template de headers literal) **et** une strophe `route_plan`
> qui référence `gateway.route_plan.json` ainsi que les templates de headers
> principal et rollback. Incluez ces blocs dans chaque ticket DG-3 pour que les
> reviewers peuvent comparer les valeurs exactes `Sora-Name/Sora-Proof/CSP` et
> confirmer que les plans promotion/rollback correspondant au bundle d'evidence
> sans ouvrir l'archive build.

## Etape 9 - Exécuter les moniteurs de publication

L'item roadmap **DOCS-3c** requiert une preuve continue que le portail, le proxy Try it et
les liaisons gateway restent saines après un release. Exécutez le moniteur consolider
juste après les Etapes 7-8 et branchez-le à vos programmes sondes :

```bash
cd docs/portal
npm run monitor:publishing -- \
  --config ../../configs/docs_monitor.json \
  --json-out ../../artifacts/sorafs/${RELEASE_TAG}/monitoring/summary-$(date -u +%Y%m%dT%H%M%SZ).json \
  --evidence-dir ../../artifacts/sorafs/${RELEASE_TAG}/monitoring
```- `scripts/monitor-publishing.mjs` charge le fichier config (voir
  `docs/portal/docs/devportal/publishing-monitoring.md` pour le schéma) et
  exécuter trois vérifications : sondes de chemins portail + validation CSP/Permissions-Policy,
  sondes proxy Try it (optionnellement via son endpoint `/metrics`), et le vérificateur
  de la passerelle de liaison (`cargo xtask soradns-verify-binding`) qui exige maintenant la
  présence et la valeur attendue de Sora-Content-CID avec les checks alias/manifeste.
- La commande retourne non nulle lorsqu'une sonde fait écho pour que CI, cron jobs, ou
  Les opérateurs de runbook peuvent arrêter une version avant de promouvoir l'alias.
- Passer `--json-out` ecrit un CV JSON unique avec le statut par cible ; `--evidence-dir`
  emet `summary.json`, `portal.json`, `tryit.json`, `binding.json`, et `checksums.sha256`
  pour que les reviewers gouvernance puissent comparer les résultats sans relancer les moniteurs.
  Archivez ce répertoire sous `artifacts/sorafs/<tag>/monitoring/` avec le bundle Sigstore
  et le descripteur DNS.
- Incluez la sortie du moniteur, l'export Grafana (`dashboards/grafana/docs_portal.json`),
  et l'ID de drill Alertmanager dans le ticket release pour que le SLO DOCS-3c soit
  vérifiable plus tard. Le playbook de monitoring Publishing dédié au vit a
  `docs/portal/docs/devportal/publishing-monitoring.md`.Les sondes portail requièrent HTTPS et rejettent les URLs base `http://` sauf si
`allowInsecureHttp` est défini dans le moniteur de configuration ; garder les cibles production/mise en scène
sur TLS et n'activez l'override que pour des aperçus locaux.

Automatisez le moniteur via `npm run monitor:publishing` en Buildkite/cron une fois
le portail en ligne. La meme commande, pointe sur les URLs production, alimente les checks
la santé continue entre les sorties.

## Automatisation avec `sorafs-pin-release.sh`

`docs/portal/scripts/sorafs-pin-release.sh` encapsule les Etapes 2-6. Il :

1. archiver `build/` dans une archive tar déterministe,
2. exécutez `car pack`, `manifest build`, `manifest sign`, `manifest verify-signature`,
   et `proof verify`,
3. exécuter optionnellement `manifest submit` (incluant le bind d'alias) quand
   les informations d'identification Torii sont présentes, et
4. écrivez `artifacts/sorafs/portal.pin.report.json`, le optionnel
  `portal.pin.proposal.json`, le descripteur DNS (après soumission),
  et le bundle de bind gateway (`portal.gateway.binding.json` plus le bloc de headers)
  pour que les équipes gouvernance, réseautage et opérations puissent comparer l'évidence
  sans fouiller les logs CI.

Définissez `PIN_ALIAS`, `PIN_ALIAS_NAMESPACE`, `PIN_ALIAS_NAME`, et (optionnellement)
`PIN_ALIAS_PROOF_PATH` avant d'invoquer le script. Utilisez `--skip-submit` pour dessèchement
court; le workflow GitHub ci-dessous bascule ceci via l'entrée `perform_submit`.

## Etape 8 - Publier les specs OpenAPI et bundles SBOMDOCS-7 requiert que le build du portail, la spec OpenAPI et les artefacts SBOM traversent
le meme pipeline déterministe. Les helpers existants couvrent les trois :

1. **Régénérer et signer la spec.**

   ```bash
   npm run sync-openapi -- --version=2025-q3 --mirror=current --latest
   cargo xtask openapi --sign docs/portal/static/openapi/manifest.json
   ```

   Fournissez un label release via `--version=<label>` quand vous voulez préserver un snapshot
   historique (par exemple `2025-q3`). Le helper écrit le snapshot dans
   `static/openapi/versions/<label>/torii.json`, le miroir dans
   `versions/current`, et enregistrer la métadonnée (SHA-256, statut du manifeste, timestamp
   mis un jour) dans `static/openapi/versions.json`. Le portail développeur lit cet index
   pour que les panneaux Swagger/RapiDoc présentent un sélecteur de version et affichent le
   digest/la signature associée en ligne. Omettre `--version` conserver les étiquettes du release
   precedent et ne rafraichit que les pointeurs `current` + `latest`.

   Le manifeste capture les digests SHA-256/BLAKE3 pour que la passerelle puisse agrafer des
   collecteurs `Sora-Proof` pour `/reference/torii-swagger`.

2. **Emettre les SBOMs CycloneDX.** Le pipeline release attend déjà des SBOMs syft
   selon `docs/source/sorafs_release_pipeline_plan.md`. Gardez la sortie
   près des artefacts de build:

   ```bash
   syft dir:build -o json > "$OUT"/portal.sbom.json
   syft file:docs/portal/static/openapi/torii.json -o json > "$OUT"/openapi.sbom.json
   ```

3. **Empaqueter chaque charge utile en CAR.**

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
   ```Suivez les mèmes étapes `manifest build` / `manifest sign` que le site principal,
   en ajustant les alias par artefact (par exemple, `docs-openapi.sora` pour la spec et
   `docs-sbom.sora` pour le bundle SBOM signé). Garder des alias distincts maintenir
   SoraDNS, GAR et tickets rollback limitent exactement la charge utile.

4. **Submit et bind.** Réutilisez l'autorité existante et le bundle Sigstore, mais enregistrez
   le tuple d'alias dans le checklist release pour que les auditeurs puissent suivre quel
   nom Sora mappe vers quel digest manifeste.

Archiver les manifestes spec/SBOM avec le build portail assure que chaque ticket release
contient le set complet d'artefacts sans ré-exécuter le packer.

### Helper d'automatisation (CI/package script)

`./ci/package_docs_portal_sorafs.sh` codifie les Etapes 1-8 pour que l'item roadmap
**DOCS-7** soit exécutable avec une seule commande. L'assistant :- exécuter la préparation requise du portail (`npm ci`, sync OpenAPI/norito, tests widgets) ;
- emet les CARs et paires manifestes du portail, OpenAPI et SBOM via `sorafs_cli` ;
- exécuter optionnellement `sorafs_cli proof verify` (`--proof`) et la signature Sigstore
  (`--sign`, `--sigstore-provider`, `--sigstore-audience`) ;
- déposer tous les artefacts sous `artifacts/devportal/sorafs/<timestamp>/` et
  écrivez `package_summary.json` pour que CI/outillage release puisse ingérer le bundle; et
- rafraichit `artifacts/devportal/sorafs/latest` pour pointeur vers la dernière exécution.

Exemple (pipeline complet avec Sigstore + PoR) :

```bash
./ci/package_docs_portal_sorafs.sh \
  --proof \
  --sign \
  --sigstore-provider=github-actions \
  --sigstore-audience=sorafs-devportal
```

Drapeaux utiles :- `--out <dir>` - override de la racine des artefacts (conserve par défaut l'horodatage des dossiers).
- `--skip-build` - réutiliser un `docs/portal/build` existant (utile quand CI ne peut pas
  reconstruire une cause de miroirs hors ligne).
- `--skip-sync-openapi` - ignorer `npm run sync-openapi` quand `cargo xtask openapi`
  ne peut pas joindre crates.io.
- `--skip-sbom` - éviter d'appeler `syft` lorsque le binaire n'est pas installé (le script logge
  un avertissement à la place).
- `--proof` - exécuter `sorafs_cli proof verify` pour chaque paire CAR/manifeste. Les charges utiles
  multi-fichiers exigeant encore le support chunk-plan dans le CLI, donc laissez ce flag
  desactive si vous rencontrez des erreurs `plan chunk count` et verifiez manuellement
  une fois le portail en amont livre.
- `--sign` - invoque `sorafs_cli manifest sign`. Fournissez un token via
  `SIGSTORE_ID_TOKEN` (ou `--sigstore-token-env`) ou laissez le CLI le récupérer via
  `--sigstore-provider/--sigstore-audience`.Pour les artefacts production, utilisez `docs/portal/scripts/sorafs-pin-release.sh`.
Il empaquete maintenant le portail, OpenAPI et SBOM, signe chaque manifeste, et enregistre
les actifs de métadonnées supplémentaires dans `portal.additional_assets.json`. L'assistant
comprend les memes boutons optionnels que le packager CI plus les nouveaux switchs
`--openapi-*`, `--portal-sbom-*`, et `--openapi-sbom-*` pour assigner des tuples d'alias
par artefact, override de la source SBOM via `--openapi-sbom-source`, sauter certains
payloads (`--skip-openapi`/`--skip-sbom`), et pointeur vers un binaire `syft` non par défaut
avec `--syft-bin`.

Le script affiche chaque commande exécutée; copiez le log dans le ticket release
avec `package_summary.json` pour que les reviewers puissent comparer les digests CAR,
les métadonnées de plan, et les hachages du bundle Sigstore sans parcourir une sortie shell ad hoc.

## Etape 9 - Passerelle de vérification + SoraDNS

Avant d'annoncer un basculement, prouvez que le nouvel alias est résolu via SoraDNS et que les passerelles
agrafffer des preuves frais:

1. **Executer le gate probe.** `ci/check_sorafs_gateway_probe.sh` exerce
   `cargo xtask sorafs-gateway-probe` contre les luminaires demo dans
   `fixtures/sorafs_gateway/probe_demo/`. Pour des déploiements reels, pointez le sonde
   vers le nom d'hôte cible :

   ```bash
   ./ci/check_sorafs_gateway_probe.sh -- \
     --gateway "https://docs.sora/.well-known/sorafs/manifest" \
     --header "Accept: application/json" \
     --gar fixtures/sorafs_gateway/probe_demo/demo.gar.jws \
     --gar-key "demo-gar=$(<fixtures/sorafs_gateway/probe_demo/gar_pub.hex>)" \
     --host "docs.sora" \
     --report-json artifacts/sorafs_gateway_probe/ci/docs.json
   ```

   La sonde décode `Sora-Name`, `Sora-Proof`, et `Sora-Proof-Status` selon
   `docs/source/sorafs_alias_policy.md` et echoue quand le digest manifeste,
   les TTLs ou les liaisons GAR dérivées.Pour les contrôles ponctuels légers (par exemple, lorsque seul le lot de liaisons
   modifié), exécutez `cargo xtask soradns-verify-binding --binding <portal.gateway.binding.json> --alias "<alias>" --hostname "<gateway-host>" --proof-status ok --manifest-json <portal.manifest.json>`.
   L'assistant valide le bundle de liaison capturé et est pratique pour la libération
   des billets qui nécessitent uniquement une confirmation contraignante au lieu d’un exercice d’enquête complet.

2. **Capturer l'evidence de drill.** Pour des forets opérateurs ou des essais à sec PagerDuty,
   enveloppez la sonde avec `scripts/telemetry/run_sorafs_gateway_probe.sh --scenario
   devportal-rollout -- ...`. Le wrapper stocke headers/logs sous
   `artifacts/sorafs_gateway_probe/<stamp>/`, avec un jour `ops/drill-log.md`, et
   (optionnellement) declenchez les hooks rollback ou les payloads PagerDuty. Définissez
   `--host docs.sora` pour valider le chemin SoraDNS au lieu d'une IP hard-codee.3. **Verifier les liaisons DNS.** Quand gouvernance publie le proof alias, enregistrez
   le fichier GAR référence par le sonde (`--gar`) et joignez-le à l'evidence release.
   Les propriétaires résolvent peuvent rejouer le meme input via `tools/soradns-resolver` pour
   s'assurer que les entrées en cache honorent le nouveau manifeste. Avant de joindre le JSON,
   exécuter
   `cargo xtask soradns-verify-gar --gar <path> --name <alias> [--manifest-cid <cid>] [--telemetry-label <label>]`
   pour valider hors ligne le mapping host déterministe, la métadonnée manifeste et les labels
   télémétrie. Le helper peut émettre un CV `--json-out` avec le GAR signé pour que les
   les critiques ont une preuve vérifiable sans ouvrir le binaire.
  Quand vous redigez un nouveau GAR, préférez
  `cargo xtask soradns-gar-template --name <alias> --manifest <portal.manifest.json> --telemetry-label <label> ...`
  (revenez a `--manifest-cid <cid>` uniquement quand le fichier manifeste n'est pas
  disponible). Le helper dérive maintenant le CID **et** le digest BLAKE3 directement
  depuis le manifest JSON, coupe les espaces, déduplique les flags `--telemetry-label`,
  essayez les labels et emet les templates CSP/HSTS/Permissions-Policy par défaut avant
  d'écrire le JSON pour que le payload reste déterministe même si les opérateurs
  capturent des étiquettes depuis des coquilles différentes.

4. **Surveiller les métriques alias.** Gardez `torii_sorafs_alias_cache_refresh_duration_ms`
   et `torii_sorafs_gateway_refusals_total{profile="docs"}` à l'écran pendant la sonde;
   ces séries sont graphées dans `dashboards/grafana/docs_portal.json`.

## Etape 10 - Monitoring et bundle d'évidence- **Tableaux de bord.** Exportez `dashboards/grafana/docs_portal.json` (portail SLO),
  `dashboards/grafana/sorafs_gateway_observability.json` (passerelle de latence +
  sante proof), et `dashboards/grafana/sorafs_fetch_observability.json`
  (sante orchestrator) pour chaque sortie. Joignez les exports JSON au ticket release
  pour que les réviseurs puissent rejouer les requêtes Prometheus.
- **Sonde archives.** Conservez `artifacts/sorafs_gateway_probe/<stamp>/` dans git-annex
  ou votre seau d'évidence. Incluez la sonde de reprise, les en-têtes et le payload PagerDuty
  capturer par le script la télémétrie.
- **Bundle de release.** Stockez les CV CAR du portail/SBOM/OpenAPI, les bundles manifestes,
  signatures Sigstore, `portal.pin.report.json`, logs sonde Try-It, et rapports link-check
  sous un horodatage de dossier (par exemple, `artifacts/sorafs/devportal/20260212T1103Z/`).
- **Drill log.** Quand les sondes font partie d'un foret, laissez
  `scripts/telemetry/run_sorafs_gateway_probe.sh` ajouter des entrées à `ops/drill-log.md`
  pour que la même preuve satisfasse l'exigence chaos SNNet-5.
- **Liens de ticket.** Referencez les IDs de panel Grafana ou les exports PNG attachés dans le
  ticket de changement, avec le chemin du rapport sonde, pour que les réviseurs puissent
  récupérer les SLO sans accès shell.

## Etape 11 - Drill fetch tableau de bord multi-sources et preuves

Publier sur SoraFS requiert maintenant une preuve fetch multi-source (DOCS-7/SF-6)
avec les preuves DNS/gateway ci-dessus. Après le pin du manifeste :1. **Lancer `sorafs_fetch` contre le manifeste live.** Utiliser les artefacts plan/manifeste
   produits dans les Etapes 2-3 avec les identifiants gateway emis pour chaque fournisseur.
   Persistez toutes les sorties pour que les auditeurs puissent rejouer la trace de décision
   orchestrateur :

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

   - Récupérez d'abord les références des fournisseurs d'annonces par le manifeste (par exemple
     `sorafs_cli manifest describe --provider-adverts-out artifacts/sorafs/provider_adverts/`)
     et passez-les via `--provider-advert name=path` pour que le scoreboard évalue les
     fenêtres de capacité de facon déterministe. Utiliser
     `--allow-implicit-provider-metadata` **uniquement** quand vous rejouez des luminaires en CI;
     les perceuses production doivent citer les panneaux publicitaires qui ont accompagné le pin.
   - Quand le manifeste référence des régions additionnelles, répétez la commande avec les
     tuples fournisseurs correspondants pour que chaque cache/alias ait un artefact fetch associé.

2. **Archiver les sorties.** Conservez `scoreboard.json`,
   `providers.ndjson`, `fetch.json`, et `chunk_receipts.ndjson` sous le dossier de preuve
   du release. Ces fichiers capturent le pondération des pairs, le budget de nouvelle tentative, la latence EWMA
   et les reçus par chunk que le paquet gouvernance doit retenir pour SF-7.3. **Mettre a jour la telemetrie.** Importez les sorties fetch dans le tableau de bord
   **SoraFS Récupérer l'observabilité** (`dashboards/grafana/sorafs_fetch_observability.json`),
   surveillez `torii_sorafs_fetch_duration_ms`/`_failures_total` et les panneaux de range supplier
   pour les anomalies. Liez les snapshots Grafana au ticket release avec le chemin scoreboard.

4. **Tester les règles d'alertes.** Exécutez `scripts/telemetry/test_sorafs_fetch_alerts.sh`
   pour valider le bundle d'alertes Prometheus avant de fermer le release. Joignez la sortie
   promtool au ticket pour que les reviewers DOCS-7 confirment que les alertes de décrochage et
   slow-provider reste armé.

5. **Brancher en CI.** Le workflow de pin portail garde une étape `sorafs_fetch` derrière
   l'entrée `perform_fetch_probe` ; activez-le pour les runs staging/production afin que
   l'evidence fetch soit produite avec le bundle manifeste sans intervention manuelle.
   Les exercices locaux peuvent réutiliser le meme script en exportant les tokens gateway et
   en définissant `PIN_FETCH_PROVIDERS` à la liste des fournisseurs séparés par virgules.

## Promotion, observabilité et rollback1. **Promotion :** gardez des alias distincts pour la mise en scène et la production. Promouvez fr
   re-executant `manifest submit` avec le meme manifeste/bundle, en changeant
   `--alias-namespace/--alias-name` pour pointeur vers la production d'alias. Cela évite
   de reconstruire ou re-signer une fois que QA approuve le pin staging.
2. **Monitoring :** importez le registre des broches du tableau de bord
   (`docs/source/grafana_sorafs_pin_registry.json`) plus les sondes spécifiques au portail
   (voir `docs/portal/docs/devportal/observability.md`). Alertez sur la dérive des sommes de contrôle,
   les sondes font écho ou les photos de la preuve de nouvelle tentative.
3. **Rollback:** pour revenir en arrière, resoumettez le manifeste précédent (ou retirez
   l'alias courant) avec `sorafs_cli manifest submit --alias ... --retire`.
   Gardez toujours le dernier bundle connu comme bon et le CV CAR pour que les
   proofs rollback peut etre recrees si les logs CI tournent.

## Modèle de workflow CI

Au minimum, votre pipeline doit :

1. Build + lint (`npm ci`, `npm run build`, génération des sommes de contrôle).
2. Empaqueter (`car pack`) et calculer les manifestes.
3. Signataire via le token OIDC du job (`manifest sign`).
4. Téléchargez les artefacts (CAR, manifeste, bundle, plan, CV) pour audit.
5. Registre Soumettre au pin :
   - Demandes d'extraction -> `docs-preview.sora`.
   - Tags/branches protégées -> production d'alias de promotion.
6. Exécuter les sondes + portes de vérification proof avant de terminer.`.github/workflows/docs-portal-sorafs-pin.yml` connecte toutes ces étapes pour les versions manuelles.
Le flux de travail :

- construire/tester le portail,
- empaqueter le build via `scripts/sorafs-pin-release.sh`,
- signer/vérifier le bundle manifest via GitHub OIDC,
- télécharger le CAR/manifeste/bundle/plan/resumes comme artefacts, et
- (optionnellement) soumettre le manifeste + alias contraignant lorsque les secrets sont présents.

Configurez les secrets/variables suivants avant de déclencher le travail :

| Nom | Mais |
| --- | --- |
| `DOCS_SORAFS_TORII_URL` | Hôte Torii qui expose `/v1/sorafs/pin/register`. |
| `DOCS_SORAFS_SUBMITTED_EPOCH` | Identifiant d'époque enregistré avec les soumissions. |
| `DOCS_SORAFS_AUTHORITY` / `DOCS_SORAFS_PRIVATE_KEY` | Autorite de signature pour la soumission manifeste. |
| `DOCS_SORAFS_ALIAS_NAMESPACE` / `DOCS_SORAFS_ALIAS_NAME` | L'alias du tuple se manifeste lorsque `perform_submit` est `true`. |
| `DOCS_SORAFS_ALIAS_PROOF_B64` | Bundle proof alias encode en base64 (optionnel; omettre pour sauter la liaison d'alias). |
| `DOCS_ANALYTICS_*` | Endpoints Analytics/Probe existants réutilisés par d'autres workflows. |

Déclenchez le workflow via l'UI Actions :

1. Fournissez `alias_label` (ex : `docs.sora.link`), un `proposal_alias` optionnel,
   et un override `release_tag` optionnel.
2. Laissez `perform_submit` décoché pour générer les artefacts sans toucher Torii
   (utile pour dry runs) ou activez-le pour publier directement sur l'alias configure.`docs/source/sorafs_ci_templates.md` documente encore les helpers CI génériques pour
les projets hors repo, mais le portail workflow doit être la voie préférée
pour les releases quotidiennes.

## Liste de contrôle

-[ ] `npm run build`, `npm run test:*`, et `npm run check:links` sont verts.
-[ ] `build/checksums.sha256` et `build/release.json` captures dans les artefacts.
- [ ] CAR, plan, manifeste, et curriculum vitae génériques sous `artifacts/`.
-[ ] Bundle Sigstore + stocks détachés signature avec les logs.
-[ ] `portal.manifest.submit.summary.json` et `portal.manifest.submit.response.json`
      capture quand des soumissions ont lieu.
-[ ] `portal.pin.report.json` (et optionnel `portal.pin.proposal.json`)
      archive avec les artefacts CAR/manifeste.
-[ ] Logs archives `proof verify` et `manifest verify-signature`.
-[ ] Tableaux de bord Grafana mis à jour + sondes Try-It reussis.
- [ ] Notes rollback (ID manifeste précédent + alias digest) jointes au
      libération des billets.