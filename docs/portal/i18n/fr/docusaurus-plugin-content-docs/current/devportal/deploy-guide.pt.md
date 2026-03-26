---
lang: fr
direction: ltr
source: docs/portal/docs/devportal/deploy-guide.pt.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

## Visa général

Ce playbook convertit les éléments en feuille de route **DOCS-7** (publication de SoraFS) et **DOCS-8**
(automatisation de la broche de CI/CD) dans une procédure à suivre pour le portail des utilisateurs.
Cobra a phase de build/lint, o empacotamento SoraFS, a assinatura de manifestes com Sigstore,
la promotion de l'alias, la vérification et les exercices de restauration pour chaque aperçu et sortie
seja reproduzivel e auditavel.

Le flux suppose que vous dites le binaire `sorafs_cli` (construit avec `--features cli`), accédez à
Un point de terminaison Torii avec des autorisations de registre de broches et des informations d'identification OIDC pour Sigstore. Gardes séparés
de longue durée (`IROHA_PRIVATE_KEY`, `SIGSTORE_ID_TOKEN`, jetons de Torii) dans votre coffre-fort de CI ; comme
les exécuteurs locaux peuvent charger des marchandises à partir des exportations de shell.

## Pré-requis- Nœud 18.18+ avec `npm` ou `pnpm`.
- `sorafs_cli` à partir de `cargo run -p sorafs_car --features cli --bin sorafs_cli`.
- URL de Torii qui expose `/v1/sorafs/*` mais qui a un contact/une personne privée d'autorisation qui peut envoyer des manifestes et des alias.
- Émetteur OIDC (Actions GitHub, GitLab, identité de charge de travail, etc.) pour émettre un `SIGSTORE_ID_TOKEN`.
- Facultatif : `examples/sorafs_cli_quickstart.sh` pour les exécutions et `docs/source/sorafs_ci_templates.md` pour l'échafaudage des workflows de GitHub/GitLab.
- Configurer comme variables OAuth de Tre it (`DOCS_OAUTH_*`) et exécuter un
 [liste de contrôle pour le renforcement de la sécurité](./security-hardening.md) avant de promouvoir une build
 hors du laboratoire. La construction du portail a maintenant échoué lorsque ces variables ont échoué
 o quand les boutons de TTL/polling sont utilisés comme fenêtres appliquées ; exporter
 `DOCS_OAUTH_ALLOW_INSECURE=1` est utilisé pour les aperçus locaux désactivés. Anexe a
 preuve du pen-test sur le ticket de sortie.

## Etapa 0 - Capturer un paquet du prochain Tre it

Antes de promouvoir un aperçu de Netlife ou de la passerelle, vendu comme sources du proximité Tre it e o
résumé du manifeste OpenAPI confirmé dans un paquet déterminé :

```bash
cd docs/portal
npm run release:tryit-proxe -- \
 --out ../../artifacts/tryit-proxy/$(date -u +%E%m%dT%H%M%SZ) \
 --target https://torii.dev.sora \
 --label preview-2026-02-14
````scripts/tryit-proxy-release.mjs` copie des helpers de proxy/probe/rollback, vérifier l'assistanat
OpenAPI et écrivons `release.json` mais `checksums.sha256`. Anexe este pacote al ticket de
promotion de Netlify/SoraFS gatewae pour que les réviseurs puissent reproduire des sources exactes
del proxe e as pistas del target Torii sem reconstruir. Le paquet sera également enregistré si vous
porteurs de provisions pour le client établi habilité (`allow_client_auth`) pour le plan
de déploiement et comme règle CSP lors de la synchronisation.

## Étape 1 - Construire le contenu du portail

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

- `build/checksums.sha256` - manifeste SHA256 adapté à `sha256sum -c`.
- `build/release.json` - métadonnées (`tag`, `generated_at`, `source`) fixées dans chaque CAR/manifeste.

Arquive ambos arquivos junto al CV CAR para que os réviseurs puissent comparer les artefacts de
aperçu sem reconstruire.

## Étape 2 - Empaqueter les actifs statiques

Exécuter l'empacotamentor CAR contre le directeur de sortie Docusaurus. L'exemple d'abajo
écrivez todos os artefactos bajo `artifacts/devportal/`.

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

Le résumé JSON capture des contenus de morceaux, des résumés et des pistes de planification de la preuve que
`manifest build` et les tableaux de bord de CI réutilisent ensuite.

## Etapa 2b - Compagnons Empaqueta OpenAPI e SBOMDOCS-7 nécessite la publication du site du portail, de l'instantané OpenAPI et des charges utiles SBOM
comme des manifestes distincts pour que les passerelles puissent graver les en-têtes
`Sora-Proof`/`Sora-Content-CID` pour chaque article. O aide de libération
(`scripts/sorafs-pin-release.sh`) vous avez mis le répertoire OpenAPI
(`static/openapi/`) et les SBOM émis via `syft` dans les CAR séparés
`openapi.*`/`*-sbom.*` et enregistrer les métadonnées dans
`artifacts/sorafs/portal.additional_assets.json`. À l'exécution du manuel de flux,
Répétez les étapes 2 à 4 pour chaque charge utile avec vos propres préfixes et étiquettes de métadonnées
(par exemple `--car-out "$OUT"/openapi.car` mas
`--metadata alias_label=docs.sora.link/openapi`). Registre cada par manifeste/alias
dans Torii (site, OpenAPI, SBOM du portail, SBOM de OpenAPI) avant de changer de DNS pour cela
Le portail peut servir de preuves gravées pour tous les artefacts publiés.

## Étape 3 - Construire le manifeste

```bash
sorafs_cli manifest build \
 --summare "$OUT"/portal.car.json \
 --manifest-out "$OUT"/portal.manifest.to \
 --manifest-json-out "$OUT"/portal.manifest.json \
 --pin-min-replicas 5 \
 --pin-storage-class warm \
 --pin-retention-epoch 14 \
 --metadata alias_label=docs.sora.link
```

