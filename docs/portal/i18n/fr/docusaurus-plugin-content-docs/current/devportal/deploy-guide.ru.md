---
lang: fr
direction: ltr
source: docs/portal/docs/devportal/deploy-guide.ru.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

## Обзор

Cet achat contient des cartes **DOCS-7** (publication SoraFS) et **DOCS-8** (automation des broches dans CI/CD) dans le cadre de la pratique. procédure pour le portail de démarrage. Dans l'étape suivante build/lint, recherchez SoraFS, fournissez des composants à partir de Sigstore, en fournissant des fournisseurs, des fournisseurs et des fournisseurs. Les séances d'entraînement sont terminées, ce qui correspond à l'aperçu et à la sortie en fonction des utilisateurs et des auditeurs.

Il est prévu que vous soyez un binaire `sorafs_cli` (associé à `--features cli`), en téléchargeant le point de terminaison Torii en utilisant le registre des broches et OIDC est disponible pour Sigstore. Les secrets des logiciels (`IROHA_PRIVATE_KEY`, `SIGSTORE_ID_TOKEN`, tokens Torii) sont stockés dans le coffre-fort CI ; Les opérations locales peuvent être effectuées via l'exportation vers le shell.

## Предварительные условия- Nœud 18.18+ avec `npm` ou `pnpm`.
- `sorafs_cli` à `cargo run -p sorafs_car --features cli --bin sorafs_cli`.
- L'URL Torii, qui ouvre `/v1/sorafs/*`, et votre clé d'accès/privée, vous permet d'ouvrir les manifestes et les alias.
- Émetteur OIDC (GitHub Actions, GitLab, identité de charge de travail et. п.) pour la livraison `SIGSTORE_ID_TOKEN`.
- En option : `examples/sorafs_cli_quickstart.sh` pour l'exécution à sec et `docs/source/sorafs_ci_templates.md` pour les workflows GitHub/GitLab.
- Enregistrez la solution OAuth Essayez-le (`DOCS_OAUTH_*`) et validez
  [Liste de contrôle pour le renforcement de la sécurité](./security-hardening.md) avant la création d'un aperçu du laboratoire précédent. Si le port est fermé, si le démarrage ou les paramètres TTL/pulling vous permettent de choisir ; `DOCS_OAUTH_ALLOW_INSECURE=1` utilise uniquement l'aperçu local. Utilisez le pen-test du document Pen-Test pour vérifier la fiabilité.

## Шаг 0 - Зафиксировать bundle прокси Essayez-le

Avant de procéder à l'aperçu du produit dans Netlify ou dans la passerelle, installez les fichiers Essayez-le proxy et digérez le manifeste OpenAPI dans le bundle de détection :

```bash
cd docs/portal
npm run release:tryit-proxy -- \
  --out ../../artifacts/tryit-proxy/$(date -u +%Y%m%dT%H%M%SZ) \
  --target https://torii.dev.sora \
  --label preview-2026-02-14
````scripts/tryit-proxy-release.mjs` copie les aides proxy/sonde/rollback, en fournissant OpenAPI et `release.json` plus `checksums.sha256`. Achetez ce bundle avec le ticket de passerelle Netlify/SoraFS pour obtenir des informations sur les processus et les transferts vers la cible Torii. пересборки. Bundle est un logiciel que le client de porteur de jetons (`allow_client_auth`) a sélectionné pour planifier le déploiement et l'installation de CSP.

## Partie 1 - Portail de construction et de charpie

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

`npm run build` démarrage automatique `scripts/write-checksums.mjs`, correspondant :

- `build/checksums.sha256` - Manifeste SHA256 pour `sha256sum -c`.
- `build/release.json` - métadonnées (`tag`, `generated_at`, `source`) pour la voiture/manifestation.

Архивируйте оба файла вместе с CAR summary, чтобы ревьюеры могли сравнивать aperçu des articles avant l'achat.

## Partie 2 - Mettre à jour les éléments statiques

Achetez CAR packer dans votre catalogue Docusaurus. L'exemple ci-dessous contient tous vos objets d'art dans `artifacts/devportal/`.

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

Résumé JSON fournit des solutions, des outils et des modules pour la planification de la preuve, qui utilisent les tableaux de bord `manifest build` et CI.

## Partie 2b - Ajouter OpenAPI et les compagnons SBOMDOCS-7 publie sur son portail, l'instantané OpenAPI et les charges utiles SBOM ainsi que les manifestes que les passerelles peuvent créer En-têtes `Sora-Proof`/`Sora-Content-CID` pour chaque article. Aide à la libération (`scripts/sorafs-pin-release.sh`) dans le paquet OpenAPI catalogue (`static/openapi/`) et SBOM de `syft` dans les voitures supplémentaires `openapi.*`/`*-sbom.*` et installez la méta dans `artifacts/sorafs/portal.additional_assets.json`. Dans le cas de l'alimentation domestique, écrivez 2 à 4 fois pour chaque charge utile avec les paramètres et les métadonnées (par exemple `--car-out "$OUT"/openapi.car` et `--metadata alias_label=docs.sora.link/openapi`). Enregistrez votre manifeste/alias dans Torii (par exemple, OpenAPI, SBOM du portail, OpenAPI SBOM) pour sélectionner DNS, votre passerelle plus отдавать les épreuves agrafées pour tous les objets d'art populaires.

## Partie 3 - Soumettre le manifeste

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

Vous pouvez facilement activer les indicateurs pin-policy (par exemple, `--pin-storage-class hot` pour le canal). JSON est une option qui ne peut pas être examinée.

## Partie 4 - Подписать через Sigstore

```bash
sorafs_cli manifest sign \
  --manifest "$OUT"/portal.manifest.to \
  --chunk-plan "$OUT"/portal.plan.json \
  --bundle-out "$OUT"/portal.manifest.bundle.json \
  --signature-out "$OUT"/portal.manifest.sig \
  --identity-token-provider github-actions \
  --identity-token-audience sorafs-devportal
```

