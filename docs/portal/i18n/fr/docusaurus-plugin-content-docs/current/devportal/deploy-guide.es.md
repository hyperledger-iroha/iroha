---
lang: fr
direction: ltr
source: docs/portal/docs/devportal/deploy-guide.es.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

## Panorama général

Ce playbook convertit les éléments de la feuille de route **DOCS-7** (publication de SoraFS) et **DOCS-8**
(automatisation de la broche de CI/CD) dans une procédure accessible au portail des développeurs.
Cubre la phase de construction/peluchage, l'empaquetado SoraFS, la firma de manifiestos con Sigstore,
la promotion de l'alias, la vérification et les exercices de restauration pour chaque aperçu et sortie
mer reproductible et vérifiable.

Le flux suppose que vous avez le binaire `sorafs_cli` (construit avec `--features cli`), accés à
un point de terminaison Torii avec autorisations de registre de broches et informations d'identification OIDC pour Sigstore. Garde secrète
une grande vie (`IROHA_PRIVATE_KEY`, `SIGSTORE_ID_TOKEN`, jetons de Torii) dans votre coffre-fort de CI ; las
Les émissions locales peuvent être chargées à partir des exportations du shell.

## Prérequis- Nœud 18.18+ avec `npm` ou `pnpm`.
- `sorafs_cli` à partir de `cargo run -p sorafs_car --features cli --bin sorafs_cli`.
- URL de Torii qui expose `/v1/sorafs/*` mais un compte/clave privé d'autorité qui peut envoyer des manifestes et des alias.
- Emisor OIDC (GitHub Actions, GitLab, Workload Identity, etc.) pour émettre un `SIGSTORE_ID_TOKEN`.
- Facultatif : `examples/sorafs_cli_quickstart.sh` pour les essais à sec et `docs/source/sorafs_ci_templates.md` pour l'échafaudage des workflows de GitHub/GitLab.
- Configurez les variables OAuth de Try it (`DOCS_OAUTH_*`) et exécutez-les
  [liste de contrôle pour le renforcement de la sécurité](./security-hardening.md) avant de promouvoir une build
  hors du laboratoire. La construction du portail tombe maintenant lorsque ces variables échouent
  o lorsque les boutons de TTL/polling sont placés hors des fenêtres appliquées ; exporter
  `DOCS_OAUTH_ALLOW_INSECURE=1` uniquement pour les aperçus locaux désactivés. Adjoint à la
  preuve du pen-test sur le ticket de sortie.

## Paso 0 - Capturer un paquet de proxy Essayez-le

Avant de promouvoir un aperçu sur Netlify ou sur la passerelle, en vendant les sources du proxy Essayez-le et
résumé du manifeste OpenAPI confirmé dans un paquet déterminé :

```bash
cd docs/portal
npm run release:tryit-proxy -- \
  --out ../../artifacts/tryit-proxy/$(date -u +%Y%m%dT%H%M%SZ) \
  --target https://torii.dev.sora \
  --label preview-2026-02-14
````scripts/tryit-proxy-release.mjs` copier les assistants de proxy/probe/rollback, vérifier la société
OpenAPI et écrivez `release.json` mais `checksums.sha256`. Ajouter ce paquet au ticket de
promotion de la passerelle Netlify/SoraFS pour que les réviseurs puissent reproduire les sources exactes
du proxy et des pistes de la cible Torii sans reconstruire. Le paquet peut également être enregistré si vous
les porteurs de provisions pour le client sont habilités (`allow_client_auth`) pour maintenir le plan
du déploiement et des règles CSP en synchronisation.

## Étape 1 - Construire et pelucher le portail

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
- `build/release.json` - métadonnées (`tag`, `generated_at`, `source`) enregistrées dans chaque CAR/manifeste.

Archiva ambos archivos junto al CVen CAR para que los revisores puedan comparar artefactos de
aperçu sans reconstruction.

## Paso 2 - Empaqueter les actifs statiques

Exécuter l’empaquetador CAR contre le directeur de sortie Docusaurus. L'exemple d'en bas
écrivez todos los artefactos bajo `artifacts/devportal/`.

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

Le résumé JSON capture des contenus de morceaux, de résumés et de pistes de planification de preuve que
`manifest build` et les tableaux de bord de CI réutilisés après.

## Paso 2b - Compagnons Empaqueta OpenAPI et SBOMDOCS-7 nécessite de publier le site du portail, l'instantané OpenAPI et les charges utiles SBOM
comme des manifestes distincts pour que les passerelles puissent graver les en-têtes
`Sora-Proof`/`Sora-Content-CID` pour chaque article. El helper de release
(`scripts/sorafs-pin-release.sh`) vous avez mis le répertoire OpenAPI
(`static/openapi/`) et les SBOM émis via `syft` et les CAR séparés
`openapi.*`/`*-sbom.*` et enregistrer les métadonnées fr
`artifacts/sorafs/portal.additional_assets.json`. Pour exécuter le manuel du fluide,
répétez les étapes 2 à 4 pour chaque charge utile avec vos propres préfixes et étiquettes de métadonnées
(par exemple `--car-out "$OUT"/openapi.car` mais
`--metadata alias_label=docs.sora.link/openapi`). Registre cada par manifeste/alias
en Torii (site, OpenAPI, SBOM du portail, SBOM de OpenAPI) avant de changer de DNS pour que
el gateway peut servir de tests d'inscription pour tous les artefacts publiés.

## Paso 3 - Construire le manifeste

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