Ajustez les drapeaux politiques à partir de votre fenêtre de sortie (par exemple, `--pin-storage-class
chaud pour les canaris). Une variante JSON est facultative, mais pratique pour la révision du code.

## Etapa 4 - Assinatura avec Sigstore

```bash
sorafs_cli manifest sign \
 --manifest "$OUT"/portal.manifest.to \
 --chunk-plan "$OUT"/portal.plan.json \
 --bundle-out "$OUT"/portal.manifest.bundle.json \
 --signature-out "$OUT"/portal.manifest.sig \
 --identity-token-provider github-actions \
 --identity-token-audience sorafs-devportal
```Le bundle enregistre le résumé du manifeste, les résumés de morceaux et un hachage BLAKE3 du jeton
OIDC sans persister sur JWT. Garder tanto o bundle como a assinatura separada ; comme promotions de
La production peut réutiliser mes objets au lieu de les réutiliser. En tant qu'exécutifs locaux
Vous pouvez remplacer les drapeaux du fournisseur avec `--identity-token-env` (ou établir
`SIGSTORE_ID_TOKEN` em ou entorno) lorsqu'un assistant OIDC externe émet un jeton.

## Etapa 5 - Envie du registre des broches

Envoyez le manifeste ferme (et le plan de morceaux) au Torii. Sollicitez toujours un curriculum vitae pour vous
l'entrée/alias résultante seja auditavel.

```bash
sorafs_cli manifest submit \
 --manifest "$OUT"/portal.manifest.to \
 --chunk-plan "$OUT"/portal.plan.json \
 --torii-url "$TORII_URL" \
 --authorite <katakana-i105-account-id> \
 --private-kee "$IROHA_PRIVATE_KEY" \
 --submitted-epoch 20260101 \
 --alias-namespace docs \
 --alias-name sora.link \
 --alias-proof "$OUT"/docs.alias.proof \
 --summary-out "$OUT"/portal.submit.json \
 --response-out "$OUT"/portal.submit.response.json
```

Lorsque vous affichez un alias de prévisualisation du canare (`docs-preview.sora`), répétez le
envoyer avec un alias unique pour que QA puisse vérifier le contenu avant la promotion
production.

La liaison de l'alias nécessite trois champs : `--alias-namespace`, `--alias-name` et `--alias-proof`.
La gouvernance produit un bundle de preuves (base64 ou octets Norito) lorsque vous demandez une sollicitation
del alias; guardalo em segredos de CI et exponlo como archivo ante de invocar `manifest submit`.
Deja os flags de alias sem establecer wheno apenas piensas fijar o manifeste sem tocar DNS.

## Etapa 5b - Genera uma propuesta de gouvernanceChaque manifeste doit être consulté avec une liste de propositions pour le Parlement pour tout citoyen
Sora peut donc introduire le changement sans avoir à obtenir des informations d'identification privilégiées. Après les étapes
soumettre/signer, exécuter :

```bash
sorafs_cli manifest proposal \
 --manifest "$OUT"/portal.manifest.to \
 --chunk-plan "$OUT"/portal.plan.json \
 --submitted-epoch 20260101 \
 --alias-hint docs.sora.link \
 --proposal-out "$OUT"/portal.pin.proposal.json
```

`portal.pin.proposal.json` capturer les instructions canoniques `RegisterPinManifest`,
o digérer les morceaux, une politique et une piste d'alias. Adjoint au ticket de gouvernance ou à
portail Parlement pour que les délégués puissent comparer la charge utile sem reconstruire les artefacts.
Comme le commandant n'a aucun contact avec la clave de l'autorité de Torii, tout citoyen peut rédiger un
propuesta localement.

## Étape 6 - Vérifier les preuves et la télémétrie

Après avoir vérifié, exécutez les étapes de vérification déterministe :

```bash
sorafs_cli proof verife \
 --manifest "$OUT"/portal.manifest.to \
 --car "$OUT"/portal.car \
 --summary-out "$OUT"/portal.proof.json

sorafs_cli manifest verify-signature \
 --manifest "$OUT"/portal.manifest.to \
 --bundle "$OUT"/portal.manifest.bundle.json \
 --chunk-plan "$OUT"/portal.plan.json
```- Vérifier `torii_sorafs_gateway_refusals_total` e
 `torii_sorafs_replication_sla_total{outcome="missed"}` pour anomalies.
- Exécutez `npm run probe:portal` pour extraire le proxy Try-It et les liens enregistrés
 contra o contenido recien fijado.
- Capturer les preuves de surveillance les décrivant
 [Publishing & Monitoring](./publishing-monitoring.md) pour la porte d'observabilité DOCS-3c
 se satisfaga junto a os etapas de publicacao. O helper ahora acepta multiples entrées
 `bindings` (site, OpenAPI, SBOM du portail, SBOM de OpenAPI) et application `Sora-Name`/`Sora-Proof`/`Sora-Content-CID`
 em o host objetivo via o guard optionnel `hostname`. A invocacao de abajo écris tanto um
 résumé JSON unique comme le bundle de preuves (`portal.json`, `tryit.json`, `binding.json` et
 `checksums.sha256`) sous le répertoire de version :

 ```bash
 npm run monitor:publishing -- \
 --config ../../configs/docs_monitor.json \
 --json-out ../../artifacts/sorafs/preview-2026-02-14/monitoring/summary.json \
 --evidence-dir ../../artifacts/sorafs/preview-2026-02-14/monitoring
 ```

## Etapa 6a - Plan certifié de la passerelle

Dérivez le plan de SAN/challenge TLS avant de créer des paquets GAR pour l'équipement des passerelles et des systèmes d'exploitation
les arobadores de DNS révisent avec mesma evidencia. Le nouvel assistant réfléchit aux prises en charge automatiques
L'énumération DG-3 héberge des canons génériques, des SAN jolis hôtes, des étiquettes DNS-01 et des codes ACME recommandés :

```bash
cargo xtask soradns-acme-plan \
 --name docs.sora \
 --json-out artifacts/sorafs/portal.acme-plan.json
```Comité du JSON conjointement avec le bundle de release (ou sous-produit avec le ticket de changement) pour les opérateurs
Possam pegar os valeurs SAN dans la configuration `torii.sorafs_gateway.acme` de Torii et les réviseurs
de GAR peut confirmer les mapeos canonico/prette sem re-executar derivaciones de host. Agréga
arguments `--name` supplémentaires pour chaque soufijo promu dans la même version.

## Etapa 6b - Dériva mapeos de host canonicos

Avant les charges utiles des Templiers GAR, enregistrez la carte de l'hôte déterminant pour chaque alias.
`cargo xtask soradns-hosts` hashea cada `--name` sur votre étiquette canonique
(`<base32>.gw.sora.id`), émet le caractère générique requis (`*.gw.sora.id`) et dérive le joli hôte
(`<alias>.gw.sora.name`). Persister à la libération des artefacts de publication pour les réviseurs
DG-3 peut comparer la carte avec l'envoi GAR :

```bash
cargo xtask soradns-hosts \
 --name docs.sora \
 --json-out artifacts/sorafs/portal.canonical-hosts.json
```

Utilisez `--verify-host-patterns <file>` pour tomber rapidement lorsque vous utilisez JSON de GAR ou la liaison de la passerelle
omettez un des hôtes requis. O helper accepte plusieurs archives de vérification, haciendo
faciliter le nettoyage de la plante GAR comme `portal.gateway.binding.json` gravée dans une même invocation :

```bash
cargo xtask soradns-hosts \
 --name docs.sora \
 --json-out artifacts/sorafs/portal.canonical-hosts.json \
 --verify-host-patterns artifacts/sorafs/portal.gar.json \
 --verify-host-patterns artifacts/sorafs/portal.gateway.binding.json
```

Annexe au résumé JSON et au journal de vérification du ticket de changement de DNS/gatewae pour cela
Les auditeurs peuvent confirmer les hôtes canoniques, les caractères génériques et les prêts à réexécuter les scripts.
Réexécutez la commande lorsque vous ajoutez un nouvel alias au bundle pour l'actualisation de GAR.
Hereden a mesma evidencia.## Etapa 7 - Générer ou descripteur de basculement DNS

Les transitions de production nécessitent un paquet de changement audité. Après une soumission exitoso
(liaison de l'alias), o l'assistant émet
`artifacts/sorafs/portal.dns-cutover.json`, capture :- métadonnées de liaison de l'alias (espace de noms/nom/preuve, résumé du manifeste, URL Torii,
 époque enviado, autoridad);
- contexte de publication (tag, alias label, rutas de manifesto/CAR, plan de chunks, bundle Sigstore) ;
- punteros de verificacao (sonde comando, alias + point final Torii) ; e
- champs optionnels de change-control (id de ticket, ventana de cutover, contacto ops,
 nom d'hôte/zone de production);
- métadonnées de promotion des itinéraires dérivées de l'en-tête `Sora-Route-Binding`
 (hôte canonique/CID, routes d'en-tête + liaison, commandes de vérification), assurer la promotion
 GAR et les exercices de référence de secours à même preuve ;
- os artefactos de route-plan generados (`gateway.route_plan.json`,
 modèles d'en-têtes et en-têtes de restauration optionnels) pour les tickets de changement et les crochets de
 Lint de CI peut vérifier que chaque paquet DG-3 fait référence aux plans de promotion/rollback
 canonicos antes de aprobacion;
- métadonnées facultatives d'invalidation du cache (point de terminaison de purge, variable d'authentification, charge utile JSON,
 et exemple de commande `curl`); e
- conseils de restauration apuntando al descriptor previo (tag de release e digest del manifesto)
 pour que les tickets capturent une route de secours déterministe.

Lorsque la version nécessite une purge du cache, génère un plan canonique avec le descripteur de basculement :

```bash
cargo xtask soradns-cache-plan \
 --name docs.sora \
 --path / \
 --path /gateway/manifest.json \
 --auth-header Authorization \
 --auth-env CACHE_PURGE_TOKEN \
 --json-out artifacts/sorafs/portal.cache_plan.json
```Anexe ou `portal.cache_plan.json` résultant du paquet DG-3 pour les opérateurs
Il y a des hôtes/chemins déterministes (et des indices d'authentification qui coïncident) avec les requêtes émettrices `PURGE`.
La section facultative du cache du descripteur peut faire référence directement à ce fichier,
assurer la maintenance des réviseurs de contrôle des modifications de manière à ce que les points finaux soient propres
pendant le basculement.

Chaque paquet DG-3 nécessite également une liste de contrôle de promotion + restauration. Générala via
`cargo xtask soradns-route-plan` pour que les réviseurs de contrôle des modifications puissent suivre les étapes
détails exacts du contrôle en amont, du basculement et de la restauration par alias :

```bash
cargo xtask soradns-route-plan \
 --name docs.sora \
 --json-out artifacts/sorafs/gateway.route_plan.json
```

O `gateway.route_plan.json` émetteur de capture héberge canonicos/pretty, enregistreurs de contrôle de santé
par étapes, actualisations de la liaison GAR, purges de cache et actions de restauration. Incluyelo com
les artefacts GAR/binding/cutover avant d'envoyer le ticket de changement pour que les opérations puissent les envoyer
aprobar os mesmos etapas com guion.

`scripts/generate-dns-cutover-plan.mjs` impulsa ce descripteur et s'exécute automatiquement à partir de
`sorafs-pin-release.sh`. Pour régénérer ou personnaliser manuellement :

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

Affichez les métadonnées facultatives via les variables d'organisation avant l'exécution ou l'assistant de broche :| Variables | Proposé |
| --- | --- |
| `DNS_CHANGE_TICKET` | ID du ticket enregistré dans le descripteur. |
| `DNS_CUTOVER_WINDOW` | Ventana de cutover ISO8601 (par exemple, `2026-03-21T15:00Z/2026-03-21T15:30Z`). |
| `DNS_HOSTNAME`, `DNS_ZONE` | Nom d'hôte de production + zone autorisée. |
| `DNS_OPS_CONTACT` | Alias ​​de garde ou contacto de escalade. |
| `DNS_CACHE_PURGE_ENDPOINT` | Endpoint de purge du cache enregistré dans le descripteur. |
| `DNS_CACHE_PURGE_AUTH_ENV` | L'environnement va contenir le jeton de purge (par défaut : `CACHE_PURGE_TOKEN`). |
| `DNS_PREVIOUS_PLAN` | Itinéraire vers le descripteur de basculement précédent pour les métadonnées de restauration. |

Anexe o JSON al review de change DNS para que os arobadores possam verificar digests of manifesto,
les liaisons d'alias et les commandes sondent sans doute la révision des journaux de CI. Les drapeaux de CLI
`--dns-change-ticket`, `--dns-cutover-window`, `--dns-hostname`,
`--dns-zone`, `--ops-contact`, `--cache-purge-endpoint`,
`--cache-purge-auth-env`, et `--previous-dns-plan` remplacements de mesmos d'os prouvés
quando se exécuter o helper fuera de CI.

## Étape 8 - Émettre le squelette du fichier de zone du résolveur (facultatif)Quand la fenêtre de transition de la production est connue, le script de sortie peut émettre le
un squelette de zonefile SNS et un extrait de résolveur automatiquement. Passer les enregistrements DNS souhaités
et métadonnées via des variables d'entrée ou des options CLI ; o assistant llamara a
`scripts/sns_zonefile_skeleton.py` immédiatement après la génération du descripteur de basculement.
Prouvez au moins une valeur A/AAAA/CNAME et le résumé GAR (BLAKE3-256 de la charge utile GAR ferme). Si un
la zone/le nom d'hôte est connu et `--dns-zonefile-out` est omite, ou l'assistant les écris
`artifacts/sns/zonefiles/<zone>/<hostname>.json` et Llena
`ops/soradns/static_zones.<hostname>.json` comme extrait du résolveur.| Variable/indicateur | Proposé |
| --- | --- |
| `DNS_ZONEFILE_OUT`, `--dns-zonefile-out` | Itinéraire pour le squelette du fichier de zone généré. |
| `DNS_ZONEFILE_RESOLVER_SNIPPET`, `--dns-zonefile-resolver-snippet` | Itinéraire de l'extrait du résolveur (par défaut : `ops/soradns/static_zones.<hostname>.json` lorsqu'il est omis). |
| `DNS_ZONEFILE_TTL`, `--dns-zonefile-ttl` | TTL appliqué aux enregistrements générés (par défaut : 600 secondes). |
| `DNS_ZONEFILE_IPV4`, `--dns-zonefile-ipv4` | Instructions IPv4 (environnement séparé par des virgules ou drapeau CLI répétable). |
| `DNS_ZONEFILE_IPV6`, `--dns-zonefile-ipv6` | Directions IPv6. |
| `DNS_ZONEFILE_CNAME`, `--dns-zonefile-cname` | Ciblez CNAME facultatif. |
| `DNS_ZONEFILE_SPKI`, `--dns-zonefile-spki-pin` | Pins SPKI SHA-256 (base64). |
| `DNS_ZONEFILE_TXT`, `--dns-zonefile-txt` | Entrées TXT supplémentaires (`key=value`). |
| `DNS_ZONEFILE_VERSION`, `--dns-zonefile-version` | Remplacez l'étiquette de la version du fichier de zone calculé. |
| `DNS_ZONEFILE_EFFECTIVE_AT`, `--dns-zonefile-effective-at` | Utilisez l'horodatage `effective_at` (RFC3339) à la place du début de la fenêtre de basculement. |
| `DNS_ZONEFILE_PROOF`, `--dns-zonefile-proof` | Remplacer la preuve littérale enregistrée dans les métadonnées. |
| `DNS_ZONEFILE_CID`, `--dns-zonefile-cid` | Remplacer le CID enregistré dans les métadonnées. |
| `DNS_ZONEFILE_FREEZE_STATE`, `--dns-zonefile-freeze-state` | Estado de gel de tuteur (doux, dur, décongélation, surveillance, urgence). |
| `DNS_ZONEFILE_FREEZE_TICKET`, `--dns-zonefile-freeze-ticket` | Referencia de ticket de tuteur/conseil pour les gels. |
| `DNS_ZONEFILE_FREEZE_EXPIRES_AT`, `--dns-zonefile-freeze-expires-at` | Horodatage RFC3339 pour la décongélation. || `DNS_ZONEFILE_FREEZE_NOTES`, `--dns-zonefile-freeze-note` | Notes de gel supplémentaires (env séparées par des virgules ou un drapeau répétitif). |
| `DNS_GAR_DIGEST`, `--dns-gar-digest` | Digest BLAKE3-256 (hex) de la charge utile GAR entreprise. Requerido lorsqu’il y a des liaisons de gateway. |

Le workflow de GitHub Actions a ces valeurs à partir des secrets du référentiel pour chaque broche de
La production émet automatiquement les artefacts du fichier de zone. Configurer les secrets suivants
(les cordes peuvent contenir des listes séparées par des comas pour des champs multivaleurs) :

| Secrets | Proposé |
| --- | --- |
| `DOCS_SORAFS_DNS_HOSTNAME`, `DOCS_SORAFS_DNS_ZONE` | Nom d’hôte/zone de production passé à l’assistant. |
| `DOCS_SORAFS_DNS_OPS_CONTACT` | Alias ​​de garde almacenado em o descriptor. |
| `DOCS_SORAFS_ZONEFILE_IPV4`, `DOCS_SORAFS_ZONEFILE_IPV6` | Registres IPv4/IPv6 à publier. |
| `DOCS_SORAFS_ZONEFILE_CNAME` | Ciblez CNAME facultatif. |
| `DOCS_SORAFS_ZONEFILE_SPKI` | Pins SPKI base64. |
| `DOCS_SORAFS_ZONEFILE_TXT` | Entrées TXT supplémentaires. |
| `DOCS_SORAFS_ZONEFILE_FREEZE_STATE/TICKET/EXPIRES_AT/NOTES` | Métadonnées de gel enregistrées sur le squelette. |
| `DOCS_SORAFS_GAR_DIGEST` | Digest BLAKE3 dans l'hexagone de la charge utile GAR entreprise. |

Au disparar `.github/workflows/docs-portal-sorafs-pin.yml`, proportionner les entrées
`dns_change_ticket` et `dns_cutover_window` pour que le descripteur/fichier de zone apparaisse dans la fenêtre
correct. Dejarlos em blanco apenas quando ejecutes dre runs.

Type d'appel (coïncidant avec le runbook du propriétaire SN-7) :

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
 ...otros flags...
```L'assistant déclenche automatiquement le ticket de changement comme entrée TXT et ici le lancement d'un
il est possible de basculer avec l'horodatage `effective_at` pour éviter tout remplacement. Pour le flux
opérationnel complet, version `docs/source/sorafs_gateway_dns_owner_runbook.md`.

### Note sur la délégation DNS publique

Le squelette du fichier de zone définit les seuls registres autorisés de la zone. Ainda e
il est nécessaire de configurer le délégué NS/DS de la zone payée par l'enregistreur ou le fournisseur
DNS pour rencontrer Internet sur les serveurs de noms.

- Pour les basculements sans apex/TLD, utilisez ALIAS/ANAME (dépendant du fournisseur) ou publique
  s'enregistre A/AAAA pour les IP anycast de la passerelle.
- Pour les sous-domaines, un CNAME public pour le joli hôte dérivé
  (`<fqdn>.gw.sora.name`).
- L'hôte canonique (`<hash>.gw.sora.id`) reste permanent sur le domaine de la passerelle et du réseau
  Il est publié dans sa zone publique.

### Plante des en-têtes de la passerelle

L'assistant de déploiement émet également `portal.gateway.headers.txt` e
`portal.gateway.binding.json`, deux artefacts qui satisfont aux exigences du DG-3
liaison de contenu de passerelle :- `portal.gateway.headers.txt` contient le blocage complet des en-têtes HTTP (y compris
 `Sora-Name`, `Sora-Content-CID`, `Sora-Proof`, CSP, HSTS, descripteur e o
 `Sora-Route-Binding`) que les passerelles de frontière doivent être gravées dans chaque réponse.
- `portal.gateway.binding.json` enregistre des informations sous une forme lisible pour les machines
 pour que les tickets de changement et l'automatique puissent comparer les liaisons hôte/cid sem
 raser la salida de shell.

Se génère automatiquement via
`cargo xtask soradns-binding-template`
et capture l'alias, le résumé du manifeste et le nom d'hôte de la passerelle qui se trouve
passer à `sorafs-pin-release.sh`. Pour régénérer ou personnaliser ou bloquer les en-têtes,
exécuter :

```bash
cargo xtask soradns-binding-template \
 --manifest artifacts/sorafs/portal.manifest.json \
 --alias docs.sora \
 --hostname docs.sora.link \
 --route-label production \
 --json-out artifacts/sorafs/portal.gateway.binding.json \
 --headers-out artifacts/sorafs/portal.gateway.headers.txt
```

Pasa `--csp-template`, `--permissions-template`, ou `--hsts-template` pour remplacement
les modèles d'en-têtes par défaut lorsqu'un téléchargement nécessite des directives supplémentaires ;
combinalos com os switchs `--no-*` existent pour éliminer un en-tête complètement.

Annexe à l'extrait d'en-têtes de la demande de changement de CDN et à l'alimentation du document JSON
dans le pipeline de portails automatiques pour que la promotion réelle de l'hôte coïncide avec un
preuve de libération.

Le script de release s'exécute automatiquement ou l'aide à la vérification pour vous
les billets DG-3 incluent toujours des preuves récentes. Voir l'exécution manuelle
quand on édite la liaison JSON à la main :

```bash
cargo xtask soradns-verify-binding \
  --binding artifacts/sorafs/portal.gateway.binding.json \
  --alias docs.sora.link \
  --hostname docs.sora.link \
  --proof-status ok \
  --manifest-json artifacts/sorafs/portal.manifest.json
```La commande décodifiant la charge utile `Sora-Proof` est enregistrée, garantissant que les métadonnées `Sora-Route-Binding`
coïncidant avec le CID du manifeste + le nom d'hôte, et tombe rapidement si un en-tête est affiché.
Archiver la sortie de la console avec les autres objets de déploiement toujours à exécuter
Le commandement de CI pour que les réviseurs de la DG-3 prouvent que la liaison a été validée
avant le basculement.

> **Intégration du descripteur DNS :** `portal.dns-cutover.json` maintenant incrusté dans une section
> `gateway_binding` apuntando a ces artefactos (rutas, content CID, estado de proof e o
> modèle littéral des en-têtes) **e** uma estrofa `route_plan` référence
> `gateway.route_plan.json` principaux modèles d'en-têtes et restauration. Inclut esos
> bloque chaque ticket de changement DG-3 pour que les réviseurs puissent comparer les valeurs
> exactos de `Sora-Name/Sora-Proof/CSP` et confirmer que les plans de promotion/rollback
> coïncide avec le bundle de preuves pour ouvrir l'archive de build.

## Étape 9 - Exécuter les moniteurs de publication

L'élément de la feuille de route **DOCS-3c** nécessite des preuves continues de ce que le portail, le proxe Tre it e os
les liaisons du portail se maintiennent saludables après une libération. Exécuter ou surveiller consolidé
immédiatement après les étapes 7-8 et connecté à vos sondes programmées :

```bash
cd docs/portal
npm run monitor:publishing -- \
 --config ../../configs/docs_monitor.json \
 --json-out ../../artifacts/sorafs/${RELEASE_TAG}/monitoring/summary-$(date -u +%E%m%dT%H%M%SZ).json \
 --evidence-dir ../../artifacts/sorafs/${RELEASE_TAG}/monitoring
```- `scripts/monitor-publishing.mjs` charge ou archive de configuration (version
 `docs/portal/docs/devportal/publishing-monitoring.md` pour le schéma) et
 exécuter trois vérifications : sondes de chemins du portail + validation de CSP/Permissions-Policy,
 sondes de proximité Tre it (facultativement collectées sur le point final `/metrics`), et le vérificateur
 de liaison de passerelle (`cargo xtask soradns-verify-binding`) qui exige maintenant une présence et
 la valeur attendue de Sora-Content-CID conjointement avec les vérifications d'alias/manifeste.
- La commande se termine avec une valeur non nulle lorsque vous tombez sur une sonde pour CI, les tâches cron ou les opérateurs
 Le runbook peut arrêter une version avant l'alias du promoteur.
- Pasar `--json-out` écris un CV JSON unique avec l'état de la cible ; `--evidence-dir`
 émettre `summary.json`, `portal.json`, `tryit.json`, `binding.json` et `checksums.sha256` pour que
 Les réviseurs de gouvernance peuvent comparer les résultats sans réexécuter les moniteurs. Archiver
 ce répertoire sous `artifacts/sorafs/<tag>/monitoring/` avec le bundle Sigstore et o
 descripteur de basculement DNS.
- Incluye une sortie du moniteur, ou une exportation de Grafana (`dashboards/grafana/docs_portal.json`),
 et l'ID de l'exercice d'Alertmanager dans le ticket de sortie pour que le SLO DOCS-3c puisse être
 audité ensuite. Le playbook dédié au moniteur de publication vive-les
 `docs/portal/docs/devportal/publishing-monitoring.md`.Les sondes du portail nécessitent HTTPS et demandent la base d'URL `http://` à moins que `allowInsecureHttp`
est-ce configuré dans la configuration du moniteur ; mantenha cibles de production/staging en TLS et apenas
autoriser ou remplacer les aperçus locaux.

Automatiser le moniteur via `npm run monitor:publishing` dans Buildkite/cron depuis le portail
este em vivo. En même temps, je commande, je signale les URL de production, j'alimente les contrôles de santé
continus que SRE/Docs utilise entre les versions.

## Automatisation avec `sorafs-pin-release.sh`

`docs/portal/scripts/sorafs-pin-release.sh` encapsule les étapes 2-6. Est-ce :

1. archiver `build/` dans une archive tar déterministe,
2. exécutez `car pack`, `manifest build`, `manifest sign`, `manifest verify-signature`,
 et `proof verify`,
3. Exécutez éventuellement `manifest submit` (y compris la liaison d'alias) lorsque vous avez des informations d'identification
 Torii, et
4. écrivez `artifacts/sorafs/portal.pin.report.json`, ou facultatif
 `portal.pin.proposal.json`, le descripteur de basculement DNS (après soumissions),
 et le bundle de liaison de passerelle (`portal.gateway.binding.json` mais le bloc d'en-têtes)
 pour que les équipes de gouvernance, de réseautage et d'exploitation puissent comparer l'ensemble des preuves
 sem réviser les journaux de CI.

Configurer `PIN_ALIAS`, `PIN_ALIAS_NAMESPACE`, `PIN_ALIAS_NAME`, et (en option)
`PIN_ALIAS_PROOF_PATH` avant d'invoquer le script. Usa `--skip-submit` pour sec
court; Le workflow de GitHub est décrit ici alternativement via l'entrée `perform_submit`.

## Étape 8 - Spécifications publiques OpenAPI et paquets SBOMDOCS-7 nécessite la construction du portail, la spécification OpenAPI et les artefacts SBOM viajen
por o mesmo pipeline déterministe. Os helpers existentes cubren os tres:

1. **Regenera e assinatura a especación.**

 ```bash
 npm run sync-openapi -- --version=2025-q3 --mirror=current --latest
 cargo xtask openapi --sign docs/portal/static/openapi/manifest.json
 ```

 Pas une étiquette de publication via `--version=<label>` lorsque vous souhaitez conserver un instantané
 historique (par exemple `2025-q3`). O assistant, écris ou instantané-les
 `static/openapi/versions/<label>/torii.json`, je les vois
 `versions/current`, et enregistre les métadonnées (SHA-256, état du manifeste, et
 horodatage actualisé) dans `static/openapi/versions.json`. Le portail des développeurs
 Voici cet indice pour que les panneaux Swagger/RapiDoc puissent présenter un sélecteur de version
 et afficher le digest/assinatura associé à la ligne. Omitir `--version` manteau comme étiquettes du
 release previo intactas e apenas refresca os punteros `current` + `latest`.

 Le manifeste capture les résumés de SHA-256/BLAKE3 pour que le portail puisse être gravé
 en-têtes `Sora-Proof` pour `/reference/torii-swagger`.

2. **Emite SBOMs CycloneDX.** Le pipeline de publication espère que les SBOMs seront basés sur syft
 » deuxième `docs/source/sorafs_release_pipeline_plan.md`. Mantenha a salida
 avec les artefacts de construction :

 ```bash
 syft dir:build -o json > "$OUT"/portal.sbom.json
 syft file:docs/portal/static/openapi/torii.json -o json > "$OUT"/openapi.sbom.json
 ```

3. **Empaqueta chaque charge utile dans une CAR.**

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
 ```Sigue os mesmos etapas de `manifest build` / `manifest sign` que o sitio principal,
 ajuster l'alias de l'artefact (par exemple, `docs-openapi.sora` pour une spécification et
 `docs-sbom.sora` pour le bundle SBOM ferme). Manter alias distincts mantem SoraDNS,
 GAR et tickets de restauration associés à la charge utile exacte.

4. **Soumettre et lier.** Réutiliser l'autorité existante et le bundle Sigstore, mais s'inscrire
 o tuple d'alias em o checklist de release para que os auditeurs rastreen que nom Sora
 mapea a que digest de manifeste.

Arquivar os manifestes de spec/SBOM junto al build del portal asegura que cada ticket de
la version contient l'ensemble complet des artefacts sans réexécuter le packer.

### Helper automatique (CI/script du package)

`./ci/package_docs_portal_sorafs.sh` codifie les étapes 1 à 8 pour l'élément de la feuille de route
**DOCS-7** peut être utilisé avec un simple commandant. Ô assistant :

- exécuter une préparation requise pour le portail (`npm ci`, synchronisation OpenAPI/norito, tests de widgets) ;
- émettre les CAR et les pages de manifeste du portail, OpenAPI et SBOM via `sorafs_cli` ;
- exécuter facultativement `sorafs_cli proof verify` (`--proof`) et l'Assinatura Sigstore
 (`--sign`, `--sigstore-provider`, `--sigstore-audience`) ;
- déjà tous les artefacts sous `artifacts/devportal/sorafs/<timestamp>/` e
 écrivez `package_summary.json` pour que CI/tooling de release puisse intégrer le bundle ; e
- rechercher `artifacts/devportal/sorafs/latest` pour pouvoir effectuer une exécution plus récente.Exemple (pipeline complet avec Sigstore + PoR) :

```bash
./ci/package_docs_portal_sorafs.sh \
 --proof \
 --sign \
 --sigstore-provider=github-actions \
 --sigstore-audience=sorafs-devportal
```

Signale un problème :

- `--out <dir>` - remplacement de la racine des objets (manteau par défaut avec horodatage).
- `--skip-build` - réutiliser un `docs/portal/build` existant (utile lorsque CI ne peut pas
 reconstruire des miroirs hors ligne).
- `--skip-sync-openapi` - omettre `npm run sync-openapi` lorsque `cargo xtask openapi`
 je ne peux pas trouver crates.io.
- `--skip-sbom` - éviter d'appeler `syft` lorsque le binaire n'est pas installé (le script est imprimé)
 une publicité sur votre lieu).
- `--proof` - exécuter `sorafs_cli proof verify` para cada par CAR/manifesto. Les charges utiles de
 plusieurs archives nécessitent un support de chunk-plan dans la CLI, donc ce drapeau est déjà présent
 semestablecer si encuentras errores de `plan chunk count` et vérifier manuellement quando
 llegue o porte en amont.
- `--sign` - invoque `sorafs_cli manifest sign`. Prouvez un jeton avec
 `SIGSTORE_ID_TOKEN` (ou `--sigstore-token-env`) ou déjà que la CLI l'obtenga utilise
 `--sigstore-provider/--sigstore-audience`.Quando envies artefactos de produccion usa `docs/portal/scripts/sorafs-pin-release.sh`.
Maintenant, j'utilise le portail, OpenAPI et les charges utiles SBOM, le manifeste d'assistanat et les métadonnées enregistrées
extra de actifs em `portal.additional_assets.json`. L'aide vous permet d'accéder à mes boutons optionnels
utilisé par l'emballeur de CI pour les nouveaux commutateurs `--openapi-*`, `--portal-sbom-*`, et
`--openapi-sbom-*` pour attribuer des tuples d'alias à un artefact, remplacer la source SBOM via
`--openapi-sbom-source`, omettre les charges utiles ciertos (`--skip-openapi`/`--skip-sbom`),
et ajouter un binaire `syft` par défaut avec `--syft-bin`.

Le script expose chaque commande à exécuter ; copia o log em o ticket de release
avec `package_summary.json` pour que les réviseurs puissent comparer les résumés de CAR, les métadonnées de
plan, et les hachages du bundle Sigstore sans réviser la sortie du shell ad hoc.

## Étape 9 - Vérification du portail + SoraDNS

Avant d'annoncer un basculement, vérifiez que le nouveau pseudonyme se résout via SoraDNS et que les passerelles
fresques engrapan provas :

1. **Exécutez la porte de la sonde.** `ci/check_sorafs_gateway_probe.sh` ejercita
 Démo des appareils contra os `cargo xtask sorafs-gateway-probe`
 `fixtures/sorafs_gateway/probe_demo/`. Pour exécuter des tâches réelles, indiquez ou sondez l'objet du nom d'hôte :

 ```bash
 ./ci/check_sorafs_gateway_probe.sh -- \
 --gatewae "https://docs.sora/.well-known/sorafs/manifest" \
 --header "Accept: application/json" \
 --gar fixtures/sorafs_gateway/probe_demo/demo.gar.jws \
 --gar-kee "demo-gar=$(<fixtures/sorafs_gateway/probe_demo/gar_pub.hex>)" \
 --host "docs.sora" \
 --report-json artifacts/sorafs_gateway_probe/ci/docs.json
 ```

 O sonde décodifica `Sora-Name`, `Sora-Proof`, et `Sora-Proof-Status` segun
 `docs/source/sorafs_alias_policy.md` et tombe quand le résumé du manifeste,
 os TTL o os liaisons GAR se desvian.Pour les contrôles ponctuels légers (par exemple, lorsque seul le lot de liaisons
   modifié), exécutez `cargo xtask soradns-verify-binding --binding <portal.gateway.binding.json> --alias "<alias>" --hostname "<gateway-host>" --proof-status ok --manifest-json <portal.manifest.json>`.
   L'assistant valide le bundle de liaison capturé et est pratique pour la libération
   des billets qui nécessitent uniquement une confirmation contraignante au lieu d’un exercice d’enquête complet.

2. **Capturez les preuves de forage.** Pour les exercices de l'opérateur ou les simulacres de PagerDuty, affichez
 o sonde avec `scripts/telemetry/run_sorafs_gateway_probe.sh --scenario
 devportal-rollout -- ...`. Le wrapper garde les en-têtes/journaux sous
 `artifacts/sorafs_gateway_probe/<stamp>/`, mise à jour `ops/drill-log.md`, et
 (facultatif) supprimez les hooks de restauration ou les charges utiles PagerDuty. Établissement
 `--host docs.sora` pour valider votre itinéraire SoraDNS à la place du codeur en dur d'une adresse IP.3. **Vérifier les liaisons DNS.** Lorsque la gouvernance est publique ou la preuve de l'alias, enregistrée ou archivée GAR
 référencé par la sonde (`--gar`) et ajouté à la preuve de libération. Os propriétaires du résolveur
 Vous pouvez réfléchir à l'entrée mémo via `tools/soradns-resolver` pour garantir l'entrée dans le cache
 respectez le nouveau manifeste. Avant d'analyser JSON, exécutez
 `cargo xtask soradns-verify-gar --gar <path> --name <alias> [--manifest-cid <cid>] [--telemetry-label <label>]`
 pour que la carte détermine l'hôte, les métadonnées du manifeste et les étiquettes de télémètre
 valider hors ligne. L'assistant peut émettre un CV `--json-out` avec le GAR firmado para que os
 les réviseurs ont des preuves vérifiables sem ouvrir le binaire.
 Quando expurgue un GAR novo, préfère
 `cargo xtask soradns-gar-template --name <alias> --manifest <portal.manifest.json> --telemetry-label <label> ...`
 (voir `--manifest-cid <cid>` lorsque le fichier du manifeste n'est pas disponible). Ô aide
 maintenant, dérivez le CID **e** ou digérez BLAKE3 directement du manifeste JSON, enregistrez les espaces en blanc,
 indicateurs de déduplication `--telemetry-label` répétés, ordonnés comme étiquettes, et émettent des modèles de système d'exploitation par défaut
 de CSP/HSTS/Permissions-Police avant d'écrire le JSON pour que la charge utile soit déterministe
 même lorsque les opérateurs capturent les étiquettes des coquilles distinctes.

4. **Observez les métriques d'alias.** Mantenha `torii_sorafs_alias_cache_refresh_duration_ms`
 et `torii_sorafs_gateway_refusals_total{profile="docs"}` sur l'écran en quantité ou la sonde
 corréler; La série ambas est gravée sur `dashboards/grafana/docs_portal.json`.

## Étape 10 - Surveillance et ensemble de preuves- **Tableaux de bord.** Exporta `dashboards/grafana/docs_portal.json` (SLO du portail),
 `dashboards/grafana/sorafs_gateway_observability.json` (latence du portail +
 santé de preuve), et `dashboards/grafana/sorafs_fetch_observability.json`
 (salud del orchestrator) pour chaque sortie. Anexe os exports JSON al ticket de
 release pour que les réviseurs puissent reproduire les requêtes de Prometheus.
- **Archivos de sonde.** Conserva `artifacts/sorafs_gateway_probe/<stamp>/` dans git-annex
 ou votre seau de preuves. Inclut le résumé de la sonde, les en-têtes et la charge utile PagerDuty
 capturé par le script de télémétrie.
- **Bundle de release.** Garder les résumés de CAR del portal/SBOM/OpenAPI, bundles de manifeste,
 sociétés Sigstore, `portal.pin.report.json`, journaux de la sonde Try-It et rapports de vérification de lien
 j'ai un tapis avec l'horodatage (par exemple, `artifacts/sorafs/devportal/20260212T1103Z/`).
- **Drill log.** Quand les sondes font partie d'un foret, déjà que
 `scripts/telemetry/run_sorafs_gateway_probe.sh` agréger les entrées à `ops/drill-log.md`
 pour que la preuve soit satisfaisante pour les exigences du réseau SNNet-5.
- **Liens du ticket.** Référence aux ID du panneau de Grafana ou aux exportations PNG ajoutées au ticket
 de changement, en même temps que l'itinéraire du rapport de sonde, pour que les réviseurs puissent croiser les SLO
 j'ai accès à un shell.

## Etapa 11 - Drill de fetch multi-source et preuves de tableau de bord

Publier le SoraFS nécessite désormais une preuve de récupération multi-source (DOCS-7/SF-6)
ainsi que les preuves des DNS/gatewae antérieures. Après avoir fijar ou manifeste:1. **Exécutez `sorafs_fetch` contre le manifeste en direct.** Utiliser nos artefacts de plan/manifeste
 produits dans les étapes 2-3 mais comme crédits de passerelle émis pour chaque fournisseur. Persister
 aujourd'hui, pour que les auditeurs puissent reproduire le rastro de la décision de l'orchestrateur :

 ```bash
 OUT=artifacts/sorafs/devportal
 FETCH_OUT="$OUT/fetch/$(date -u +%E%m%dT%H%M%SZ)"
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

 - Cherchez premièrement les publicités des fournisseurs référencées par le manifeste (par exemple
 `sorafs_cli manifest describe --provider-adverts-out artifacts/sorafs/provider_adverts/`)
 et pasalos via `--provider-advert name=path` pour que le tableau de bord puisse évaluer les fenêtres
 de capacité de forme déterministe. États-Unis
 `--allow-implicit-provider-metadata` **apenas** quando reproduit les luminaires em CI ; exercices d'os
 de produccion doit citar os advertados que llegaron com o pin.
 - Quand le manifeste fait référence à des régions supplémentaires, répétez la commande avec les tuples de
 fournisseurs correspondants pour que chaque cache/alias ait un artefact à récupérer associé.

2. **Arquiver comme salidas.** Guarda `scoreboard.json`,
 `providers.ndjson`, `fetch.json`, et `chunk_receipts.ndjson` sous le tapis de preuve du
 libération. Ces archives capturent la pondération des pairs, ou le budget de retraite, la latence EWMA, et
 les reçus du morceau que le paquet de gouvernance doivent retenir pour SF-7.3. **Actualiser la télémétrie.** Importé comme moyen de récupérer le tableau de bord **SoraFS Récupérer
 Observabilité** (`dashboards/grafana/sorafs_fetch_observability.json`),
 observer `torii_sorafs_fetch_duration_ms`/`_failures_total` et les panneaux de rang du fournisseur
 para anomalies. Afficher les instantanés du panneau Grafana sur le ticket de sortie en même temps que l'itinéraire du
 tableau de bord.

4. **Fumez comme règle d'alerte.** Exécutez `scripts/telemetry/test_sorafs_fetch_alerts.sh`
 pour valider le paquet d'alertes Prometheus avant de terminer la publication. Anexe a salida de
 promtool al ticket pour que les réviseurs DOCS-7 confirment les alertes de décrochage et
 armadas siguen à fournisseur lent.

5. **Câble avec CI.** Le flux de travail de la broche du portail porte sur l'étapa `sorafs_fetch` detras del input
 `perform_fetch_probe` ; habilitalo para execucoes de staging/production para que a evidencia de
 chercher se produzca junto al bundle de manifeste sem manuel d’intervention. Os perce le podcast local
 réutiliser mon script en exportant les jetons de passerelle et en installant `PIN_FETCH_PROVIDERS`
 une liste de fournisseurs séparés par des virgules.

## Promotion, observabilité et restauration1. **Promocao :** mantenha alias séparés pour la mise en scène et la production. Promotion réexécutée
 `manifest submit` avec mon manifeste/bundle, modifié
 `--alias-namespace/--alias-name` pour identifier l'alias de production. Esto evita
 reconstruire ou ré-assister une fois que QA aprueba la broche de mise en scène.
2. **Surveillance :** importer le tableau de bord du registre des broches
 (`docs/source/grafana_sorafs_pin_registry.json`) sondes mas os spécifiques au portail
 (version `docs/portal/docs/devportal/observability.md`). Alerte à la dérive des sommes de contrôle,
 sondes fallidos ou picos de retre de proof.
3. **Rollback :** pour revenir en arrière, renvoyer le manifeste précédent (ou retirer ou alias actuel) en utilisant
 `sorafs_cli manifest submit --alias ... --retire`. Mantenha toujours ou le dernier bundle
 Conocido comme bon et le CV CAR pour que, comme preuve de rollback, vous puissiez recréer si
 les journaux de CI se tournent.

## Plante de workflow CI

Au minimum, votre pipeline doit être :

1. Build + Lint (`npm ci`, `npm run build`, génération de sommes de contrôle).
2. Empacotar (`car pack`) et manifestes informatiques.
3. Utilisez le jeton OIDC du travail (`manifest sign`).
4. Subir artefactos (CAR, manifeste, bundle, plan, résumés) para auditoria.
5. Envoyer le registre des broches :
 - Demandes d'extraction -> `docs-preview.sora`.
 - Tags / branches protégées -> promocao de alias de produccion.
6. Exécuter les sondes + portes de vérification de la preuve avant la fin.

`.github/workflows/docs-portal-sorafs-pin.yml` connectez toutes ces étapes pour publier des manuels.
O flux de travail :- construire/tester un portail,
- empaqueta ou build via `scripts/sorafs-pin-release.sh`,
- Assinatura/vérifique le bundle de manifeste en utilisant GitHub OIDC,
- sous-rubrique CAR/manifesto/bundle/plan/resumos como artefacts, e
- (facultatif) envie du manifeste + alias contraignant quand il y a des secrets.

Configurez les suites du référentiel de secrets/variables avant de supprimer le travail :

| Nom | Proposé |
| --- | --- |
| `DOCS_SORAFS_TORII_URL` | Hôte Torii qui expose `/v1/sorafs/pin/register`. |
| `DOCS_SORAFS_SUBMITTED_EPOCH` | Identificateur d'époque enregistré avec les soumissions. |
| `DOCS_SORAFS_AUTHORITY` / `DOCS_SORAFS_PRIVATE_KEY` | Autoridad de assinatura para o soumettre del manifeste. |
| `DOCS_SORAFS_ALIAS_NAMESPACE` / `DOCS_SORAFS_ALIAS_NAME` | Tuple d'alias inscrit dans le manifeste lorsque `perform_submit` est `true`. |
| `DOCS_SORAFS_ALIAS_PROOF_B64` | Bundle de preuves d'alias codifiés en base64 (facultatif ; omettre la liaison d'alias). |
| `DOCS_ANALYTICS_*` | Points de terminaison d'analyse/de sonde existants réutilisés pour d'autres flux de travail. |

Disparaissez le flux de travail via une interface utilisateur d'actions :

1. Prouvez `alias_label` (par exemple, `docs.sora.link`), `proposal_alias` facultatif,
 Il s'agit d'un remplacement facultatif de `release_tag`.
2. Deja `perform_submit` se marcar pour générer des artefacts sem tocar Torii
 (utile pour dre runs) ou habilité à publier directement l'alias configuré.`docs/source/sorafs_ci_templates.md` contient une documentation sur les aides génériques de CI pour
les projets autour de ce dépôt, mais le workflow du portail doit être une option préférée
para libère del dia a dia.

## Liste de contrôle

-[ ] `npm run build`, `npm run test:*`, et `npm run check:links` sont en vert.
-[ ] `build/checksums.sha256` et `build/release.json` capturés dans les artefacts.
- [ ] CAR, plan, manifeste et résumé généré sous `artifacts/`.
- [ ] Bundle Sigstore + journaux d'assinatura separada almacenados com.
-[ ] `portal.manifest.submit.summary.json` et `portal.manifest.submit.response.json`
 capturados quando hae soumissions.
-[ ] `portal.pin.report.json` (et `portal.pin.proposal.json` en option)
 archivé conjointement avec les artefacts CAR/manifeste.
-[ ] Journaux de `proof verify` et `manifest verify-signature` archivés.
-[ ] Tableaux de bord de Grafana actualisés + sondes Try-It exitosos.
- [ ] Notes de rollback (ID du manifeste précédent + résumé de l'alias) ajoutées au
 ticket de sortie.