L'ensemble de solutions pour le manifeste du jour, les chaînes du jour et BLAKE3 est OIDC, n'est pas JWT. Храните bundle и signature détachée ; Les produits peuvent être utilisés pour les produits d'art les plus récents. Les utilisateurs locaux peuvent sélectionner les indicateurs du fournisseur pour `--identity-token-env` (ou envoyer `SIGSTORE_ID_TOKEN` dans la fenêtre d'accueil), ainsi que l'assistant OIDC. выпускает токен.## Partie 5 - Ouvrir le registre des codes PIN

Ouvrez le manifeste (et le plan de fragmentation) dans Torii. Всегда запрашивайте summary, чтобы запись registre/alias была аудируемой.

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

Lors de l'aperçu du déploiement ou de l'alias Canary (`docs-preview.sora`), vous pouvez ouvrir l'alias unique afin que le contrôle qualité puisse vérifier le contenu avant la promotion de la production.

Liaison d'alias entre trois pôles : `--alias-namespace`, `--alias-name` et `--alias-proof`. La gouvernance fournit un ensemble de preuves (base64 ou Norito octets) après la suppression de l'alias ; Livrez-vous dans CI secrets et envoyez-le avant `manifest submit`. Vous pouvez définir un indicateur d'alias si vous souhaitez supprimer le manifeste sans modifier le DNS.

## Partie 5b - Créer une proposition de gouvernance

Le manifeste de la proposition parlementaire est celui de Sora qui doit prendre des décisions sans privilèges clé. Ensuite, soumettez/signez et indiquez :

```bash
sorafs_cli manifest proposal \
  --manifest "$OUT"/portal.manifest.to \
  --chunk-plan "$OUT"/portal.plan.json \
  --submitted-epoch 20260101 \
  --alias-hint docs.sora.link \
  --proposal-out "$OUT"/portal.pin.proposal.json
```

`portal.pin.proposal.json` corrige les instructions canoniques `RegisterPinManifest`, résume les fonctionnalités, la politique et l'indice d'alias. Prenez-le sur le ticket de gouvernance ou sur le portail du Parlement, les délégués peuvent utiliser la charge utile sans les travailleurs. La commande n'utilise pas le bouton Torii, mais l'utilisateur peut envoyer une proposition localement.

## Partie 6 - Vérifier les preuves et télémétrie

Ensuite, vérifiez les broches suivantes :

```bash
sorafs_cli proof verify \
  --manifest "$OUT"/portal.manifest.to \
  --car "$OUT"/portal.car \
  --summary-out "$OUT"/portal.proof.json

sorafs_cli manifest verify-signature \
  --manifest "$OUT"/portal.manifest.to \
  --bundle "$OUT"/portal.manifest.bundle.json \
  --chunk-plan "$OUT"/portal.plan.json
```- Passez à `torii_sorafs_gateway_refusals_total` et `torii_sorafs_replication_sla_total{outcome="missed"}`.
- Tapez `npm run probe:portal` pour vérifier le proxy Try-It et télécharger les fichiers sur votre contenu.
- Surveiller les preuves
  [Publication et surveillance](./publishing-monitoring.md), pour créer la porte d'observabilité DOCS-3c. Helper prend en charge le `bindings` (OpenAPI, portail SBOM, OpenAPI SBOM) et le travail `Sora-Name`/`Sora-Proof`/`Sora-Content-CID` pour votre hôte `hostname` garde. Vous trouverez ici un ensemble de résumés et de preuves JSON (`portal.json`, `tryit.json`, `binding.json`, `checksums.sha256`) dans le catalogue :

  ```bash
  npm run monitor:publishing -- \
    --config ../../configs/docs_monitor.json \
    --json-out ../../artifacts/sorafs/preview-2026-02-14/monitoring/summary.json \
    --evidence-dir ../../artifacts/sorafs/preview-2026-02-14/monitoring
  ```

## Partie 6a - Planification de la passerelle de certification

Élaborez un plan TLS SAN/challenge pour la gestion des paquets GAR, par exemple la passerelle de commande et les approbateurs DNS travaillant pour vous et pour ces preuves. Un nouvel assistant sélectionne les entrées DG-3, les hôtes génériques canoniques, les SAN jolis hôtes, les étiquettes DNS-01 et les défis ACME recommandés :

```bash
cargo xtask soradns-acme-plan \
  --name docs.sora \
  --json-out artifacts/sorafs/portal.acme-plan.json
```

En engageant JSON dans le bundle de versions (ou en achetant un ticket de modification), les opérateurs peuvent utiliser la configuration SAN dans la configuration `torii.sorafs_gateway.acme`, et les réviseurs GAR sont tous les mêmes. Utilisez des mappages canoniques/jolis sans aucun problème. Ajoutez un autre `--name` pour votre suffixe dans votre cas.

## Partie 6b - Améliorer les mappages d'hôtes canoniquesAvant de créer des charges utiles GAR, veuillez déterminer le mappage d'hôte pour l'alias de votre client. `cargo xtask soradns-hosts` contient `--name` dans l'étiquette canonique (`<base32>.gw.sora.id`), émet un caractère générique différent (`*.gw.sora.id`) et vous permet de créer un joli hôte. (`<alias>.gw.sora.name`). En examinant les artefacts de publication, les réviseurs du DG-3 peuvent également contribuer à la cartographie de la soumission GAR :

```bash
cargo xtask soradns-hosts \
  --name docs.sora \
  --json-out artifacts/sorafs/portal.canonical-hosts.json
```

Utilisez `--verify-host-patterns <file>`, si vous avez un serveur GAR ou une liaison de passerelle JSON, vous ne pouvez pas utiliser d'hôtes. Helper fournit seulement des fichiers, qui peuvent facilement vérifier le modèle GAR et `portal.gateway.binding.json` dans votre commande :

```bash
cargo xtask soradns-hosts \
  --name docs.sora \
  --json-out artifacts/sorafs/portal.canonical-hosts.json \
  --verify-host-patterns artifacts/sorafs/portal.gar.json \
  --verify-host-patterns artifacts/sorafs/portal.gateway.binding.json
```