Ajustez les drapeaux politiques de la broche avant votre sortie (par exemple, `--pin-storage-class
chaud pour les canaris). La variante JSON est facultative mais pratique pour la révision du code.

## Étape 4 - Entreprise avec Sigstore

```bash
sorafs_cli manifest sign \
  --manifest "$OUT"/portal.manifest.to \
  --chunk-plan "$OUT"/portal.plan.json \
  --bundle-out "$OUT"/portal.manifest.bundle.json \
  --signature-out "$OUT"/portal.manifest.sig \
  --identity-token-provider github-actions \
  --identity-token-audience sorafs-devportal
```Le bundle enregistre le résumé du manifeste, les résumés de morceaux et un hachage BLAKE3 du jeton
OIDC ne persiste pas dans le JWT. Gardez le paquet comme la société séparée ; les promotions de
La production peut réutiliser les mêmes artefacts à la place de la rénovation. Les émissions locales
Vous pouvez remplacer les drapeaux du fournisseur avec `--identity-token-env` (ou établir
`SIGSTORE_ID_TOKEN` à l'intérieur) lorsqu'un assistant OIDC externe émet le jeton.

## Paso 5 - Envoyer le registre des broches

Envoyez le manifeste ferme (et le plan de morceaux) au Torii. Solliciter toujours un CV pour que
la entrada/alias résultante mer auditable.

```bash
sorafs_cli manifest submit \
  --manifest "$OUT"/portal.manifest.to \
  --chunk-plan "$OUT"/portal.plan.json \
  --torii-url "$TORII_URL" \
  --authority <katakana-i105-account-id> \
  --private-key "$IROHA_PRIVATE_KEY" \
  --submitted-epoch 20260101 \
  --alias-namespace docs \
  --alias-name sora.link \
  --alias-proof "$OUT"/docs.alias.proof \
  --summary-out "$OUT"/portal.submit.json \
  --response-out "$OUT"/portal.submit.response.json
```

Lorsque vous affichez un alias de prévisualisation ou Canary (`docs-preview.sora`), répétez le
envoyer avec un alias unique pour que QA puisse vérifier le contenu avant la promotion
production.

La liaison de l'alias nécessite trois champs : `--alias-namespace`, `--alias-name` et `--alias-proof`.
Governance produit le bundle de preuve (base64 ou octets Norito) lorsque vous répondez à la demande
del alias; garde les secrets de CI et expose comme fichier avant l'appel `manifest submit`.
Deja los flags de alias sin establecer cuando solo piensas fijar el manifiesto sin tocar DNS.

## Paso 5b - Générer une proposition de gouvernanceCada manifesteto debe viajar con a propuesta lista para Parliament para que cualquier ciudadano
Sora peut introduire le changement sans avoir à obtenir des informations d'identification privilégiées. Après les étapes
soumettre/signer, ejecuta :

```bash
sorafs_cli manifest proposal \
  --manifest "$OUT"/portal.manifest.to \
  --chunk-plan "$OUT"/portal.plan.json \
  --submitted-epoch 20260101 \
  --alias-hint docs.sora.link \
  --proposal-out "$OUT"/portal.pin.proposal.json
```

`portal.pin.proposal.json` capture les instructions canoniques `RegisterPinManifest`,
le résumé des morceaux, la politique et la piste d'alias. Adjoint au ticket de gouvernance ou à
portail Parlement pour que les délégués puissent comparer la charge utile sans reconstruire les artefacts.
Comme le commandant n'a aucun contact avec la clave de l'autorité de Torii, tout citoyen peut rédiger le
propuesta localement.

## Étape 6 - Vérifier les tests et la télémétrie

Après avoir vérifié, effectuez les étapes de vérification déterministes :

```bash
sorafs_cli proof verify \
  --manifest "$OUT"/portal.manifest.to \
  --car "$OUT"/portal.car \
  --summary-out "$OUT"/portal.proof.json

sorafs_cli manifest verify-signature \
  --manifest "$OUT"/portal.manifest.to \
  --bundle "$OUT"/portal.manifest.bundle.json \
  --chunk-plan "$OUT"/portal.plan.json
```- Vérifier `torii_sorafs_gateway_refusals_total` et
  `torii_sorafs_replication_sla_total{outcome="missed"}` pour anomalies.
- Exécutez `npm run probe:portal` pour exécuter le proxy Try-It et les liens enregistrés
  contra el contenido recien fijado.
- Capturer les preuves du moniteur décrites en
  [Publishing & Monitoring](./publishing-monitoring.md) pour la porte d'observabilité DOCS-3c
  il se satisfait des étapes de publication. El helper accepte désormais plusieurs entrées
  `bindings` (site, OpenAPI, SBOM du portail, SBOM de OpenAPI) et application `Sora-Name`/`Sora-Proof`/`Sora-Content-CID`
  dans l'objet hôte via le garde optionnel `hostname`. L'invocation d'abajo écris tanto un
  reprendre JSON unique comme le bundle de preuves (`portal.json`, `tryit.json`, `binding.json` et
  `checksums.sha256`) sous le répertoire de sortie :

  ```bash
  npm run monitor:publishing -- \
    --config ../../configs/docs_monitor.json \
    --json-out ../../artifacts/sorafs/preview-2026-02-14/monitoring/summary.json \
    --evidence-dir ../../artifacts/sorafs/preview-2026-02-14/monitoring
  ```

## Paso 6a - Planification certifiée de la passerelle

Dérivez le plan de SAN/challenge TLS avant de créer des paquets GAR pour l'équipement de passerelle et les
les arobadores de DNS révisent la même preuve. Le nouvel assistant reflète les informations d'automatisation
L'énumération DG-3 héberge des canons génériques, des SAN jolis hôtes, des étiquettes DNS-01 et des codes ACME recommandés :

```bash
cargo xtask soradns-acme-plan \
  --name docs.sora \
  --json-out artifacts/sorafs/portal.acme-plan.json
```Comité JSON conjointement avec le bundle de release (ou soumis au ticket de changement) pour les opérateurs
Vous pouvez déterminer les valeurs SAN dans la configuration `torii.sorafs_gateway.acme` de Torii et les réviseurs
de GAR peut confirmer les mapeos canonico/pretty sin re-ecutar derivaciones de host. Agréga
arguments `--name` supplémentaires pour chaque soufijo promu dans la même version.

## Paso 6b - Dérivez les cartes des hôtes canoniques

Avant les charges utiles des Templiers GAR, enregistrez la carte de l'hôte déterminant pour chaque alias.
`cargo xtask soradns-hosts` hashea cada `--name` sur votre étiquette canonique
(`<base32>.gw.sora.id`), émet le caractère générique requis (`*.gw.sora.id`) et dérive le joli hôte
(`<alias>.gw.sora.name`). Persistez la libération des artefacts de publication pour les réviseurs
DG-3 peut comparer la carte avec l'environnement GAR :

```bash
cargo xtask soradns-hosts \
  --name docs.sora \
  --json-out artifacts/sorafs/portal.canonical-hosts.json
```

Utilisez `--verify-host-patterns <file>` pour tomber rapidement lorsque vous avez un JSON de GAR ou une liaison de passerelle
omettez un des hôtes requis. El helper accepte plusieurs archives de vérification, haciendo
il facilite la tâche de la plante GAR comme le `portal.gateway.binding.json` gravé dans la même invocation :

```bash
cargo xtask soradns-hosts \
  --name docs.sora \
  --json-out artifacts/sorafs/portal.canonical-hosts.json \
  --verify-host-patterns artifacts/sorafs/portal.gar.json \
  --verify-host-patterns artifacts/sorafs/portal.gateway.binding.json
```Ajouter le CV JSON et le journal de vérification du ticket de changement de DNS/passerelle pour cela
Les auditeurs peuvent confirmer les hôtes canoniques, les caractères génériques et les jolis péchés en réexécutant les scripts.
Relancez le commande lorsqu'il ajoute de nouveaux alias au bundle pour les actualisations GAR
Hereden la misma evidencia.

## Étape 7 - Générer le descripteur de basculement DNS

Les transitions de production nécessitent un paquet de changement vérifiable. Après une soumission exitoso
(reliure de l'alias), l'assistant émet
`artifacts/sorafs/portal.dns-cutover.json`, capture :- métadonnées de liaison de l'alias (espace de noms/nom/preuve, résumé du manifeste, URL Torii,
  époque enviado, autoridad);
- contexte de publication (tag, alias label, rutas de manifiesto/CAR, plan de chunks, bundle Sigstore) ;
- punteros de verificacion (sonde commando, alias + point final Torii) ; oui
- champs optionnels de change-control (id de ticket, ventana de cutover, contacto ops,
  nom d'hôte/zone de production);
- métadonnées de promotion des itinéraires dérivées de l'en-tête `Sora-Route-Binding`
  (hôte canonique/CID, routes d'en-tête + liaison, commandes de vérification), garantissant la promotion
  GAR et les exercices de repli font référence à la même preuve ;
- les artefactos de plan de route générés (`gateway.route_plan.json`,
  modèles d'en-têtes et d'en-têtes de restauration optionnels) pour les tickets de changement et les crochets de
  les peluches de CI peuvent vérifier que chaque paquet DG-3 fait référence aux plans de promotion/rollback
  canonicos antes de la probación;
- métadonnées facultatives d'invalidation du cache (point de terminaison de purge, variable d'authentification, charge utile JSON,
  et exemple de commande `curl`); oui
- conseils de restauration apuntando al descriptor previo (tag de release y digest del manifiesto)
  pour que les tickets capturent une route de secours déterministe.

Lorsque la version nécessite des purges de cache, génère un plan canonique associé au descripteur de basculement :

```bash
cargo xtask soradns-cache-plan \
  --name docs.sora \
  --path / \
  --path /gateway/manifest.json \
  --auth-header Authorization \
  --auth-env CACHE_PURGE_TOKEN \
  --json-out artifacts/sorafs/portal.cache_plan.json
```Adjoint au `portal.cache_plan.json` résultant du paquet DG-3 pour les opérateurs
Il y a des hôtes/chemins déterministes (et les indices d'authentification qui coïncident) avec les requêtes émettrices `PURGE`.
La section facultative du cache du descripteur peut faire référence directement à ce fichier,
assurer l'alignement des réviseurs de contrôle des modifications de manière à ce que les points finaux soient propres
pendant un basculement.

Chaque paquet DG-3 nécessite également une liste de contrôle de promotion + restauration. Générala via
`cargo xtask soradns-route-plan` pour que les réviseurs de contrôle des modifications puissent suivre les étapes
détails exacts du contrôle en amont, du basculement et de la restauration par alias :

```bash
cargo xtask soradns-route-plan \
  --name docs.sora \
  --json-out artifacts/sorafs/gateway.route_plan.json
```

El `gateway.route_plan.json` émet une capture héberge des canonicos/pretty, enregistreurs de bilan de santé
par étapes, actualisations de la liaison GAR, purges de cache et actions de restauration. Inclut avec
les artefacts GAR/binding/cutover avant d'envoyer le ticket de changement pour que les opérations puissent les envoyer
aprobar los mismos pasos con guion.

`scripts/generate-dns-cutover-plan.mjs` impulsion est ce descripteur et est exécuté automatiquement à partir de
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

Indiquez les métadonnées facultatives via les variables d'entrée avant d'exécuter l'assistant de broche :| Variables | Proposé |
| --- | --- |
| `DNS_CHANGE_TICKET` | ID du ticket enregistré dans le descripteur. |
| `DNS_CUTOVER_WINDOW` | Ventana de cutover ISO8601 (par exemple, `2026-03-21T15:00Z/2026-03-21T15:30Z`). |
| `DNS_HOSTNAME`, `DNS_ZONE` | Nom d'hôte de production + zone autorisée. |
| `DNS_OPS_CONTACT` | Alias ​​de garde ou contacto de escalade. |
| `DNS_CACHE_PURGE_ENDPOINT` | Endpoint de purge du cache enregistré dans le descripteur. |
| `DNS_CACHE_PURGE_AUTH_ENV` | L'environnement va contenir le jeton de purge (par défaut : `CACHE_PURGE_TOKEN`). |
| `DNS_PREVIOUS_PLAN` | Itinéraire vers le descripteur de basculement précédent pour les métadonnées de restauration. |

Ajoutez le JSON à l'examen du changement de DNS pour que les demandeurs puissent vérifier les résumés du manifeste,
les liaisons d'alias et de commandes sondent sans devoir réviser les journaux de CI. Les drapeaux de CLI
`--dns-change-ticket`, `--dns-cutover-window`, `--dns-hostname`,
`--dns-zone`, `--ops-contact`, `--cache-purge-endpoint`,
`--cache-purge-auth-env`, et `--previous-dns-plan` ont prouvé les mêmes remplacements
lorsque l'assistant est exécuté hors de CI.

## Paso 8 - Émettre le squelette du fichier de zone du résolveur (facultatif)Lorsque la fenêtre de transition de la production est connue, le script de sortie peut émettre le
le squelette du fichier de zone SNS et l'extrait de code du résolveur automatique. Pas les registres DNS souhaités
et métadonnées via des variables d'entrée ou des options CLI ; l'assistant appelle un
`scripts/sns_zonefile_skeleton.py` immédiatement après avoir généré le descripteur de basculement.
Prouvez au moins une valeur A/AAAA/CNAME et le résumé GAR (BLAKE3-256 de la charge utile GAR ferme). Si la
zona/hostname est connu et `--dns-zonefile-out` est omite, l'assistant écris fr
`artifacts/sns/zonefiles/<zone>/<hostname>.json` et Llena
`ops/soradns/static_zones.<hostname>.json` comme extrait de résolveur.| Variable/indicateur | Proposé |
| --- | --- |
| `DNS_ZONEFILE_OUT`, `--dns-zonefile-out` | Itinéraire pour le squelette du fichier de zone généré. |
| `DNS_ZONEFILE_RESOLVER_SNIPPET`, `--dns-zonefile-resolver-snippet` | L'extrait de code du résolveur (par défaut : `ops/soradns/static_zones.<hostname>.json` lorsqu'il est omis). |
| `DNS_ZONEFILE_TTL`, `--dns-zonefile-ttl` | TTL appliqué aux registres générés (par défaut : 600 secondes). |
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
| `DNS_GAR_DIGEST`, `--dns-gar-digest` | Digest BLAKE3-256 (hex) de la charge utile GAR entreprise. Requerido cuando hay fixations de gateway. |

Le workflow de GitHub Actions comprend ces valeurs des secrets du référentiel pour chaque broche de
La production émet automatiquement les artefacts du fichier de zone. Configurer les secrets suivants
(les cordes peuvent contenir des listes séparées par des comas pour des champs multivaleurs) :

| Secrets | Proposé |
| --- | --- |
| `DOCS_SORAFS_DNS_HOSTNAME`, `DOCS_SORAFS_DNS_ZONE` | Nom d’hôte/zone de production passé à l’assistant. |
| `DOCS_SORAFS_DNS_OPS_CONTACT` | Alias ​​de garde enregistré dans le descripteur. |
| `DOCS_SORAFS_ZONEFILE_IPV4`, `DOCS_SORAFS_ZONEFILE_IPV6` | Registres IPv4/IPv6 à publier. |
| `DOCS_SORAFS_ZONEFILE_CNAME` | Ciblez CNAME facultatif. |
| `DOCS_SORAFS_ZONEFILE_SPKI` | Pins SPKI base64. |
| `DOCS_SORAFS_ZONEFILE_TXT` | Entrées TXT supplémentaires. |
| `DOCS_SORAFS_ZONEFILE_FREEZE_STATE/TICKET/EXPIRES_AT/NOTES` | Métadonnées de gel enregistrées sur le squelette. |
| `DOCS_SORAFS_GAR_DIGEST` | Digest BLAKE3 dans l'hexagone de la charge utile GAR ferme. |

Au disparar `.github/workflows/docs-portal-sorafs-pin.yml`, proportionner les entrées
`dns_change_ticket` et `dns_cutover_window` pour que le descripteur/fichier de zone tienne la fenêtre
correct. Dejarlos en blanco solo cuando éjecte des essais à sec.

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
```L'assistant enverra automatiquement le ticket de changement comme entrée TXT et ici le début du
il est possible de basculer avec l'horodatage `effective_at` pour éviter tout remplacement. Pour le flux
opérationnel complet, version `docs/source/sorafs_gateway_dns_owner_runbook.md`.

### Note sur la délégation DNS publique

Le squelette du fichier de zone définit seul les registres autorisés de la zone. Aun
vous devez configurer la délégation NS/DS de la zone père en votre registrateur ou
fournisseur DNS pour que le public Internet puisse trouver les serveurs de noms.

- Pour les basculements vers l'apex/TLD, avec ALIAS/ANAME (en fonction du fournisseur) ou
  Publica registros A/AAAA qui apunten a la IP anycast del gateway.
- Pour les sous-domaines, publier un CNAME hacia el Pretty Host dérivé
  (`<fqdn>.gw.sora.name`).
- L'hôte canonique (`<hash>.gw.sora.id`) reste en permanence sous le domaine de la passerelle
  et n'est pas publica dans votre zone publique.

### Plante des en-têtes de la passerelle

L'assistant de déploiement émet également `portal.gateway.headers.txt` et
`portal.gateway.binding.json`, deux artefacts qui satisfont aux exigences du DG-3 de
liaison de contenu de passerelle :- `portal.gateway.headers.txt` contient le blocage complet des en-têtes HTTP (y compris
  `Sora-Name`, `Sora-Content-CID`, `Sora-Proof`, CSP, HSTS et le descripteur
  `Sora-Route-Binding`) que les passerelles de frontière doivent être gravées dans chaque réponse.
- `portal.gateway.binding.json` enregistre la même information sous une forme lisible pour les machines
  pour que les tickets de changement et l'automatisation puissent comparer les liaisons hôte/cid sin
  raser la salida de shell.

Se génère automatiquement via
`cargo xtask soradns-binding-template`
et capturer l'alias, le résumé du manifeste et le nom d'hôte de la passerelle qui se trouve
passer à `sorafs-pin-release.sh`. Pour régénérer ou personnaliser le blocage des en-têtes,
éjecté :

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
les modèles d'en-têtes par défaut lorsqu'ils nécessitent des directives supplémentaires ;
combinez-les avec les commutateurs `--no-*` existants pour éliminer complètement un en-tête.

Ajouter l'extrait d'en-têtes à la demande de changement de CDN et alimenter le document JSON
dans le pipeline d'automatisation de la passerelle pour que la promotion réelle de l'hôte coïncide avec la
preuve de libération.

Le script de publication exécute automatiquement l'aide à la vérification pour que les
les billets DG-3 incluent toujours des preuves récentes. Voir l'exécution manuelle
lorsque vous modifiez la liaison JSON à la main :

```bash
cargo xtask soradns-verify-binding \
  --binding artifacts/sorafs/portal.gateway.binding.json \
  --alias docs.sora.link \
  --hostname docs.sora.link \
  --proof-status ok \
  --manifest-json artifacts/sorafs/portal.manifest.json
```La commande décodifie la charge utile `Sora-Proof` gravée, garantissant les métadonnées `Sora-Route-Binding`
coïncidant avec le CID du manifeste + le nom d'hôte, et tombe rapidement si un en-tête est supprimé.
Archivage de la sortie de console avec les autres objets de téléchargement toujours exécutés
le commandement de CI pour que les réviseurs DG-3 vérifient que la liaison a été validée
avant le basculement.

> **Intégration du descripteur DNS :** `portal.dns-cutover.json` maintenant incrusté dans une section
> `gateway_binding` apuntando a ces artefactos (rutas, content CID, estado de proof y el
> modèle littéral des en-têtes) **y** une estrofa `route_plan` référencée
> `gateway.route_plan.json` mas les modèles d'en-têtes principaux et de restauration. Inclut esos
> bloque en chaque ticket de changement DG-3 pour que les réviseurs puissent comparer les valeurs
> exactos de `Sora-Name/Sora-Proof/CSP` et confirmer que les avions de promotion/rollback
> coïncide avec le bundle de preuves sans ouvrir l'archive de build.

## Paso 9 - Exécuter un suivi de publication

L'élément de la feuille de route **DOCS-3c** nécessite des preuves continues du portail, du proxy Essayez-le et los
les liaisons de la passerelle sont maintenues saludables après une version. Exécuter le moniteur consolidé
immédiatement après les étapes 7-8 et connectez-vous à vos sondes programmées :

```bash
cd docs/portal
npm run monitor:publishing -- \
  --config ../../configs/docs_monitor.json \
  --json-out ../../artifacts/sorafs/${RELEASE_TAG}/monitoring/summary-$(date -u +%Y%m%dT%H%M%SZ).json \
  --evidence-dir ../../artifacts/sorafs/${RELEASE_TAG}/monitoring
```- `scripts/monitor-publishing.mjs` charge le fichier de configuration (version
  `docs/portal/docs/devportal/publishing-monitoring.md` pour l'esquema) et
  effectuer trois vérifications : sondes de chemins du portail + validation de CSP/Permissions-Policy,
  sondes du proxy Essayez-le (en sélectionnant facultativement votre point de terminaison `/metrics`), et le vérificateur
  de liaison de passerelle (`cargo xtask soradns-verify-binding`) qui exige maintenant la présence et
  la valeur attendue de Sora-Content-CID avec les vérifications de l'alias/du manifeste.
- La commande se termine avec une valeur non nulle lorsqu'une sonde est utilisée pour CI, les tâches cron ou les opérateurs
  Le runbook peut arrêter une version avant de promouvoir l'alias.
- Pasar `--json-out` écris un CV JSON unique avec l'état de la cible ; `--evidence-dir`
  émettre `summary.json`, `portal.json`, `tryit.json`, `binding.json` et `checksums.sha256` pour que
  Les réviseurs de gouvernance peuvent comparer les résultats sans rejeter les moniteurs. Archives
  ce répertoire sous `artifacts/sorafs/<tag>/monitoring/` avec le bundle Sigstore et le
  descripteur de basculement DNS.
- Inclut la sortie du moniteur, l'exportation de Grafana (`dashboards/grafana/docs_portal.json`),
  et l'ID de l'exercice d'Alertmanager sur le ticket de libération pour que le SLO DOCS-3c puisse être
  audité ensuite. Le playbook dédié au moniteur de publication vive en
  `docs/portal/docs/devportal/publishing-monitoring.md`.Les sondes du portail nécessitent HTTPS et rechazan URL base `http://` à moins que `allowInsecureHttp`
est-ce configuré dans la configuration du moniteur ; Manten Targets de production/staging en TLS et solo
autoriser le remplacement des aperçus locaux.

Automatisez le moniteur via `npm run monitor:publishing` et Buildkite/cron une fois sur le portail
est-ce en vivo. Le même commando, apposé sur les URL de production, alimente les chèques de santé
continus que SRE/Docs utilise entre les versions.

## Automatisation avec `sorafs-pin-release.sh`

`docs/portal/scripts/sorafs-pin-release.sh` encapsule les étapes 2-6. Est-ce :

1. archivez `build/` dans une archive tar déterministe,
2. exécutez `car pack`, `manifest build`, `manifest sign`, `manifest verify-signature`,
   et `proof verify`,
3. Exécutez facultativement `manifest submit` (y compris la liaison d'alias) lorsque vous avez des informations d'identification
   Torii, et
4. écrivez `artifacts/sorafs/portal.pin.report.json`, en option
  `portal.pin.proposal.json`, le descripteur de basculement DNS (après soumissions),
  et le bundle de liaison de la passerelle (`portal.gateway.binding.json` mais le blocage des en-têtes)
  pour que les équipes de gouvernance, de réseautage et d'exploitation puissent comparer l'ensemble des preuves
  sans réviser les journaux de CI.

Configurer `PIN_ALIAS`, `PIN_ALIAS_NAMESPACE`, `PIN_ALIAS_NAME`, et (en option)
`PIN_ALIAS_PROOF_PATH` avant d’invoquer le script. Usa `--skip-submit` pour sec
court; Le workflow de GitHub est décrit plus bas alternativement via l'entrée `perform_submit`.## Paso 8 - Spécifications publiques OpenAPI et paquets SBOM

DOCS-7 nécessite la construction du portail, la spécification OpenAPI et les artefacts SBOM viajen
par le mismo pipeline déterministe. Les assistants existent sous les trois :

1. **Régénérer et confirmer la spécification.**

   ```bash
   npm run sync-openapi -- --version=2025-q3 --mirror=current --latest
   cargo xtask openapi --sign docs/portal/static/openapi/manifest.json
   ```

   Pas une étiquette de publication via `--version=<label>` lorsque vous souhaitez conserver un instantané
   historique (par exemple `2025-q3`). L'assistant décrit l'instantané fr
   `static/openapi/versions/<label>/torii.json`, je l'écoute
   `versions/current`, et enregistre les métadonnées (SHA-256, état du manifeste, et
   timestamp actualisé) et `static/openapi/versions.json`. Le portail des développeurs
   voici cet indice pour que les panneaux Swagger/RapiDoc puissent présenter un sélecteur de version
   et afficher le résumé/la société associée en ligne. Omitir `--version` conserver les étiquettes du
   release previo intactas y solo refresca los punteros `current` + `latest`.

   Le formulaire de capture digère SHA-256/BLAKE3 pour que la passerelle puisse être gravée
   en-têtes `Sora-Proof` pour `/reference/torii-swagger`.

2. **Emite SBOMs CycloneDX.** Le pipeline de release vous attend SBOMs basados en syft
   » deuxième `docs/source/sorafs_release_pipeline_plan.md`. Garder la sortie
   avec les artefacts de construction :

   ```bash
   syft dir:build -o json > "$OUT"/portal.sbom.json
   syft file:docs/portal/static/openapi/torii.json -o json > "$OUT"/openapi.sbom.json
   ```

3. **Empaqueta cada payload dans un CAR.**

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
   ```Sigue los mismos pasos de `manifest build` / `manifest sign` que le site principal,
   ajuster l'alias de l'artefact (par exemple, `docs-openapi.sora` pour la spécification et
   `docs-sbom.sora` pour le bundle SBOM ferme). Mantener alias distincts mantiene SoraDNS,
   GAR et tickets de restauration associés à la charge utile exacte.

4. **Soumettre et lier.** Réutiliser l'autorité existante et le bundle Sigstore, mais s'inscrire
   le tuple d'alias et la liste de contrôle de publication pour que les auditeurs rastreen que le nom Sora
   mapea a que digest de manifiesto.

Archivar los manifestestos de spec/SBOM junto al build del portal asegura que cada ticket de
release contient l'ensemble complet des artefacts sans réexécuter le packer.

### Helper d'automatisation (CI/script de package)

`./ci/package_docs_portal_sorafs.sh` codifie les étapes 1 à 8 pour l'élément de la feuille de route
**DOCS-7** peut être exercé avec une commande solo. L'assistant :- exécuter la préparation requise du portail (`npm ci`, synchronisation OpenAPI/norito, tests de widgets) ;
- émettre les CAR et les pares de manifeste du portail, OpenAPI et SBOM via `sorafs_cli` ;
- éventuellement exécuter `sorafs_cli proof verify` (`--proof`) et la société Sigstore
  (`--sign`, `--sigstore-provider`, `--sigstore-audience`) ;
- déjà tous les artefacts bajo `artifacts/devportal/sorafs/<timestamp>/` et
  écrivez `package_summary.json` pour que CI/tooling de release puisse ingérer le bundle ; oui
- Refresca `artifacts/devportal/sorafs/latest` pour identifier l'exécution la plus récente.

Exemple (pipeline complet avec Sigstore + PoR) :

```bash
./ci/package_docs_portal_sorafs.sh \
  --proof \
  --sign \
  --sigstore-provider=github-actions \
  --sigstore-audience=sorafs-devportal
```

Signale un problème :- `--out <dir>` - remplace la racine des objets (par défaut, les tapis sont conservés avec l'horodatage).
- `--skip-build` - réutiliser un `docs/portal/build` existant (util lorsque CI ne peut pas
  reconstruire des miroirs hors ligne).
- `--skip-sync-openapi` - omettre `npm run sync-openapi` lorsque `cargo xtask openapi`
  je ne peux pas accéder à crates.io.
- `--skip-sbom` - évitez d'appeler le `syft` lorsque le binaire n'est pas installé (le script est imprimé
  une publicité sur votre lieu).
- `--proof` - exécuté `sorafs_cli proof verify` pour chaque voiture/manifeste. Les charges utiles de
  plusieurs archives nécessitent un support de chunk-plan dans la CLI, car c'est déjà ce flag
  ne pas établir si vous rencontrez des erreurs de `plan chunk count` et vérifier manuellement quand
  llegue el porte en amont.
- `--sign` - invoque `sorafs_cli manifest sign`. Prouvez un jeton con
  `SIGSTORE_ID_TOKEN` (ou `--sigstore-token-env`) ou déjà que la CLI l'obtenga utilise
  `--sigstore-provider/--sigstore-audience`.Cuando envie les artefactos de produccion usa `docs/portal/scripts/sorafs-pin-release.sh`.
Maintenant, emballez le portail, OpenAPI et les charges utiles SBOM, fermez chaque manifeste et enregistrez les métadonnées
extra de actifs en `portal.additional_assets.json`. L'aide permet d'accéder aux boutons optionnels
utilisé par l'emballeur de CI mais les nouveaux commutateurs `--openapi-*`, `--portal-sbom-*`, et
`--openapi-sbom-*` pour attribuer des tuples d'alias à un artefact, remplacer la source SBOM via
`--openapi-sbom-source`, omettre les charges utiles ciertos (`--skip-openapi`/`--skip-sbom`),
et trouver un binaire `syft` sans défaut avec `--syft-bin`.

Le script expose chaque commande qui est exécutée ; copier le journal et le ticket de sortie
avec `package_summary.json` pour que les réviseurs puissent comparer les résumés de CAR, les métadonnées de
plan, et les hachages du bundle Sigstore sans réviser la sortie du shell ad hoc.

## Étape 9 - Vérification de la passerelle + SoraDNS

Avant d'annoncer un basculement, vérifiez que le nouveau alias sera généré via SoraDNS et que les passerelles
fresques engrapan pruebas :

1. **Éjectez la porte de la sonde.** `ci/check_sorafs_gateway_probe.sh` éjecteur
   `cargo xtask sorafs-gateway-probe` contre les luminaires démo fr
   `fixtures/sorafs_gateway/probe_demo/`. Pour exécuter des tâches réelles, appuyez sur la sonde sur l'objet du nom d'hôte :

   ```bash
   ./ci/check_sorafs_gateway_probe.sh -- \
     --gateway "https://docs.sora/.well-known/sorafs/manifest" \
     --header "Accept: application/json" \
     --gar fixtures/sorafs_gateway/probe_demo/demo.gar.jws \
     --gar-key "demo-gar=$(<fixtures/sorafs_gateway/probe_demo/gar_pub.hex>)" \
     --host "docs.sora" \
     --report-json artifacts/sorafs_gateway_probe/ci/docs.json
   ```

   La sonde décodifie `Sora-Name`, `Sora-Proof`, et `Sora-Proof-Status` ensuite
   `docs/source/sorafs_alias_policy.md` et tombe lorsque le résumé du manifeste,
   les TTL ou les liaisons GAR sont desvian.Pour les contrôles ponctuels légers (par exemple, lorsque seul le lot de liaisons
   modifié), exécutez `cargo xtask soradns-verify-binding --binding <portal.gateway.binding.json> --alias "<alias>" --hostname "<gateway-host>" --proof-status ok --manifest-json <portal.manifest.json>`.
   L'assistant valide le bundle de liaison capturé et est pratique pour la libération
   des billets qui nécessitent uniquement une confirmation contraignante au lieu d’un exercice d’enquête complet.

2. **Capturez les preuves de forage.** Pour les exercices de l'opérateur ou les simulacres de PagerDuty, affichez
   la sonde avec `scripts/telemetry/run_sorafs_gateway_probe.sh --scenario
   devportal-rollout -- ...`. Le wrapper garde les en-têtes/journaux sous
   `artifacts/sorafs_gateway_probe/<stamp>/`, mise à jour `ops/drill-log.md`, et
   (facultatif) supprimez les hooks de restauration ou les charges utiles PagerDuty. Établissement
   `--host docs.sora` pour valider la route SoraDNS à la place du codeur en dur une IP.3. **Vérifier les liaisons DNS.** Lorsque la gouvernance publie la preuve de l'alias, enregistre le fichier GAR
   référencé par la sonde (`--gar`) et ajouté à la preuve de libération. Les propriétaires du résolveur
   Vous pouvez réfléchir à l'entrée correspondante via `tools/soradns-resolver` pour garantir les entrées dans le cache
   respectez le manifeste nouveau. Avant d'ajouter le JSON, exécutez
   `cargo xtask soradns-verify-gar --gar <path> --name <alias> [--manifest-cid <cid>] [--telemetry-label <label>]`
   pour que la carte détermine l'hôte, les métadonnées du manifeste et les étiquettes de télémétrie
   valider hors ligne. L'assistant peut émettre un CV `--json-out` associé au GAR firmado pour que les
   les réviseurs ont des preuves vérifiables sans ouvrir le binaire.
  Cuando rédige un nouveau GAR, préfère
  `cargo xtask soradns-gar-template --name <alias> --manifest <portal.manifest.json> --telemetry-label <label> ...`
  (voir `--manifest-cid <cid>` seulement lorsque le fichier du manifeste n'est pas disponible). L'assistant
  Maintenant, dérivez le CID **y** le digest BLAKE3 directement du manifeste JSON, enregistrez les espaces en blanc,
  drapeaux de déduplication `--telemetry-label` répétés, ordre des étiquettes, et émission des modèles par défaut
  de CSP/HSTS/Permissions-Policy avant d'écrire le JSON pour que la charge utile soit déterministe
  également lorsque les opérateurs capturent les étiquettes des coquilles distinctes.

4. **Observation des métriques d'alias.** Manten `torii_sorafs_alias_cache_refresh_duration_ms`
   y `torii_sorafs_gateway_refusals_total{profile="docs"}` sur l'écran au milieu de la sonde
   corréler; La série ambas est gravée sur `dashboards/grafana/docs_portal.json`.## Paso 10 - Surveillance et ensemble de preuves

- **Tableaux de bord.** Exporta `dashboards/grafana/docs_portal.json` (SLO du portail),
  `dashboards/grafana/sorafs_gateway_observability.json` (latence de la passerelle +
  salut de preuve), et `dashboards/grafana/sorafs_fetch_observability.json`
  (salud del orchestrator) pour chaque sortie. Ajouter les exportations JSON au ticket de
  release pour que les réviseurs puissent reproduire les requêtes de Prometheus.
- **Archivos de sonde.** Conserva `artifacts/sorafs_gateway_probe/<stamp>/` en git-annex
  ou votre seau de preuves. Inclut le résumé de la sonde, les en-têtes et la charge utile PagerDuty
  capturé par le script de télémétrie.
- **Bundle de release.** Garder les CV du CAR du portail/SBOM/OpenAPI, bundles de manifeste,
  firmas Sigstore, `portal.pin.report.json`, journaux de la sonde Try-It et rapports de vérification de lien
  j'ai un tapis avec horodatage (par exemple, `artifacts/sorafs/devportal/20260212T1103Z/`).
- **Drill log.** Lorsque les sondes font partie d'un foret, déjà que
  `scripts/telemetry/run_sorafs_gateway_probe.sh` agréger les entrées à `ops/drill-log.md`
  pour que la même preuve satisfasse aux exigences du caos SNNet-5.
- **Liens du ticket.** Référence aux ID du panneau de Grafana ou aux exportations PNG ajoutées au ticket
  de changement, en même temps que la route du rapport de sonde, pour que les réviseurs puissent croiser les SLO
  sans accéder à une coquille.

## Paso 11 - Drill de fetch multi-source et preuves du tableau de bordPublier le SoraFS nécessite désormais une preuve de récupération multi-source (DOCS-7/SF-6)
en même temps que les tests DNS/passerelle antérieurs. Après avoir fijar le manifeste:

1. **Éjecter `sorafs_fetch` contre le manifeste en direct.** Utiliser les mêmes artefacts de plan/manifeste
   produits en los Pasos 2-3 mais les informations d'identification de la passerelle émises pour chaque fournisseur. Persister
   toute la sortie pour que les auditeurs puissent reproduire le rastro de décision de l'orchestrateur :

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

   - Recherchez d'abord les publicités des fournisseurs référencées par le manifeste (par exemple
     `sorafs_cli manifest describe --provider-adverts-out artifacts/sorafs/provider_adverts/`)
     et pasalos via `--provider-advert name=path` pour que le tableau de bord puisse évaluer les fenêtres
     de capacité de forme déterministe. États-Unis
     `--allow-implicit-provider-metadata` **solo** cuando reproduit les luminaires en CI ; les exercices
     de produccion doit citar los adverts firmados que llegaron con el pin.
   - Lorsque le manifeste fait référence à des régions supplémentaires, répétez la commande avec les tuples de
     fournisseurs correspondants pour que chaque cache/alias ait un artefact de récupération associé.

2. **Archiva las salidas.** Garde `scoreboard.json`,
   `providers.ndjson`, `fetch.json` et `chunk_receipts.ndjson` sous le tapis de preuve du
   libération. Ces archives capturent la pondération des pairs, le budget de nouvelle tentative, la latence EWMA et les
   les reçus par morceau que le paquet de gouvernance doit retenir pour SF-7.3. **Actualiser la télémétrie.** Importer les sorties de récupération dans le tableau de bord **SoraFS Récupérer
   Observabilité** (`dashboards/grafana/sorafs_fetch_observability.json`),
   observer `torii_sorafs_fetch_duration_ms`/`_failures_total` et les panneaux de rang du fournisseur
   para anomalies. Afficher les instantanés du panneau Grafana sur le ticket de sortie avec la route du
   tableau de bord.

4. **Fumez les règles d'alerte.** Éjection `scripts/telemetry/test_sorafs_fetch_alerts.sh`
   pour valider le paquet d'alertes Prometheus avant de supprimer la version. Adjoint à la sortie de
   promtool al ticket pour que les réviseurs DOCS-7 confirment que les alertes de décrochage et
   armadas siguen à fournisseur lent.

5. **Câble en CI.** Le flux de travail des broches du portail maintient une étape `sorafs_fetch` derrière l'entrée
   `perform_fetch_probe` ; habilitalo para ejecuciones de staging/produccion para que la preuve de
   chercher se produzca junto al bundle de manifiesto sans manuel d’intervention. Les exercices locaux peuvent se faire
   réutiliser le même script, exporter les jetons de la passerelle et configurer `PIN_FETCH_PROVIDERS`
   sur la liste des fournisseurs séparés par des virgules.

## Promotion, observabilité et restauration1. **Promotion :** manten alias séparés pour la mise en scène et la production. Promociona re-ejecutando
   `manifest submit` avec le même manuel/bundle, à modifier
   `--alias-namespace/--alias-name` pour identifier l'alias de production. Esto evita
   reconstruire ou re-firmer une fois que QA aprueba la broche de mise en scène.
2. **Monitoreo :** importer le tableau de bord du registre des broches
   (`docs/source/grafana_sorafs_pin_registry.json`) plus les sondes spécifiques au portail
   (version `docs/portal/docs/devportal/observability.md`). Alerte en dérive des sommes de contrôle,
   sondes fallidos ou picos de retry de proof.
3. **Rollback :** pour revenir en arrière, renvoyer le manifeste précédent (ou retirer l'alias actuel) en utilisant
   `sorafs_cli manifest submit --alias ... --retire`. Gardez toujours le dernier bundle
   Conocido comme bon et le CV CAR pour que les essais de rollback puissent recréer si
   los logs de CI se rotan.

## Plante de workflow CI

Au minimum, votre pipeline doit être :

1. Build + Lint (`npm ci`, `npm run build`, génération de sommes de contrôle).
2. Empaquetar (`car pack`) et manifestes informatiques.
3. Fermez en utilisant le jeton OIDC du travail (`manifest sign`).
4. Subir artefactos (CAR, manifeste, bundle, plan, résumés) para auditoria.
5. Envoyer le registre des broches :
   - Demandes d'extraction -> `docs-preview.sora`.
   - Tags / branches protégées -> promotion d'alias de production.
6. Exécuter des sondes + des portes de vérification de preuve avant la fin.

`.github/workflows/docs-portal-sorafs-pin.yml` connectez toutes ces étapes pour publier des manuels.
El flux de travail :- construire/tester le portail,
- empaqueta el build via `scripts/sorafs-pin-release.sh`,
- confirmer/vérifier le bundle de manifeste en utilisant GitHub OIDC,
- sous le CAR/manifiesto/bundle/plan/resumenes comme artefacts, et
- (optionnellement) envoyer le manifeste + alias contraignant cuando hay secrets.

Configurez les secrets/variables du référentiel suivants avant de supprimer le travail :

| Nom | Proposé |
| --- | --- |
| `DOCS_SORAFS_TORII_URL` | Hôte Torii qui expose `/v1/sorafs/pin/register`. |
| `DOCS_SORAFS_SUBMITTED_EPOCH` | Identificateur d'époque enregistré avec soumissions. |
| `DOCS_SORAFS_AUTHORITY` / `DOCS_SORAFS_PRIVATE_KEY` | Autoridad de firma para el submit del manifiesto. |
| `DOCS_SORAFS_ALIAS_NAMESPACE` / `DOCS_SORAFS_ALIAS_NAME` | Tuple d'alias inscrit dans le manifeste lorsque `perform_submit` est `true`. |
| `DOCS_SORAFS_ALIAS_PROOF_B64` | Bundle de preuves d'alias codifiés en base64 (facultatif ; omettre la liaison d'alias). |
| `DOCS_ANALYTICS_*` | Points de terminaison d'analyse/de sonde existants réutilisés pour d'autres flux de travail. |

Supprimer le workflow via l'interface utilisateur des actions :

1. Provee `alias_label` (par exemple, `docs.sora.link`), `proposal_alias` facultatif,
   et un remplacement facultatif de `release_tag`.
2. Deja `perform_submit` sin marcar para genear artefactos sin tocar Torii
   (utile pour les courses à sec) ou habilité à publier directement sous l'alias configuré.`docs/source/sorafs_ci_templates.md` dans la documentation des assistants génériques CI pour
les projets autour de ce dépôt, mais le flux de travail du portail doit être l'option préférée
para libère del dia a dia.

## Liste de contrôle

-[ ] `npm run build`, `npm run test:*` et `npm run check:links` sont en vert.
-[ ] `build/checksums.sha256` et `build/release.json` capturés en artefacts.
- [ ] CAR, plan, manifeste, et reprise générées bajo `artifacts/`.
- [ ] Bundle Sigstore + firma separada almacenados con logs.
-[ ] `portal.manifest.submit.summary.json` et `portal.manifest.submit.response.json`
      capturados cuando soumissions de foin.
-[ ] `portal.pin.report.json` (et facultatif `portal.pin.proposal.json`)
      archivé conjointement avec les artefacts CAR/manifeste.
-[ ] Journaux de `proof verify` et `manifest verify-signature` archivés.
-[ ] Tableaux de bord de Grafana actualisés + sondes Try-It exitosos.
- [ ] Notes de restauration (ID du manifeste précédent + résumé de l'alias) ajoutées au
      ticket de sortie.