En utilisant le résumé JSON et le journal de vérification du ticket de changement DNS/passerelle, les auditeurs peuvent modifier les hôtes canoniques/wildcard/jolis sans autorisation. En utilisant la commande pour créer de nouveaux alias, les mises à jour de GAR ont fourni ces preuves.

## Partie 7 - Générer le descripteur de basculement DNS

Les interruptions de production entraînent des modifications au niveau du paquet. Après que l'assistant de soumission (liaison d'alias) soit généré par `artifacts/sorafs/portal.dns-cutover.json`, la solution :- liaison d'alias de métadonnées (espace de noms/nom/preuve, résumé du manifeste, URL Torii, époque soumise, autorité) ;
- contexte de publication (balise, étiquette d'alias, manifeste/CAR, plan de bloc, bundle Sigstore) ;
- pointeurs de vérification (commande de sonde, alias + point final Torii) ; je
- Zone de contrôle des modifications optionnelle (identifiant du ticket, fenêtre de basculement, contact opérationnel, nom d'hôte/zone de production) ;
- promotion de l'itinéraire des métadonnées à partir de l'en-tête `Sora-Route-Binding` (hôte canonique/CID, en-tête + liaison, preuves de commandes), la promotion GAR et les exercices de secours sont basés sur certaines preuves ;
- Des éléments de plan de route générés (`gateway.route_plan.json`, modèles d'en-tête et en-têtes d'annulation facultatifs), des tickets de changement et des crochets à charpie CI peuvent être vérifiés, ce que le DG-3 a mis en place pour la promotion/rollback canonique plans;
- Métadonnées facultatives pour l'invalidation du cache (point de terminaison de purge, variable d'authentification, charge utile JSON et par exemple `curl`) ;
- des conseils de restauration sur le descripteur précédent (balise de version et résumé du manifeste), pour modifier les tickets en cas de repli déterminé.

Si vous lancez une purge du cache, créez un plan canonique avec le descripteur :

```bash
cargo xtask soradns-cache-plan \
  --name docs.sora \
  --path / \
  --path /gateway/manifest.json \
  --auth-header Authorization \
  --auth-env CACHE_PURGE_TOKEN \
  --json-out artifacts/sorafs/portal.cache_plan.json
```En utilisant `portal.cache_plan.json` dans le paquet DG-3, les opérateurs peuvent déterminer les hôtes/chemins d'accès (et les conseils d'authentification suggérés) par `PURGE`. Le descripteur de cache spécial peut être trouvé dans ce fichier, que les réviseurs de contrôle des modifications ont vu, ainsi que les points de terminaison sont détectés lors du basculement.

Le paquet DG-3 contient une promotion + une liste de contrôle de restauration. Créez ici à partir de `cargo xtask soradns-route-plan` les réviseurs de contrôle des modifications qui peuvent utiliser les paramètres de contrôle en amont/basculement/restauration pour l'alias :

```bash
cargo xtask soradns-route-plan \
  --name docs.sora \
  --json-out artifacts/sorafs/gateway.route_plan.json
```

`gateway.route_plan.json` corrige les hôtes canoniques/jolis, les rappels de contrôle de santé par étapes, les mises à jour de liaison GAR, les purges de cache et les actions de restauration. Si vous souhaitez utiliser les objets GAR/binding/cutover en cas de changement de ticket, Ops peut répéter vos opérations.

`scripts/generate-dns-cutover-plan.mjs` forme un descripteur et l'utilise à partir de `sorafs-pin-release.sh`. Pour les générations quotidiennes :

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

Ajoutez des métadonnées opérationnelles lors de l'ouverture temporaire avant d'installer l'assistant PIN :| Переменная | Назначение |
| --- | --- |
| `DNS_CHANGE_TICKET` | ID du ticket, qui correspond au descripteur. |
| `DNS_CUTOVER_WINDOW` | ISO8601 okно cutover (par exemple, `2026-03-21T15:00Z/2026-03-21T15:30Z`). |
| `DNS_HOSTNAME`, `DNS_ZONE` | Nom d'hôte de production + zone faisant autorité. |
| `DNS_OPS_CONTACT` | Alias ​​d’astreinte ou contact d’escalade. |
| `DNS_CACHE_PURGE_ENDPOINT` | Purger le point de terminaison, en remplaçant le descripteur. |
| `DNS_CACHE_PURGE_AUTH_ENV` | Env var avec jeton de purge (par défaut : `CACHE_PURGE_TOKEN`). |
| `DNS_PREVIOUS_PLAN` | Mettez en avant le descripteur de basculement pour les métadonnées de restauration. |

En utilisant JSON et l'examen des modifications DNS, les approbateurs peuvent vérifier le résumé du manifeste, les liaisons d'alias et les commandes de sonde sans promouvoir les logos CI. Indicateurs CLI `--dns-change-ticket`, `--dns-cutover-window`, `--dns-hostname`, `--dns-zone`, `--ops-contact`, `--cache-purge-endpoint`, `--cache-purge-auth-env` et `--previous-dns-plan` vous permet de remplacer l'assistant par CI.

## Partie 8 - Former le squelette du fichier de zone du résolveur (officiellement)Lorsque vous créez une fenêtre de basculement en production, le script de publication peut générer automatiquement le squelette du fichier de zone SNS et l'extrait de code du résolveur. Assurez-vous d'avoir accès aux informations DNS et aux métadonnées de l'environnement ou de la CLI ; helper utilise `scripts/sns_zonefile_skeleton.py` après la génération du descripteur de basculement. Sélectionnez un minimum de données A/AAAA/CNAME et GAR digest (BLAKE3-256 prend en charge la charge utile GAR). Si la zone/nom d'hôte est sélectionnée et que `--dns-zonefile-out` est proposé, l'assistant installe `artifacts/sns/zonefiles/<zone>/<hostname>.json` et utilise `ops/soradns/static_zones.<hostname>.json` comme extrait de code du résolveur.| Переменная / флаг | Назначение |
| --- | --- |
| `DNS_ZONEFILE_OUT`, `--dns-zonefile-out` | Placez le squelette du fichier de zone généré. |
| `DNS_ZONEFILE_RESOLVER_SNIPPET`, `--dns-zonefile-resolver-snippet` | Ajoutez un extrait de résolveur (en utilisant `ops/soradns/static_zones.<hostname>.json`). |
| `DNS_ZONEFILE_TTL`, `--dns-zonefile-ttl` | TTL pour la durée (par défaut : 600 secondes). |
| `DNS_ZONEFILE_IPV4`, `--dns-zonefile-ipv4` | Adresse IPv4 (env séparé par des virgules ou drapeau national). |
| `DNS_ZONEFILE_IPV6`, `--dns-zonefile-ipv6` | Adresse IPv6. |
| `DNS_ZONEFILE_CNAME`, `--dns-zonefile-cname` | Cible CNAME spéciale. |
| `DNS_ZONEFILE_SPKI`, `--dns-zonefile-spki-pin` | Broches SHA-256 SPKI (base64). |
| `DNS_ZONEFILE_TXT`, `--dns-zonefile-txt` | Fichiers TXT supplémentaires (`key=value`). |
| `DNS_ZONEFILE_VERSION`, `--dns-zonefile-version` | Remplacer l'étiquette de version calculée du fichier de zone. |
| `DNS_ZONEFILE_EFFECTIVE_AT`, `--dns-zonefile-effective-at` | Téléchargez l'horodatage `effective_at` (RFC3339) pour ouvrir la fenêtre de basculement. |
| `DNS_ZONEFILE_PROOF`, `--dns-zonefile-proof` | Remplacer la preuve littérale par les métadonnées. |
| `DNS_ZONEFILE_CID`, `--dns-zonefile-cid` | Remplacer les métadonnées du CID. |
| `DNS_ZONEFILE_FREEZE_STATE`, `--dns-zonefile-freeze-state` | État de gel du gardien (doux, dur, décongélation, surveillance, urgence). |
| `DNS_ZONEFILE_FREEZE_TICKET`, `--dns-zonefile-freeze-ticket` | Référence du ticket du tuteur/conseil pour le gel. |
| `DNS_ZONEFILE_FREEZE_EXPIRES_AT`, `--dns-zonefile-freeze-expires-at` | Horodatage RFC3339 pour la décongélation. |
| `DNS_ZONEFILE_FREEZE_NOTES`, `--dns-zonefile-freeze-note` | Notes de gel supplémentaires (env séparés par des virgules ou drapeau initial). || `DNS_GAR_DIGEST`, `--dns-gar-digest` | BLAKE3-256 digest (hex) prend en charge la charge utile GAR. Testez les liaisons de passerelle. |

Workflow GitHub Actions est une zone de stockage de secrets, pour les broches de production automatiques des artefacts de fichier de zone. Enregistrez les secrets suivants (vous pouvez utiliser des listes séparées par des virgules pour les champs à valeurs multiples) :

| Secrets | Назначение |
| --- | --- |
| `DOCS_SORAFS_DNS_HOSTNAME`, `DOCS_SORAFS_DNS_ZONE` | Nom d'hôte/zone de production pour l'assistant. |
| `DOCS_SORAFS_DNS_OPS_CONTACT` | Alias ​​d'astreinte et descripteur. |
| `DOCS_SORAFS_ZONEFILE_IPV4`, `DOCS_SORAFS_ZONEFILE_IPV6` | IPv4/IPv6 pour les publications. |
| `DOCS_SORAFS_ZONEFILE_CNAME` | Cible CNAME spéciale. |
| `DOCS_SORAFS_ZONEFILE_SPKI` | Broches SPKI base64. |
| `DOCS_SORAFS_ZONEFILE_TXT` | Дополнительные TXT записи. |
| `DOCS_SORAFS_ZONEFILE_FREEZE_STATE/TICKET/EXPIRES_AT/NOTES` | Geler les métadonnées du squelette. |
| `DOCS_SORAFS_GAR_DIGEST` | Hex BLAKE3 digest подписанного GAR charge utile. |

Lorsque vous ouvrez `.github/workflows/docs-portal-sorafs-pin.yml`, sélectionnez les entrées `dns_change_ticket` et `dns_cutover_window` pour que le descripteur/fichier de zone ne corresponde pas à la métadonnée correcte. Installez-le uniquement pour le fonctionnement à sec.

Commande type (pour le propriétaire du runbook SN-7) :

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
  ...other flags...
```

Helper permet de modifier automatiquement le ticket dans TXT et d'ouvrir la fenêtre de basculement comme `effective_at`, si ce n'est pas le cas. Полный flux de travail opérationnel см. dans `docs/source/sorafs_gateway_dns_owner_runbook.md`.

### Aperçu de la gestion DNS publiqueLe fichier de zone Skeleton gère seulement les zones autorisées. Délégation NS/DS
Les zones mobiles ne peuvent pas créer un registre ou un fournisseur DNS,
Vous êtes connecté à Internet pour votre serveur de noms.

- Pour le basculement vers apex/TLD, utilisez ALIAS/ANAME (en utilisant le fournisseur) ou
  Publiez les instructions A/AAAA pour accéder à la passerelle anycast-IP.
- Pour que les utilisateurs puissent publier CNAME sur l'hôte Pretty dérivé
  (`<fqdn>.gw.sora.name`).
- L'hôte canonique (`<hash>.gw.sora.id`) est installé dans la passerelle domestique et non
  publié dans votre zone publique.

### Passerelle Шаблон заголовков

L'assistant de déploiement génère `portal.gateway.headers.txt` et `portal.gateway.binding.json` - pour l'art, vous avez ajouté DG-3 à la liaison de contenu de passerelle :

- `portal.gateway.headers.txt` contient les en-têtes HTTP du bloc principal (avec `Sora-Name`, `Sora-Content-CID`, `Sora-Proof`, CSP, HSTS et `Sora-Route-Binding`), cette passerelle Edge должен приклеивать к каждому ответу.
- `portal.gateway.binding.json` vous permet d'obtenir des informations dans la vidéo automatique sur la manière de modifier les tickets et de gérer automatiquement les liaisons hôte/cid sans analyse. coquille вывода.

Il s'agit de générer à partir de `cargo xtask soradns-binding-template` et de définir l'alias, le résumé du manifeste et le nom d'hôte de la passerelle, qui sont basés sur `sorafs-pin-release.sh`. Pour la régénération ou la conversion des en-têtes de bloc, sélectionnez :

```bash
cargo xtask soradns-binding-template \
  --manifest artifacts/sorafs/portal.manifest.json \
  --alias docs.sora \
  --hostname docs.sora.link \
  --route-label production \
  --json-out artifacts/sorafs/portal.gateway.binding.json \
  --headers-out artifacts/sorafs/portal.gateway.headers.txt
```Avant `--csp-template`, `--permissions-template` ou `--hsts-template`, vous devez utiliser les connecteurs ; utilisez les drapeaux `--no-*` pour l'en-tête de connexion.

Ajoutez un extrait de code à une demande de modification CDN et pré-installez JSON dans le pipeline d'automatisation de passerelle, afin de réellement fournir des preuves à votre hébergeur.

Le script de publication fournit automatiquement une aide à la vérification, pour les tickets DG-3 et les preuves. Utilisez-le si vous utilisez la liaison JSON :

```bash
cargo xtask soradns-verify-binding \
  --binding artifacts/sorafs/portal.gateway.binding.json \
  --alias docs.sora.link \
  --hostname docs.sora.link \
  --proof-status ok \
  --manifest-json artifacts/sorafs/portal.manifest.json
```

La commande agrafé `Sora-Proof`, fournit la connexion `Sora-Route-Binding` avec le CID manifeste + le nom d'hôte et se connecte aux en-têtes appropriés. Vous devez archiver tous les artefacts de publication des artefacts publiés par CI, que les réviseurs de la DG-3 ont fourni des preuves.

> **Détail du descripteur DNS :** `portal.dns-cutover.json` vous permet d'accéder à la section `gateway_binding` pour rechercher cet article (par exemple, contenu CID, preuve de statut et modèle d'en-tête littéral) ** et ** section `route_plan`, compatible avec `gateway.route_plan.json` et modèle de restauration/rollback. Vérifiez ces blocs dans le ticket de changement DG-3, afin que les évaluateurs puissent vérifier le numéro `Sora-Name/Sora-Proof/CSP` et modifier les plans de promotion/retrait de l'ensemble de preuves. без открытия construire архива.

## Partie 9 - Installer les moniteurs de publicationL'élément de feuille de route **DOCS-3c** propose des documents sur le portail, Essayez-le, les liaisons de proxy et de passerelle sont disponibles après la publication. Installez le moniteur consolidé après les étapes 7 à 8 et ajoutez-le à vos sondes programmées :

```bash
cd docs/portal
npm run monitor:publishing -- \
  --config ../../configs/docs_monitor.json \
  --json-out ../../artifacts/sorafs/${RELEASE_TAG}/monitoring/summary-$(date -u +%Y%m%dT%H%M%SZ).json \
  --evidence-dir ../../artifacts/sorafs/${RELEASE_TAG}/monitoring
```

- `scripts/monitor-publishing.mjs` ajoute la configuration (par exemple `docs/portal/docs/devportal/publishing-monitoring.md` pour les schémas) et utilise trois tests : sondes sur le portail + validation CSP/Permissions-Policy, essayez-le avec les sondes proxy (en général `/metrics`), et le vérificateur de liaison de passerelle (`cargo xtask soradns-verify-binding`), qui vous permettent de vérifier la disponibilité et de vérifier que Sora-Content-CID est associé à la vérification d'alias/manifeste.
- La commande est différente de zéro, mais la sonde de l'utilisateur est compatible avec les opérateurs CI/cron/runbook qui peuvent utiliser l'alias de production.
- Le drapeau `--json-out` présente un résumé JSON en ce moment ; `--evidence-dir` ci-dessous `summary.json`, `portal.json`, `tryit.json`, `binding.json` et `checksums.sha256`, que les réviseurs de gouvernance peuvent examiner результаты без повторного запуска мониторинга. Enregistrez ce catalogue dans le module `artifacts/sorafs/<tag>/monitoring/` avec le bundle Sigstore et le descripteur de basculement DNS.
- Cliquez sur la surveillance, l'exportation Grafana (`dashboards/grafana/docs_portal.json`) et l'ID d'exercice Alertmanager dans le ticket de version, pour que le SLO DOCS-3c puisse être entendu. Playbook a été publié dans `docs/portal/docs/devportal/publishing-monitoring.md`.Les sondes de portail génèrent HTTPS et ferment `http://` en fonction de l'URL, ou encore `allowInsecureHttp` dans le moniteur de configuration ; Supprimez la production/mise en scène de TLS et activez le remplacement uniquement pour l'aperçu local.

Installez automatiquement le moniteur `npm run monitor:publishing` dans Buildkite/cron après avoir ouvert le portail. Cette commande, qui s'applique aux URL de production, peut effectuer des contrôles de santé postés.

## Automatisation à partir de `sorafs-pin-release.sh`

`docs/portal/scripts/sorafs-pin-release.sh` intègre les étapes 2 à 6. Oui :

1. archiviruет `build/` dans l'archive tar déterminirovannye,
2. Téléchargez `car pack`, `manifest build`, `manifest sign`, `manifest verify-signature` et `proof verify`,
3. Utilisez éventuellement `manifest submit` (avec liaison d'alias) pour les informations d'identification Torii, et
4. Choisissez `artifacts/sorafs/portal.pin.report.json`, `portal.pin.proposal.json` optionnel, descripteur de basculement DNS (après soumission) et ensemble de liaisons de passerelle (`portal.gateway.binding.json` plus bloc d'en-tête), pour les commandes de gouvernance/réseau/opérations que vous avez. сверять preuve без изучения CI логов.

Avant d'utiliser `PIN_ALIAS`, `PIN_ALIAS_NAMESPACE`, `PIN_ALIAS_NAME` et (officiellement) `PIN_ALIAS_PROOF_PATH`. Utilisez `--skip-submit` pour le fonctionnement à sec ; Le workflow GitHub n'est plus disponible avec l'entrée `perform_submit`.

## Partie 8 - Publication des spécifications OpenAPI et des bundles SBOM

DOCS-7 est disponible sur le portail, les spécifications OpenAPI et les objets SBOM fournis pour ce pipeline de détection. Les assistants suivants proposent chacun des trois :1. **Перегенерировать и подписать spec.**

   ```bash
   npm run sync-openapi -- --version=2025-q3 --mirror=current --latest
   cargo xtask openapi --sign docs/portal/static/openapi/manifest.json
   ```

   Sélectionnez `--version=<label>` pour la sauvegarde de l'instantané de l'historique (par exemple `2025-q3`). Helper prend un instantané dans `static/openapi/versions/<label>/torii.json`, localise dans `versions/current` et enregistre les métadonnées (SHA-256, statut du manifeste, horodatage mis à jour) dans `static/openapi/versions.json`. Le portail utilise ces index pour la vérification des versions et la sélection du résumé/signature. Si `--version` n'est pas utilisé, vous devez utiliser les étiquettes correspondantes et utiliser uniquement `current` + `latest`.

   Le correctif manifeste SHA-256/BLAKE3 digests, pour la passerelle, peut utiliser `Sora-Proof` pour `/reference/torii-swagger`.

2. **Formater les SBOM CycloneDX.** Le pipeline de publication permet de supprimer les SBOM basés sur Syft selon `docs/source/sorafs_release_pipeline_plan.md`. Vous pouvez créer des artefacts :

   ```bash
   syft dir:build -o json > "$OUT"/portal.sbom.json
   syft file:docs/portal/static/openapi/torii.json -o json > "$OUT"/openapi.sbom.json
   ```

3. **Упаковать каждый payload в CAR.**

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

   Sélectionnez le modèle `manifest build`/`manifest sign`, qui et pour votre propre compte, indiquez un autre alias à l'art (par exemple, `docs-openapi.sora` pour les spécifications et `docs-sbom.sora` pour le SBOM). Un autre alias fournit SoraDNS, GAR et des tickets de restauration en fonction de la charge utile concrète.

4. **Soumettre et lier.** Sélectionnez l'autorité de votre choix et le bundle Sigstore, et vous ne pouvez pas définir l'alias du tuple dans la liste de contrôle de publication, les auditeurs sont disponibles, ainsi que le nom Sora. какому digérer.L'archivage des manifestes spec/SBOM est assuré par le portail de construction, ce qui signifie que le ticket de sortie est disponible pour les articles d'art sans avoir à utiliser Packer.

### Assistant d'automatisation (script CI/package)

`./ci/package_docs_portal_sorafs.sh` code les étapes 1 à 8, pour l'élément de feuille de route **DOCS-7**, vous pouvez utiliser une autre commande. Aide :

- выполняет подготовку портала (synchronisation `npm ci`, OpenAPI/Norito, tests de widgets) ;
- former des CAR et des paires de manifestes pour le portail OpenAPI et SBOM à partir de `sorafs_cli` ;
- опционально выполняет `sorafs_cli proof verify` (`--proof`) et Sigstore signature (`--sign`, `--sigstore-provider`, `--sigstore-audience`);
- placez tous les articles dans `artifacts/devportal/sorafs/<timestamp>/` et placez `package_summary.json` pour l'outillage CI/release ; je
- Mettre à jour `artifacts/devportal/sorafs/latest` lors de la prochaine mise en service.

Exemple (pour le pipeline avec Sigstore + PoR) :

```bash
./ci/package_docs_portal_sorafs.sh \
  --proof \
  --sign \
  --sigstore-provider=github-actions \
  --sigstore-audience=sorafs-devportal
```

Drapeaux polonais :- `--out <dir>` - remplace les articles de maïs (en incluant les catalogues avec l'horodatage).
- `--skip-build` - использовать существующий `docs/portal/build` (possible pour les miroirs hors ligne).
- `--skip-sync-openapi` - proposez `npm run sync-openapi`, mais `cargo xtask openapi` ne peut pas être téléchargé sur crates.io.
- `--skip-sbom` - ne sélectionnez pas `syft`, si le binaire n'est pas utilisé (le script est indiqué avant la date d'ouverture).
- `--proof` - выполнить `sorafs_cli proof verify` для каждой пары CAR/manifeste. Pour les charges utiles multi-tâches qui nécessitent la prise en charge des plans de blocs dans la CLI, vous pouvez afficher ce drapeau lors de l'installation `plan chunk count` et vérifier вручную после появления porte.
- `--sign` - sélectionnez `sorafs_cli manifest sign`. Sélectionnez le jeton pour `SIGSTORE_ID_TOKEN` (ou `--sigstore-token-env`) en ouvrant la CLI pour votre `--sigstore-provider/--sigstore-audience`.

Pour les articles de production, utilisez `docs/portal/scripts/sorafs-pin-release.sh`. Sur le portail OpenAPI et SBOM, vous pouvez télécharger le manifeste et télécharger les métadonnées supplémentaires dans `portal.additional_assets.json`. Helper vous aide à gérer les boutons du packager CI, ainsi que les nouveaux `--openapi-*`, `--portal-sbom-*` et `--openapi-sbom-*` pour la création de tuples d'alias, remplacer la source SBOM ici `--openapi-sbom-source`, propose des charges utiles externes (`--skip-openapi`/`--skip-sbom`) et n'est pas disponible pour `syft` par `--syft-bin`.Le script contient toutes les commandes ; Enregistrez le journal dans le ticket de sortie avec `package_summary.json`, pour consulter les résumés CAR, les métadonnées du plan et les hachages de bundle Sigstore sans passer par le shell. вывода.

## Partie 9 - Passerelle Province + SoraDNS

Avant la mise en service du basculement, vous devez déterminer quelle nouvelle résolution d'alias est disponible pour SoraDNS et les passerelles en utilisant les preuves suivantes :

1. **Démontez la porte de la sonde.** `ci/check_sorafs_gateway_probe.sh` compare `cargo xtask sorafs-gateway-probe` aux appareils de démonstration dans `fixtures/sorafs_gateway/probe_demo/`. Pour les déploiements réels, sélectionnez le nom d'hôte cible :

   ```bash
   ./ci/check_sorafs_gateway_probe.sh -- \
     --gateway "https://docs.sora/.well-known/sorafs/manifest" \
     --header "Accept: application/json" \
     --gar fixtures/sorafs_gateway/probe_demo/demo.gar.jws \
     --gar-key "demo-gar=$(<fixtures/sorafs_gateway/probe_demo/gar_pub.hex>)" \
     --host "docs.sora" \
     --report-json artifacts/sorafs_gateway_probe/ci/docs.json
   ```

   La sonde détecte `Sora-Name`, `Sora-Proof` et `Sora-Proof-Status` sur `docs/source/sorafs_alias_policy.md` et est également compatible avec le manifeste de digestion, les liaisons TTL ou GAR.

   Pour les contrôles ponctuels légers (par exemple, lorsque seul le lot de liaisons
   modifié), exécutez `cargo xtask soradns-verify-binding --binding <portal.gateway.binding.json> --alias "<alias>" --hostname "<gateway-host>" --proof-status ok --manifest-json <portal.manifest.json>`.
   L'assistant valide le bundle de liaison capturé et est pratique pour la libération
   des billets qui nécessitent uniquement une confirmation contraignante au lieu d’un exercice d’enquête complet.

2. **Obtenez des preuves pour la perceuse.** Pour les perceuses d'opérateur ou les essais à sec PagerDuty, utilisez `scripts/telemetry/run_sorafs_gateway_probe.sh --scenario devportal-rollout -- ...`. Le wrapper prend en charge les en-têtes/journaux dans `artifacts/sorafs_gateway_probe/<stamp>/`, prend en charge `ops/drill-log.md` et utilise éventuellement les hooks de restauration ou les charges utiles PagerDuty. Installez `--host docs.sora` pour vérifier SoraDNS sur une adresse IP différente.3. **Prouvez les liaisons DNS.** Lorsque la gouvernance publie une preuve d'alias, exécutez le fichier GAR trouvé dans la sonde (`--gar`) et enregistrez-la pour publier des preuves. Les propriétaires de résolveurs peuvent programmer celui-ci à partir de `tools/soradns-resolver`, qui est en cours de préparation et qui nécessite une nouvelle manipulation. Avant d'utiliser JSON, vous devez utiliser `cargo xtask soradns-verify-gar --gar <path> --name <alias> [--manifest-cid <cid>] [--telemetry-label <label>]` pour déterminer le mappage d'hôte, le manifeste de métadonnées et les étiquettes de télémétrie en faisant preuve de transparence. Helper peut former un résumé `--json-out` avec l'aide de GAR.
  Pour le nouveau GAR, utilisez `cargo xtask soradns-gar-template --name <alias> --manifest <portal.manifest.json> --telemetry-label <label> ...` (vous devez utiliser `--manifest-cid <cid>` juste avant l'ouverture du manifeste). Helper vous aide à créer le CID ** et ** BLAKE3 digest à partir du manifeste JSON, à tester, à dédoubler la version `--telemetry-label`, à trier les étiquettes et à utiliser par défaut Les modèles CSP/HSTS/Permissions-Policy avant la génération de JSON, qui permettent de déterminer la charge utile.

4. **Sélectionnez l'alias des mesures.** Cliquez sur l'écran `torii_sorafs_alias_cache_refresh_duration_ms` et `torii_sorafs_gateway_refusals_total{profile="docs"}` ; La série est `dashboards/grafana/docs_portal.json`.

## Partie 10 - Surveillance et regroupement de preuves- **Tableaux de bord.** Exportez `dashboards/grafana/docs_portal.json` (port SLO), `dashboards/grafana/sorafs_gateway_observability.json` (passerelle de latence + preuve d'intégrité) et `dashboards/grafana/sorafs_fetch_observability.json` (intégrité de l'orchestrateur) pour chaque réponse. Achetez le ticket d'exportation JSON pour la sortie.
- **Probe archives.** Enregistrez `artifacts/sorafs_gateway_probe/<stamp>/` dans git-annex ou dans le compartiment de preuves. Consultez le résumé de la sonde, les en-têtes et la charge utile PagerDuty.
- **Release bundle.** Consultez le portail de résumé CAR/SBOM/OpenAPI, les bundles de manifestes, les signatures Sigstore, `portal.pin.report.json`, les journaux de sonde Try-It et les rapports de vérification des liens dans un autre document (par exemple, `artifacts/sorafs/devportal/20260212T1103Z/`).
- **Journal de forage.** Sondes de recherche - forage en cours, `scripts/telemetry/run_sorafs_gateway_probe.sh` est ajouté à `ops/drill-log.md`, ce qui correspond à l'exigence de chaos SNNet-5.
- **Liens de ticket.** Recherchez les ID de panneau Grafana ou utilisez les exportations PNG dans le ticket de modification lors du rapport de sonde, afin que les rapports puissent gérer les SLO sans le shell.

## Partie 11 - Exercice de récupération multi-sources et preuves du tableau de bord

La publication dans SoraFS permet de récupérer des preuves multi-sources (DOCS-7/SF-6) avec des preuves DNS/passerelle. Ensuite, le manifeste de la broche :

1. **Utilisez `sorafs_fetch` sur le manifeste en direct.** Utilisez les éléments de plan/manifeste des étapes 2-3 et les informations d'identification de la passerelle pour chaque fournisseur. Prenez soin de toutes vos données, les auditeurs peuvent utiliser l'orchestrateur de résolution :

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
   ```- Vous pouvez afficher les annonces des fournisseurs en les plaçant dans le manifeste (par exemple, `sorafs_cli manifest describe --provider-adverts-out artifacts/sorafs/provider_adverts/`) et en prévoyant votre `--provider-advert name=path` pour afficher le tableau de bord. возможностей детерминированно. `--allow-implicit-provider-metadata` используйте **только** для CI фикстур; Les forets de production doivent être téléchargés sur les publicités des broches.
   - Lorsque vous activez deux régions, vous pouvez utiliser la commande sur les tuples du fournisseur correspondant, qui peuvent récupérer le cache/alias et récupérer l'article correspondant.

2. **Архивировать выходы.** Enregistrez `scoreboard.json`, `providers.ndjson`, `fetch.json` et `chunk_receipts.ndjson` dans le catalogue de preuves publié. Nous allons maintenant corriger la pondération des pairs, le budget de nouvelle tentative, la latence EWMA et les reçus de fragments pour SF-7.

3. **Activer la télémétrie.** Importer les sorties de récupération dans le tableau de bord **SoraFS Récupérer l'observabilité** (`dashboards/grafana/sorafs_fetch_observability.json`), sélectionnez `torii_sorafs_fetch_duration_ms`/`_failures_total` et les panneaux. по gammes de fournisseurs. Прикладывайте Grafana snapshots к release ticket вместе с путем к scoreboard.

4. **Prouver les règles d'alerte.** Cliquez sur `scripts/telemetry/test_sorafs_fetch_alerts.sh` pour vérifier le paquet d'alertes Prometheus avant de lancer la réponse. En utilisant la sortie de Promtool, les réviseurs de DOCS-7 ont été informés de l'activité des alertes de décrochage/fournisseur lent.5. **Подключить в CI.** Le flux de travail des broches du portail correspond à `sorafs_fetch` pour l'entrée `perform_fetch_probe` ; Par exemple, pour la mise en scène/production, vous devez récupérer les preuves formées dans le lot de manifestes. Les exercices locaux peuvent utiliser ce script, exporter des jetons de passerelle et le code `PIN_FETCH_PROVIDERS` (fournisseurs de certificats séparés par des virgules).

## Promotion, observabilité et restauration

1. **Promotion :** proposez un alias de mise en scène et de production. Le fournisseur qui a initialement saisi `manifest submit` pour le manifeste/bundle correspond à `--alias-namespace/--alias-name` pour l'alias de production. Cela inclut le travail/le service après-vente après l'approbation de l'assurance qualité.
2. **Surveillance :** ajoutez le registre des broches du tableau de bord (`docs/source/grafana_sorafs_pin_registry.json`) et les sondes de portail (`docs/portal/docs/devportal/observability.md`). Vérifiez la dérive de la somme de contrôle, les sondes défaillantes ou la preuve de nouvelle tentative de pointes.
3. **Rollback :** pour obtenir le manifeste précédent (ou retirer votre alias) avec `sorafs_cli manifest submit --alias ... --retire`. Lors du dernier bundle de marchandises connues et du résumé CAR, les preuves de restauration peuvent être utilisées pour la rotation des journaux CI.

## Workflow CI de Shablon

Le pipeline minimal est terminé :1. Build + Lint (`npm ci`, `npm run build`, sommes de contrôle de génération).
2. Упаковка (`car pack`) et вычисление manifeste.
3. Ajoutez le jeton OIDC à portée de travail (`manifest sign`).
4. Загрузка артефактов (CAR, manifeste, bundle, plan, résumés) для аудита.
5. Ouvrir le registre des broches :
   - Demandes d'extraction -> `docs-preview.sora`.
   - Tags / branches protégées -> alias de production de promotion.
6. Sondes de vérification + portes de vérification de preuve avant la vérification.

`.github/workflows/docs-portal-sorafs-pin.yml` est destiné aux versions manuelles. Flux de travail :

- construire/tester le portail,
- упаковка build через `scripts/sorafs-pin-release.sh`,
- ajouter/ajouter un bundle de manifestes à partir de GitHub OIDC,
- загрузка CAR/manifest/bundle/plan/preuve résumés как artefacts, и
- (опционально) отправка manifeste + liaison d'alias pour les secrets.

Enregistrez les secrets/variables du référentiel avant de terminer le travail :

| Nom | Назначение |
| --- | --- |
| `DOCS_SORAFS_TORII_URL` | Hôte Torii et `/v1/sorafs/pin/register`. |
| `DOCS_SORAFS_SUBMITTED_EPOCH` | Identifiant d'époque, записанный при soumission. |
| `DOCS_SORAFS_AUTHORITY` / `DOCS_SORAFS_PRIVATE_KEY` | Подписывающая autorité для soumission манифеста. |
| `DOCS_SORAFS_ALIAS_NAMESPACE` / `DOCS_SORAFS_ALIAS_NAME` | Tuple d'alias, associé à `perform_submit` = `true`. |
| `DOCS_SORAFS_ALIAS_PROOF_B64` | Bundle de preuve d'alias Base64 (officiellement). |
| `DOCS_ANALYTICS_*` | Points finaux d'analyse/sonde pertinents. |

Sélectionnez le workflow dans l'interface utilisateur d'Actions :1. Sélectionnez `alias_label` (par exemple, `docs.sora.link`), en option `proposal_alias` et en option `release_tag`.
2. Installez `perform_submit` pour la génération d'objets d'art autres que Torii (fonctionnement à sec) ou activez la publication sur la base de données. pseudonyme.

`docs/source/sorafs_ci_templates.md` propose des assistants CI pour de nombreux projets, mais également pour les opérations ultérieures en utilisant le flux de travail du portail.

## Checklist

-[ ] `npm run build`, `npm run test:*` et `npm run check:links`.
-[ ] `build/checksums.sha256` et `build/release.json` sont associés aux artefacts.
- [ ] CAR, plan, manifeste et résumé сгенерированы под `artifacts/`.
-[ ] Bundle Sigstore + signature détachée сохранены с логами.
-[ ] `portal.manifest.submit.summary.json` et `portal.manifest.submit.response.json` sont pris en compte lors de la soumission.
- [ ] `portal.pin.report.json` (et опциональный `portal.pin.proposal.json`) archивированы рядом с CAR/manifeste artefacts.
- [ ] Les journaux `proof verify` et `manifest verify-signature` sont affichés.
-[ ] Grafana tableaux de bord обновлены + Try-It sondes успешны.
- [ ] Notes de restauration (ID du manifeste précédent + résumé de l'alias) associées au ticket de sortie.