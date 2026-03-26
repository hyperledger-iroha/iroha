---
lang: fr
direction: ltr
source: docs/portal/docs/devportal/deploy-guide.ur.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

## جائزہ

Il s'agit de **DOCS-7** (SoraFS اشاعت) et **DOCS-8**
(CI/CD en anglais) کو ڈیولپر پورٹل کے لئے قابلِ عمل طریقہ کار میں بدلتی ہے۔
یہ build/lint مرحلہ، SoraFS پیکجنگ، Sigstore کے ذریعے مینی فیسٹ سائننگ،
alias پروموشن، توثیق، اور rollback ڈرلز کو کور کرتی ہے تاکہ ہر aperçu اور
release آرٹیفیکٹ قابلِ اعادہ اور قابلِ آڈٹ ہو۔

یہ فلو فرض کرتا ہے کہ آپ کے پاس `sorafs_cli` بائنری (`--features cli` کے ساتھ
build شدہ) ہے، pin-registry اجازتوں والے Torii endpoint تک رسائی ہے، اور
Sigstore pour OIDC pour OIDC pour Sigstore طویل مدتی راز (`IROHA_PRIVATE_KEY`,
`SIGSTORE_ID_TOKEN`, Torii ٹوکنز) et CI والٹ میں رکھیں؛ لوکل رنز انہیں
exportations de coquilles

## پیشگی شرائط- Nœud 18.18+ pour `npm` ou `pnpm`.
- `sorafs_cli` et `cargo run -p sorafs_car --features cli --bin sorafs_cli` ou `cargo run -p sorafs_car --features cli --bin sorafs_cli`.
- Torii URL et `/v1/sorafs/*` ظاہر کرے اور ایک اتھارٹی اکاؤنٹ/پرائیویٹ کی جو
  مینی فیسٹس اور alias جمع کر سکے۔
- Émetteur OIDC (GitHub Actions, GitLab, Workload Identity et autres)
  `SIGSTORE_ID_TOKEN` منٹ کیا جا سکے۔
- Fonctionnement : essais à sec depuis `examples/sorafs_cli_quickstart.sh` sur GitHub/GitLab
  flux de travail کے لئے `docs/source/sorafs_ci_templates.md`.
- Essayez-le OAuth ویری ایبلز (`DOCS_OAUTH_*`) pour créer un build pour votre ordinateur
  پروموٹ کرنے سے پہلے [liste de contrôle pour le renforcement de la sécurité](./security-hardening.md)
  چلائیں۔ Il y a plusieurs boutons de sondage TTL/polling disponibles
  حدود سے باہر ہوں تو پورٹل build ناکام ہو جاتا ہے؛ `DOCS_OAUTH_ALLOW_INSECURE=1`
  Aperçu des aperçus et exportation des fichiers pen-test ثبوت ریلیز ٹکٹ کے ساتھ
  منسلک کریں۔

## مرحلہ 0 — Essayez-le پروکسی بنڈل محفوظ کریں

Netlify gateway پر preview پروموٹ کرنے سے پہلے Essayez-le maintenant
Il s'agit du résumé OpenAPI de Digest, qui est en cours de lecture :

```bash
cd docs/portal
npm run release:tryit-proxy -- \
  --out ../../artifacts/tryit-proxy/$(date -u +%Y%m%dT%H%M%SZ) \
  --target https://torii.dev.sora \
  --label preview-2026-02-14
````scripts/tryit-proxy-release.mjs` Aides à la vérification/probe/rollback
Signature OpenAPI pour la signature `release.json`
`checksums.sha256` ہے۔ Comment utiliser la passerelle Netlify/SoraFS
ساتھ منسلک کریں تاکہ ریویورز عین وہی پروکسی سورسز اور Torii ٹارگٹ ہنٹس بغیر
دوبارہ build کے ری پلے کر سکیں۔ Les frais de livraison sont fournis par le client
Bearers تھے (`allow_client_auth`) Déploiement de تاکہ pour CSP قواعد آہنگ
رہیں۔

## مرحلہ 1 — پورٹل build اور lint

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

`npm run build` خودکار طور پر `scripts/write-checksums.mjs` چلاتا ہے، اور
یہ تیار کرتا ہے:

- `build/checksums.sha256` — `sha256sum -c` pour le modèle SHA256
- `build/release.json` — Modèles (`tag`, `generated_at`, `source`) et
  CAR/manifeste میں پِن کیا جاتا ہے۔

دونوں فائلیں CAR سمری کے ساتھ آرکائیو کریں تاکہ ریویورز بغیر دوبارہ build کیے
aperçu آرٹیفیکٹس کا فرق دیکھ سکیں۔

## مرحلہ 2 — اسٹیٹک اثاثوں کی پیکجنگ

CAR پیکر کو Docusaurus آؤٹ پٹ ڈائریکٹری پر چلائیں۔ نیچے کی مثال تمام آرٹیفیکٹس
`artifacts/devportal/` est une solution pour vous

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

Le nombre de fragments JSON, les résumés et les conseils de planification de la preuve sont également disponibles.
`manifest build` اور CI ڈیش بورڈز بعد میں دوبارہ استعمال کرتے ہیں۔

## مرحلہ 2b — OpenAPI pour SBOM معاون پیکج کریںDOCS-7 prend en charge les charges utiles OpenAPI pour les charges utiles SBOM
Il existe plusieurs passerelles pour les passerelles et les passerelles.
`Sora-Proof`/`Sora-Content-CID` ہیڈرز اسٹپل کر سکیں۔ ریلیز ہیلپر
(`scripts/sorafs-pin-release.sh`) et OpenAPI (`static/openapi/`)
Pour `syft` et SBOMs pour `openapi.*`/`*-sbom.*` CARs میں پیک
کرتا ہے اور میٹا ڈیٹا `artifacts/sorafs/portal.additional_assets.json` میں
ریکارڈ کرتا ہے۔ Il y a une charge utile et une charge utile 2-4 fois
اس کے اپنے préfixes اور étiquettes de métadonnées استعمال کریں (مثال کے طور پر
`--car-out "$OUT"/openapi.car` et `--metadata alias_label=docs.sora.link/openapi`).
DNS سوئچ کرنے سے پہلے ہر manifeste/alias جوڑے کو Torii میں رجسٹر کریں (سائٹ،
OpenAPI, پورٹل SBOM, OpenAPI SBOM) تاکہ gateway تمام شائع شدہ آرٹیفیکٹس کے لئے
épreuves agrafées فراہم کر سکے۔

## مرحلہ 3 — مینی فیسٹ بنائیں

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

pin-policy فلیگز کو اپنے ریلیز ونڈو کے مطابق ٹیون کریں (مثال کے طور پر
canaris کے لئے `--pin-storage-class hot`)۔ Révision du code JSON et révision du code
کے لئے سہولت دیتا ہے۔

## مرحلہ 4 — Sigstore est en cours de réalisation

```bash
sorafs_cli manifest sign \
  --manifest "$OUT"/portal.manifest.to \
  --chunk-plan "$OUT"/portal.plan.json \
  --bundle-out "$OUT"/portal.manifest.bundle.json \
  --signature-out "$OUT"/portal.manifest.sig \
  --identity-token-provider github-actions \
  --identity-token-audience sorafs-devportal
```bundle contient des résumés de fragments, des résumés de morceaux et OIDC pour le hachage BLAKE3
کرتا ہے بغیر JWT محفوظ کیے۔ bundle اور signature détachée دونوں سنبھال کر
رکھیں؛ promotions de production
کر سکتی ہیں۔ Le fournisseur d'accès Internet est en ligne avec `--identity-token-env` pour le client
(`SIGSTORE_ID_TOKEN` est un appareil photo) pour OIDC
ہیلپر ٹوکن جاری کرے۔

## مرحلہ 5 — pin رجسٹری میں جمع کرائیں

Plan de fragmentation Torii pour le plan de tranches ہمیشہ résumé طلب کریں
تاکہ رجسٹری preuve d'entrée/alias قابلِ آڈٹ رہے۔

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

Aperçu de l'alias Canary (`docs-preview.sora`) et de l'alias Canary (`docs-preview.sora`)
ساتھ soumission دوبارہ کریں تاکہ QA production پروموشن سے پہلے مواد کی تصدیق کر سکے۔

La liaison d'alias est la suivante: `--alias-namespace`, `--alias-name`,
Par `--alias-proof`۔ Un alias درخواست منظور ہونے پر proof bundle (base64 یا
Norito octets) اسے CI secrets میں رکھیں اور `manifest submit`
چلانے سے پہلے اسے فائل کی صورت میں پیش کریں۔ جب آپ صرف مینی فیسٹ pin کرنا
Votre DNS est votre nom d'utilisateur et votre alias est votre nom d'utilisateur.

## مرحلہ 5b — گورننس پروپوزل تیار کریں

ہر مینی فیسٹ کے ساتھ پارلیمنٹ کے لئے تیار پروپوزل ہونا چاہیے تاکہ کوئی بھی
Sora شہری خصوصی اسناد ادھار لئے بغیر تبدیلی پیش کر سکے۔ soumettre/signer مراحل کے
بعد چلائیں:

```bash
sorafs_cli manifest proposal \
  --manifest "$OUT"/portal.manifest.to \
  --chunk-plan "$OUT"/portal.plan.json \
  --submitted-epoch 20260101 \
  --alias-hint docs.sora.link \
  --proposal-out "$OUT"/portal.pin.proposal.json
````portal.pin.proposal.json` est un résumé de fragments `RegisterPinManifest`.
پالیسی، اور indice d'alias کو محفوظ کرتا ہے۔ اسے گورننس ٹکٹ یا پارلیمنٹ پورٹل کے
Il s'agit d'un projet de construction de charge utile pour la charge utile
دیکھ سکیں۔ Clé d'autorité Torii pour clé d'autorité Torii pour clé d'autorité
شہری لوکل طور پر پروپوزل ڈرافٹ کر سکتا ہے۔

## مرحلہ 6 — preuves اور ٹیلیمیٹری کی تصدیق

l'épinglage est une vérification déterministe par exemple :

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

- `torii_sorafs_gateway_refusals_total` ici
  `torii_sorafs_replication_sla_total{outcome="missed"}` anomalies de fonctionnement
- `npm run probe:portal` Essayez-le et essayez-le.
  نئے pin شدہ مواد کے خلاف پرکھا جا سکے۔
- [Publication et surveillance](./publishing-monitoring.md) میں بیان کردہ مانیٹرنگ
  L'observabilité de DOCS-3c est basée sur la recherche en matière d'observabilité.
  پورا ہو۔ ہیلپر اب متعدد `bindings` entrées (سائٹ، OpenAPI, پورٹل SBOM, OpenAPI
  SBOM) قبول کرتا ہے اور اختیاری `hostname` گارڈ کے ذریعے ہدف hôte پر
  `Sora-Name`/`Sora-Proof`/`Sora-Content-CID` نافذ کرتا ہے۔ invocation de نیچے والی
  Il s'agit d'un ensemble de preuves JSON (`portal.json`, `tryit.json`, `binding.json`,
  اور `checksums.sha256`) دونوں ریلیز ڈائریکٹری میں لکھتی ہے:

  ```bash
  npm run monitor:publishing -- \
    --config ../../configs/docs_monitor.json \
    --json-out ../../artifacts/sorafs/preview-2026-02-14/monitoring/summary.json \
    --evidence-dir ../../artifacts/sorafs/preview-2026-02-14/monitoring
  ```

## مرحلہ 6a — gateway سرٹیفکیٹس کی منصوبہ بندیTLS SAN/challenge pour GAR est un système de passerelle et de DNS
منظور کنندگان ایک ہی شواہد دیکھیں۔ Le DG-3 est en train de faire un miroir
Hôtes génériques canoniques, SAN de jolis hôtes, DNS-01 pour les utilisateurs
L'ACME défie les gens suivants :

```bash
cargo xtask soradns-acme-plan \
  --name docs.sora \
  --json-out artifacts/sorafs/portal.acme-plan.json
```

JSON est un outil de création de fichiers JSON (il s'agit d'un code source JSON)
Torii et `torii.sorafs_gateway.acme` pour SAN et `torii.sorafs_gateway.acme` pour SAN
GAR propose des mappages canoniques/jolis et des dérivations d'hôtes.
تصدیق کر سکیں۔ اسی ریلیز میں پروموٹ ہونے والے ہر suffixe کے لئے اضافی `--name`
دلائل شامل کریں۔

## مرحلہ 6b — mappages d'hôtes canoniques ici

Charges utiles GAR pour le mappage déterministe de l'hôte
کریں۔ `cargo xtask soradns-hosts` et `--name` pour la version canonique
(`<base32>.gw.sora.id`) Il s'agit d'un joker générique (`*.gw.sora.id`)
ہے، اور jolie hôte (`<alias>.gw.sora.name`) اخذ کرتا ہے۔ ریلیز آرٹیفیکٹس میں
Il s'agit d'une demande de la DG-3 de la soumission du GAR pour la cartographie et la cartographie
سکیں:

```bash
cargo xtask soradns-hosts \
  --name docs.sora \
  --json-out artifacts/sorafs/portal.canonical-hosts.json
```

Il s'agit d'une liaison GAR et d'une passerelle de liaison JSON pour les hôtes qui sont en train de créer des liens
ناکامی کے لئے `--verify-host-patterns <file>` استعمال کریں۔ ہیلپر متعدد
vérification فائلیں قبول کرتا ہے، جس سے ایک ہی invocation میں GAR template اور
agrafé `portal.gateway.binding.json` pour les peluches et les peluches:

```bash
cargo xtask soradns-hosts \
  --name docs.sora \
  --json-out artifacts/sorafs/portal.canonical-hosts.json \
  --verify-host-patterns artifacts/sorafs/portal.gar.json \
  --verify-host-patterns artifacts/sorafs/portal.gateway.binding.json
```DNS/passerelle Résumé des détails de la vérification JSON et vérification de la configuration de la passerelle
Il s'agit d'un caractère canonique, d'un caractère générique et de jolis hôtes, ainsi que de dérivations d'hôtes.
چلائے بغیر کر سکیں۔ Il existe des alias pour tous les noms d'alias.
چلائیں تاکہ بعد کے GAR اپڈیٹس میں وہی preuve trail برقرار رہے۔

## مرحلہ 7 — Descripteur de basculement DNS en cours

coupures de production کامیاب
soumission (alias contraignant) کے بعد ہیلپر `artifacts/sorafs/portal.dns-cutover.json`
بناتا ہے، جس میں یہ شامل ہے:- Métadonnées de liaison d'alias (espace de noms/nom/preuve, résumé du manifeste, URL Torii,
  époque soumise, autorité) ;
- ریلیز سیاق و سباق (tag, alias label, manifest/CAR paths, chunk plan, Sigstore bundle) ؛
- pointeurs de vérification (alias de la sonde + point de terminaison Torii)
- اختیاری change-control فیلڈز (identifiant du ticket, fenêtre de basculement, contact opérationnel,
  nom d'hôte/zone de production)؛
- Métadonnées de promotion d'itinéraire `Sora-Route-Binding` agrafées
  (Hôte canonique/CID, en-tête + chemins de liaison, commandes de vérification) et GAR
  promotion et exercices de repli
- plan de route généré آرٹیفیکٹس (`gateway.route_plan.json`, modèles d'en-tête,
  (voir les en-têtes de restauration) et les tickets de changement ainsi que les crochets à charpie CI et DG-3
  Il s'agit d'une promotion/annulation canonique.
- Métadonnées d'invalidation du cache (purge du point de terminaison, variable d'authentification, charge utile JSON)
  اور مثال `curl` کمانڈ)؛ اور
- astuces de restauration et descripteur (balise de version et résumé du manifeste)
  اشارہ کریں تاکہ changer les tickets et chemin de repli déterministe

Il s'agit d'une purge de cache et d'un descripteur de basculement et d'un canonique
پلان بنائیں:

```bash
cargo xtask soradns-cache-plan \
  --name docs.sora \
  --path / \
  --path /gateway/manifest.json \
  --auth-header Authorization \
  --auth-env CACHE_PURGE_TOKEN \
  --json-out artifacts/sorafs/portal.cache_plan.json
```Le formulaire `portal.cache_plan.json` du DG-3 est disponible en ligne.
Il y a des hôtes/chemins déterministes (astuces d'authentification supplémentaires) et `PURGE`
demandes بھیجیں۔ descripteur pour les métadonnées du cache
Il s'agit d'un système de contrôle des modifications pour les points de terminaison.
رہیں جو cutover کے دوران flush کیے جاتے ہیں۔

Liste de contrôle de promotion + rollback de la DG-3 pour la promotion et le retour en arrière اسے
`cargo xtask soradns-route-plan` Support de commande de changement de commande
Un alias pour le contrôle en amont, le basculement et la restauration sont les suivants :

```bash
cargo xtask soradns-route-plan \
  --name docs.sora \
  --json-out artifacts/sorafs/gateway.route_plan.json
```

`gateway.route_plan.json` pour les hôtes canoniques/jolis, contrôle de santé par étapes
La liaison GAR et les purges de cache, ainsi que les actions de restauration, sont également disponibles.
GAR/binding/cutover Opérations en ligne avec script
قدموں کی مشق اور منظوری دے سکیں۔

`scripts/generate-dns-cutover-plan.mjs` descripteur pour votre demande
`sorafs-pin-release.sh` خودکار طور پر چلتا ہے۔ دستی طور پر دوبارہ بنانے
یا تبدیل کرنے کے لئے:

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

pin helper et métadonnées facultatives et variables d'environnement
Par ھریں:| Variables | Objectif |
|----------|---------|
| `DNS_CHANGE_TICKET` | descripteur میں محفوظ ہونے والا Ticket ID۔ |
| `DNS_CUTOVER_WINDOW` | Fenêtre de basculement ISO8601 (pour `2026-03-21T15:00Z/2026-03-21T15:30Z`). |
| `DNS_HOSTNAME`, `DNS_ZONE` | nom d'hôte de production + zone faisant autorité۔ |
| `DNS_OPS_CONTACT` | alias d'astreinte یا contact d'escalade۔ |
| `DNS_CACHE_PURGE_ENDPOINT` | descripteur de point de terminaison de purge du cache |
| `DNS_CACHE_PURGE_AUTH_ENV` | jeton de purge est et env var (par défaut : `CACHE_PURGE_TOKEN`). |
| `DNS_PREVIOUS_PLAN` | métadonnées de restauration comme descripteur de basculement ou chemin |

Examen des modifications DNS et résumés de manifeste des approbateurs, alias JSON
liaisons, puis sonde, et CI, pour vérifier, et vérifier Indicateurs CLI
`--dns-change-ticket`, `--dns-cutover-window`, `--dns-hostname`,
`--dns-zone`, `--ops-contact`, `--cache-purge-endpoint`,
`--cache-purge-auth-env`, et `--previous-dns-plan` et remplacent les règles de sécurité
جب helper کو CI کے باہر چلایا جائے۔

## مرحلہ 8 — squelette de fichier de zone du résolveur (اختیاری)Fenêtre de basculement de la production Aperçu du squelette du fichier de zone SNS ici
extrait de code du résolveur مطلوبہ Enregistrements DNS et métadonnées
variables d'environnement et options CLI pour les variables d'environnement descripteur de basculement d'assistance
Il s'agit d'un produit `scripts/sns_zonefile_skeleton.py`. کم از کم ایک
A/AAAA/CNAME et GAR digest (il y a une charge utile GAR par BLAKE3-256) ici
کریں۔ La zone/le nom d'hôte correspond à `--dns-zonefile-out` et correspond à votre nom d'hôte.
helper `artifacts/sns/zonefiles/<zone>/<hostname>.json` pour plus de détails
`ops/soradns/static_zones.<hostname>.json` et un extrait de code du résolveur sont disponibles.| Variable/indicateur | Objectif |
|-----------------|---------|
| `DNS_ZONEFILE_OUT`, `--dns-zonefile-out` | Voici le squelette du fichier de zone et le chemin d'accès |
| `DNS_ZONEFILE_RESOLVER_SNIPPET`, `--dns-zonefile-resolver-snippet` | chemin de l'extrait de résolveur (par défaut `ops/soradns/static_zones.<hostname>.json`) |
| `DNS_ZONEFILE_TTL`, `--dns-zonefile-ttl` | La durée de vie du temps est TTL (par défaut : 600 secondes) |
| `DNS_ZONEFILE_IPV4`, `--dns-zonefile-ipv4` | Adresses IPv4 (env séparés par des virgules et indicateur CLI répétable)۔ |
| `DNS_ZONEFILE_IPV6`, `--dns-zonefile-ipv6` | Adresses IPv6۔ |
| `DNS_ZONEFILE_CNAME`, `--dns-zonefile-cname` | Utiliser la cible CNAME۔ |
| `DNS_ZONEFILE_SPKI`, `--dns-zonefile-spki-pin` | Broches SHA-256 SPKI (base64)۔ |
| `DNS_ZONEFILE_TXT`, `--dns-zonefile-txt` | اضافی Entrées TXT (`key=value`)۔ |
| `DNS_ZONEFILE_VERSION`, `--dns-zonefile-version` | version calculée du fichier de zone et remplacement du fichier de zone |
| `DNS_ZONEFILE_EFFECTIVE_AT`, `--dns-zonefile-effective-at` | démarrage de la fenêtre de basculement avec horodatage `effective_at` (RFC3339) et horodatage forcé |
| `DNS_ZONEFILE_PROOF`, `--dns-zonefile-proof` | métadonnées ریکارڈ preuve remplacement littéral کریں۔ |
| `DNS_ZONEFILE_CID`, `--dns-zonefile-cid` | métadonnées en cours de remplacement CID override en cours |
| `DNS_ZONEFILE_FREEZE_STATE`, `--dns-zonefile-freeze-state` | État de gel du gardien (doux, dur, décongélation, surveillance, urgence)۔ |
| `DNS_ZONEFILE_FREEZE_TICKET`, `--dns-zonefile-freeze-ticket` | gèle کے لئے Référence du ticket du gardien/conseil ۔ |
| `DNS_ZONEFILE_FREEZE_EXPIRES_AT`, `--dns-zonefile-freeze-expires-at` | décongélation selon l'horodatage RFC3339 || `DNS_ZONEFILE_FREEZE_NOTES`, `--dns-zonefile-freeze-note` | Geler les notes (env séparés par des virgules et drapeau répétable) |
| `DNS_GAR_DIGEST`, `--dns-gar-digest` | charge utile GAR signée کا BLAKE3-256 digest (hex)۔ liaisons de passerelle ہونے پر لازم۔ |

Flux de travail GitHub Actions et secrets du référentiel en cours de réalisation
broche de production خودکار طور پر zonefile آرٹیفیکٹس بنائے۔ درج ذیل secrets/valeurs
Chaînes de caractères (chaînes de caractères à valeurs multiples séparées par des virgules) :

| Secrets | Objectif |
|--------|---------|
| `DOCS_SORAFS_DNS_HOSTNAME`, `DOCS_SORAFS_DNS_ZONE` | helper کو دیا جانے والا production hostname/zone۔ |
| `DOCS_SORAFS_DNS_OPS_CONTACT` | descripteur میں محفوظ alias de garde۔ |
| `DOCS_SORAFS_ZONEFILE_IPV4`, `DOCS_SORAFS_ZONEFILE_IPV6` | شائع ہونے والے Enregistrements IPv4/IPv6۔ |
| `DOCS_SORAFS_ZONEFILE_CNAME` | Utiliser la cible CNAME۔ |
| `DOCS_SORAFS_ZONEFILE_SPKI` | Broches SPKI base64۔ |
| `DOCS_SORAFS_ZONEFILE_TXT` | اضافی Entrées TXT۔ |
| `DOCS_SORAFS_ZONEFILE_FREEZE_STATE/TICKET/EXPIRES_AT/NOTES` | squelette میں محفوظ geler les métadonnées۔ |
| `DOCS_SORAFS_GAR_DIGEST` | charge utile GAR signée et résumé BLAKE3 codé en hexadécimal |

`.github/workflows/docs-portal-sorafs-pin.yml` et `dns_change_ticket`
`dns_cutover_window` entrées فراہم کریں تاکہ descriptor/zonefile درست fenêtre de modification
métadonnées لے۔ انہیں خالی صرف courses à sec کے لئے چھوڑیں۔

Invocation (runbook du propriétaire SN-7 ici) :

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
```helper خودکار طور پر changer de ticket کو Entrée TXT کے طور پر لے جاتا ہے اور cutover
démarrage de la fenêtre et horodatage `effective_at` et hérite de l'horodatage
override نہ کیا جائے۔ مکمل عملی workflow کے لئے
`docs/source/sorafs_gateway_dns_owner_runbook.md` دیکھیں۔

### پبلک DNS ڈیلیگیشن نوٹ

zonefile squelette صرف زون کے faisant autorité ریکارڈز متعین کرتا ہے۔ عام انٹرنیٹ کو
Il s'agit d'une zone parent et d'une délégation NS/DS avec DNS.
پر الگ سے سیٹ کرنا ضروری ہے۔
- basculement apex/TLD vers ALIAS/ANAME (spécifique au fournisseur) vers une passerelle
  Les IP anycast sont disponibles en mode A/AAAA.
- Il s'agit d'un joli hôte dérivé (`<fqdn>.gw.sora.name`) par CNAME
  شائع کریں۔
- Passerelle d'hôte canonique (`<hash>.gw.sora.id`) pour plus de détails
  پبلک زون میں شائع نہیں ہوتا۔

### Gateway ہیڈر ٹیمپلیٹ

déployer l'assistant `portal.gateway.headers.txt` et `portal.gateway.binding.json` par
Il s'agit d'une exigence de liaison de contenu de passerelle de la DG-3, comme suit :

- `portal.gateway.headers.txt` contient un bloc d'en-tête HTTP (par exemple
  `Sora-Name`, `Sora-Content-CID`, `Sora-Proof`, CSP, HSTS, et
  Descripteur `Sora-Route-Binding` (voir)) Passerelles Edge et réponse (réponse)
  ساتھ agrafé کرتے ہیں۔
- `portal.gateway.binding.json` et cartes lisibles par machine.
  Comment modifier les tickets et les liaisons hôte/cid d'automatisation et shell
  بغیر diff کر سکیں۔Il s'agit d'un `cargo xtask soradns-binding-template`, d'un résumé du manifeste et d'une passerelle.
nom d'hôte est votre nom d'hôte et `sorafs-pin-release.sh` votre nom d'hôte est votre nom d'hôte.
bloc d'en-tête pour personnaliser et personnaliser les éléments suivants :

```bash
cargo xtask soradns-binding-template \
  --manifest artifacts/sorafs/portal.manifest.json \
  --alias docs.sora \
  --hostname docs.sora.link \
  --route-label production \
  --json-out artifacts/sorafs/portal.gateway.binding.json \
  --headers-out artifacts/sorafs/portal.gateway.headers.txt
```

`--csp-template`, `--permissions-template`, et `--hsts-template` par défaut
modèles d'en-tête et remplacement des directives pour le déploiement et les directives d'en-tête
Le commutateur `--no-*` est équipé d'un en-tête et d'un connecteur de câble

extrait d'en-tête et demande de modification CDN sont également associés à une demande de modification JSON.
pipeline d'automatisation de passerelle flux d'informations lien vers la promotion de l'hôte preuves
سے میل کھائے۔

xtask helper en version canonique ریلیز اسکرپٹ aide à la vérification خودکار طور
La DG-3 a déclaré que les éléments de preuve étaient pertinents. Il s'agit d'une liaison JSON
دستی طور پر تبدیل کریں تو اسے دوبارہ چلائیں:

```bash
cargo xtask soradns-verify-binding \
  --binding artifacts/sorafs/portal.gateway.binding.json \
  --alias docs.sora.link \
  --hostname docs.sora.link \
  --proof-status ok \
  --manifest-json artifacts/sorafs/portal.manifest.json
```

Charge utile `Sora-Proof` agrafée et charge utile `Sora-Route-Binding`
les métadonnées et le CID manifeste + le nom d'hôte sont associés à une dérive d'en-tête
ہو تو فوراً fail کرتی ہے۔ جب بھی آپ یہ کمانڈ CI کے باہر چلائیں تو کنسول آؤٹ پٹ کو
Déploiement de la DG-3 en cours de déploiement
Il s'agit du basculement de liaison et de la validation de la validation.> **Intégration du descripteur DNS :** `portal.dns-cutover.json` et `gateway_binding`
> سیکشن شامل کرتا ہے جو ان آرٹیفیکٹس (chemins, contenu CID, statut de preuve, اور
> modèle d'en-tête littéral) کی طرف اشارہ کرتا ہے **اور** Strophe `route_plan`
> `gateway.route_plan.json` pour les modèles d'en-tête principaux + rollback
> Billet de changement DG-3
> `Sora-Name`/`Sora-Proof`/`CSP` ویلیوز کا فرق دیکھ سکیں اور تصدیق کر سکیں کہ
> promotion/annulation d'itinéraire dans le cadre d'un ensemble de preuves
> créer une archive کھولے۔

## مرحلہ 9 — publication de moniteurs چلائیں

Essayez-le **DOCS-3c** Essayez-le maintenant
Les liaisons de passerelle sont liées aux liens vers les liens مراحل 7-8 کے فوراً بعد consolidé
surveiller les sondes programmées et les câbles :

```bash
cd docs/portal
npm run monitor:publishing -- \
  --config ../../configs/docs_monitor.json \
  --json-out ../../artifacts/sorafs/${RELEASE_TAG}/monitoring/summary-$(date -u +%Y%m%dT%H%M%SZ).json \
  --evidence-dir ../../artifacts/sorafs/${RELEASE_TAG}/monitoring
```- `scripts/monitor-publishing.mjs` config est en cours de configuration (schéma est en cours
  `docs/portal/docs/devportal/publishing-monitoring.md` دیکھیں) et les chèques
  Voici : sondes de chemin de portail + validation CSP/Permissions-Policy, essayez-le proxy
  sondes (pour le point de terminaison `/metrics`) et pour le vérificateur de liaison de passerelle
  (`cargo xtask soradns-verify-binding`) et les vérifications d'alias/manifeste
  Sora-Content-CID en français + en français et en anglais
- Il s'agit d'une sonde de type non nulle et de tâches cron CI.
  Les alias des opérateurs de runbook sont également disponibles.
- `--json-out` Résumé du résumé de la charge utile JSON et de l'état de la cible
  `--evidence-dir` `summary.json`, `portal.json`, `tryit.json`, `binding.json`, et
  `checksums.sha256` Moniteurs pour moniteurs et moniteurs
  نتائج diff کر سکیں۔ Il s'agit d'un article `artifacts/sorafs/<tag>/monitoring/`.
  Sigstore bundle et descripteur de basculement DNS pour les utilisateurs
- surveiller l'exportation Grafana (`dashboards/grafana/docs_portal.json`) اور
  L'ID d'exercice Alertmanager est utilisé pour créer un SLO DOCS-3c
  آڈٹ ہو سکے۔ playbook de moniteur de publication dédié یہاں ہے :
  `docs/portal/docs/devportal/publishing-monitoring.md`۔

Sondes de portail HTTPS pour les URLs et les URL pour les utilisateurs `http://`
La configuration du moniteur est `allowInsecureHttp` en cours objectifs de production/mise en scène
Le TLS est également utilisé pour remplacer les aperçus et les prévisualisations.Il s'agit d'une application `npm run monitor:publishing` pour Buildkite/cron.
automatiser Il y a des URL de production pointues et des liens vers des sites Web
کرتی ہے جن پر SRE/Docs ریلیز کے درمیان انحصار کرتے ہیں۔

## `sorafs-pin-release.sh` کے ساتھ آٹومیشن

`docs/portal/scripts/sorafs-pin-release.sh` 2-6 pour encapsuler les images یہ :

1. `build/` est une archive tar déterministe.
2. `car pack`, `manifest build`, `manifest sign`, `manifest verify-signature`,
   Pour `proof verify`, il s'agit d'un produit
3. Torii informations d'identification موجود ہوں تو اختیاری طور پر `manifest submit` (alias de liaison)
   چلاتا ہے، اور
4. `artifacts/sorafs/portal.pin.report.json`, pour `portal.pin.proposal.json`,
  Descripteur de basculement DNS (soumissions par exemple) et ensemble de liaisons de passerelle
  (`portal.gateway.binding.json` + bloc d'en-tête de texte) لکھتا ہے تاکہ gouvernance,
  mise en réseau, opérations et CI pour un ensemble de preuves pour un ensemble de preuves

`PIN_ALIAS`, `PIN_ALIAS_NAMESPACE`, `PIN_ALIAS_NAME`, اور (اختیاری)
`PIN_ALIAS_PROOF_PATH` سیٹ کریں۔ essais à sec pour `--skip-submit` استعمال کریں؛
Workflow GitHub est disponible pour l'entrée `perform_submit` et est disponible en ligne.

## مرحلہ 8 — Spécifications OpenAPI pour les bundles SBOM ici

DOCS-7 est un outil de construction pour la version OpenAPI et la version SBOM.
pipeline déterministe سے گزریں۔ موجودہ helpers تینوں کو کور کرتے ہیں:

1. **spec دوبارہ بنائیں اور سائن کریں۔**

   ```bash
   npm run sync-openapi -- --version=2025-q3 --mirror=current --latest
   cargo xtask openapi --sign docs/portal/static/openapi/manifest.json
   ```Il s'agit d'un instantané pris en charge par `--version=<label>`.
   لیبل فراہم کریں (مثلاً `2025-q3`)۔ instantané d'aide
   `static/openapi/versions/<label>/torii.json` `versions/current`
   Mettre en miroir les métadonnées (SHA-256, statut du manifeste, horodatage mis à jour)
   `static/openapi/versions.json` میں ریکارڈ کرتا ہے۔ ڈیولپر پورٹل یہ index پڑھتا
   Swagger/RapiDoc est un sélecteur de version et un résumé/signature est disponible.
   info en ligne ici `--version` چھوڑ دینے سے پچھلے ریلیز لیبل برقرار رہتے
   ہیں اور صرف `current` + `latest` pointeurs تازہ ہوتے ہیں۔

   manifeste SHA-256/BLAKE3 digère la passerelle `/reference/torii-swagger`
   En-têtes `Sora-Proof` agrafés en haut

2. **Les SBOM CycloneDX sont en cours** Le pipeline de pipeline contient plusieurs SBOM basés sur Syft.
   رکھتی ہے، جیسا کہ `docs/source/sorafs_release_pipeline_plan.md` میں درج ہے۔
   Voici comment construire des artefacts :

   ```bash
   syft dir:build -o json > "$OUT"/portal.sbom.json
   syft file:docs/portal/static/openapi/torii.json -o json > "$OUT"/openapi.sbom.json
   ```

3. **Charge utile et CAR میں پیک کریں۔**

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

   مین سائٹ جیسے ہی `manifest build` / `manifest sign` مراحل پر عمل کریں، اور
   Il existe des alias pour les utilisateurs (spécifications pour `docs-openapi.sora` اور
   Le SBOM est actuellement en contact avec `docs-sbom.sora`)۔ الگ alias رکھنے سے preuves SoraDNS,
   GARs, اور rollback ٹکٹس مخصوص تک محدود رہتے ہیں۔4. **Le lien est lié à l'autorité** et l'autorité + le bundle Sigstore est associé à l'autorité.
   Liste de contrôle d'alias tuple pour une liste de contrôle pour un tuple d'alias
   Sora نام کس manifeste digest سے منسلک ہے۔

Spec/SBOM manifeste la construction de la version précédente
ریلیز ٹکٹ میں مکمل آرٹیفیکٹ سیٹ موجود ہو بغیر packer دوبارہ چلائے۔

### آٹومیشن ہیلپر (script CI/package)

`./ci/package_docs_portal_sorafs.sh` 1-8 encodent les valeurs et les valeurs de code
Le **DOCS-7** est déjà en cours de réalisation. ہیلپر :

- مطلوبہ پورٹل prep چلاتا ہے (`npm ci`, OpenAPI/norito sync, tests de widgets)؛
- Pour OpenAPI, pour les SBOM CARs + paires de manifestes pour `sorafs_cli` pour les cartes SBOM.
- اختیاری طور پر `sorafs_cli proof verify` (`--proof`) et signature Sigstore
  (`--sign`, `--sigstore-provider`, `--sigstore-audience`)
- تمام آرٹیفیکٹس کو `artifacts/devportal/sorafs/<timestamp>/` کے تحت چھوڑتا ہے اور
  `package_summary.json` Outils de CI/version pour l'ingestion de fichiers اور
- `artifacts/devportal/sorafs/latest` کو ریفریش کر کے تازہ ترین رن کی طرف اشارہ کرتا ہے۔

Exemple (Sigstore + PoR et pipeline de stockage) :

```bash
./ci/package_docs_portal_sorafs.sh \
  --proof \
  --sign \
  --sigstore-provider=github-actions \
  --sigstore-audience=sorafs-devportal
```

Drapeaux de قابلِ توجہ :- `--out <dir>` – remplacement de la racine de l'artefact (horodaté par défaut en anglais)۔
- `--skip-build` – موجودہ `docs/portal/build` دوبارہ استعمال کریں (جب CI آف لائن
  miroirs کی وجہ سے reconstruire نہ کر سکے)۔
- `--skip-sync-openapi` – `npm run sync-openapi` et `cargo xtask openapi`
  crates.io est disponible en ligne
- `--skip-sbom` – Pour `syft`, l'avertissement n'est pas disponible (avertissement d'avertissement)
- `--proof` – CAR/manifeste pour `sorafs_cli proof verify` multi-fichiers
  payloads CLI et chunk-plan pour les charges utiles `plan chunk count`
  erreurs sont liées à la porte en amont et à la porte en amont.
- `--sign` – `sorafs_cli manifest sign` Il s'agit de `SIGSTORE_ID_TOKEN`.
  (`--sigstore-token-env`) et CLI et `--sigstore-provider/--sigstore-audience`
  کے ذریعے اسے fetch کرنے دیں۔

artefacts de production `docs/portal/scripts/sorafs-pin-release.sh` استعمال کریں۔
Il s'agit du OpenAPI pour les charges utiles SBOM et du manifeste pour le manifeste.
اور `portal.additional_assets.json` میں اضافی اثاثوں کا métadonnées ریکارڈ کرتا ہے۔
helper et boutons de commande et packager CI pour les boutons de commande
`--openapi-*`, `--portal-sbom-*`, et commutateurs `--openapi-sbom-*` pour les commutateurs
Il s'agit de tuples d'alias pour le modèle `--openapi-sbom-source` de SBOM
Il est possible de remplacer les charges utiles par les charges utiles (`--skip-openapi`/`--skip-sbom`)
`syft` non par défaut pour `--syft-bin` pour `--syft-bin`.یہ اسکرپٹ ہر کمانڈ دکھاتا ہے جو وہ چلاتا ہے؛ journal `package_summary.json`
Il s'agit d'un aperçu des résumés CAR, des métadonnées du plan et du bundle Sigstore.
hashs est un shell ad hoc qui est utilisé pour créer des hachages

## مرحلہ 9 — Gateway + SoraDNS ici

cutover est en cours de création d'un alias SoraDNS et résout le problème.
رہا ہے اور gateways تازہ épreuves agrafées کر رہے ہیں :

1. **Porte de la sonde** `ci/check_sorafs_gateway_probe.sh` Fixations et fixations
   `cargo xtask sorafs-gateway-probe` چلاتا ہے جو
   `fixtures/sorafs_gateway/probe_demo/` میں موجود ہیں۔ حقیقی déploiements کے لئے
   sondez le nom d'hôte et le nom :

   ```bash
   ./ci/check_sorafs_gateway_probe.sh -- \
     --gateway "https://docs.sora/.well-known/sorafs/manifest" \
     --header "Accept: application/json" \
     --gar fixtures/sorafs_gateway/probe_demo/demo.gar.jws \
     --gar-key "demo-gar=$(<fixtures/sorafs_gateway/probe_demo/gar_pub.hex>)" \
     --host "docs.sora" \
     --report-json artifacts/sorafs_gateway_probe/ci/docs.json
   ```

   sonde `Sora-Name`, `Sora-Proof`, et `Sora-Proof-Status` et
   `docs/source/sorafs_alias_policy.md` کے مطابق décoder کرتا ہے اور اگر manifeste digest,
   TTL et les liaisons GAR avec drift et les liaisons GAR

   Pour les contrôles ponctuels légers (par exemple, lorsque seul le lot de liaisons
   modifié), exécutez `cargo xtask soradns-verify-binding --binding <portal.gateway.binding.json> --alias "<alias>" --hostname "<gateway-host>" --proof-status ok --manifest-json <portal.manifest.json>`.
   L'assistant valide le bundle de liaison capturé et est pratique pour la libération
   des billets qui nécessitent uniquement une confirmation contraignante au lieu d’un exercice d’enquête complet.2. ** Preuves de forage en cours ** Forets en cours et essais à sec PagerDuty et sonde
   `scripts/telemetry/run_sorafs_gateway_probe.sh --scenario devportal-rollout -- …`
   کے ساتھ wrap کریں۔ en-têtes/journaux du wrapper
   `artifacts/sorafs_gateway_probe/<stamp>/` est en ligne avec `ops/drill-log.md`.
   Il existe des crochets de restauration et des charges utiles PagerDuty pour les utilisateurs. SoraDNS
   Le code IP en dur est `--host docs.sora` pour le code en dur IP noir

3. **Liaisons DNS pour la vérification** Pour la preuve d'alias, la sonde est disponible.
   حوالہ دیا گیا GAR فائل (`--gar`) ریکارڈ کریں اور ریلیز preuve کے ساتھ منسلک کریں۔
   Resolver est un outil de résolution `tools/soradns-resolver` qui est en cours de réalisation.
   کہ entrées mises en cache نئے manifeste کو honneur کریں۔ JSON prend en charge le code source
   `cargo xtask soradns-verify-gar --gar <path> --name <alias> [--manifest-cid <cid>] [--telemetry-label <label>]`
   Cartographie déterministe des hôtes, métadonnées manifestes et étiquettes de télémétrie hors ligne
   valider ہو جائیں۔ assistant signé GAR کے ساتھ `--json-out` résumé بھی نکال سکتا ہے
   Les critiques des critiques binaires sont des preuves de preuve
  نیا GAR بناتے وقت ترجیح دیں
  `cargo xtask soradns-gar-template --name <alias> --manifest <portal.manifest.json> --telemetry-label <label> ...`
  (`--manifest-cid <cid>` پر صرف تب جائیں جب manifeste فائل دستیاب نہ ہو)۔ assistant اب
  CID **اور** BLAKE3 digest pour le manifeste JSON et les espaces vides
  trim کرتا ہے، دہرے `--telemetry-label` flags ختم کرتا ہے، labels trier کرتا ہے، اور
  JSON propose des modèles CSP/HSTS/Permissions-Policy par défaut
  charge utile déterministe étiquettes étiquettes étiquettes obus obus étiquettes4. **métriques d'alias pour la version actuelle** `torii_sorafs_alias_cache_refresh_duration_ms`
   Pour `torii_sorafs_gateway_refusals_total{profile="docs"}`, sonde et capteur
   اسکرین پر رکھیں؛ Série `dashboards/grafana/docs_portal.json` pour la série `dashboards/grafana/docs_portal.json`

## مرحلہ 10 — Surveillance et regroupement de preuves

- **Tableaux de bord۔** ہر ریلیز کے لئے `dashboards/grafana/docs_portal.json` (SLO du portail)،
  `dashboards/grafana/sorafs_gateway_observability.json` (latence de la passerelle + preuve d'état de santé)،
  اور `dashboards/grafana/sorafs_fetch_observability.json` (santé de l'orchestre)
  exporter کریں۔ JSON exporte des fichiers vers d'autres fichiers
  Prometheus requêtes pour les utilisateurs
- **Archives de sonde** `artifacts/sorafs_gateway_probe/<stamp>/` et git-annex ici
  اپنے seau de preuves میں رکھیں۔ résumé de la sonde, en-têtes, et script de télémétrie
  La charge utile PagerDuty est disponible
- **Release bundle۔** پورٹل/SBOM/OpenAPI Résumés CAR, bundles de manifestes, Sigstore
  signatures, `portal.pin.report.json`, journaux de sonde Try-It et rapports de vérification des liens
  ایک timestamped فولڈر (مثال `artifacts/sorafs/devportal/20260212T1103Z/`) میں رکھیں۔
- **Journal de forage** جب sondes کسی forage کا حصہ ہوں تو
  `scripts/telemetry/run_sorafs_gateway_probe.sh` et `ops/drill-log.md` ajouter
  Il s'agit d'une preuve et d'une exigence de chaos SNNet-5.
- **Liens de ticket** modifier le ticket avec les identifiants du panneau Grafana et les exportations PNG jointes
  حوالہ دیں، ساتھ ہی chemin du rapport de sonde et accès au shell des réviseurs
  Vérifiez par recoupement les SLO

## مرحلہ 11 — Exercice de récupération multi-sources et preuves du tableau de bordSoraFS pour la recherche de preuves multi-sources (DOCS-7/SF-6)
اور یہ اوپر والے DNS/gateway proofs کے ساتھ ہونا چاہیے۔ broche manifeste کرنے کے بعد :

1. **manifeste en direct et `sorafs_fetch` چلائیں۔** 2-3 plan/manifeste
   Fournisseur de fournisseur d'accès et informations d'identification de la passerelle ہر
   Il s'agit d'un orchestrateur et d'un processus de décision et d'une relecture :

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

   - manifeste میں حوالہ دی گئی fournisseur d'annonces پہلے حاصل کریں (مثلاً
     `sorafs_cli manifest describe --provider-adverts-out artifacts/sorafs/provider_adverts/`)
     اور انہیں `--provider-advert name=path` کے ذریعے پاس کریں تاکہ scoreboard
     les fenêtres de capacités déterministes d'analyse évaluent les capacités
     `--allow-implicit-provider-metadata` **صرف** CI میں replay des rencontres کرتے وقت
     استعمال کریں؛ forets de production میں pin کے ساتھ آنے والے annonces signées ہی دیں۔
   - Les régions manifestes et les tuples du fournisseur sont également disponibles.
     Il y a un cache/alias correspondant à l'artefact de récupération correspondant

2. **آؤٹ پٹس آرکائیو کریں۔** `scoreboard.json`, `providers.ndjson`, `fetch.json`, et
   `chunk_receipts.ndjson` کو ریلیز preuves فولڈر کے اندر رکھیں۔ یہ فائلیں pair
   pondération, budget de nouvelle tentative, latence EWMA, et réceptions par morceau
   Le SF-7 est un appareil photo de type SF-7.3. **Récupérer les sorties et **SoraFS Récupérer l'observabilité**
   ڈیش بورڈ (`dashboards/grafana/sorafs_fetch_observability.json`) میں امپورٹ کریں،
   `torii_sorafs_fetch_duration_ms`/`_failures_total` pour les panneaux de la gamme fournisseur ici
   anomalies دیکھیں۔ Grafana instantanés du panneau et chemin du tableau de bord
   لنک کریں۔

4. **règles d'alerte pour la fumée et la fumée** `scripts/telemetry/test_sorafs_fetch_alerts.sh`
   L'ensemble d'alertes Prometheus est validé par le groupe d'alertes Prometheus. outil de promotion
   Il s'agit d'un processus de décrochage de DOCS-7.
   alertes de fournisseur lent مسلح رہیں۔

5. ** Fil de connexion CI ** Flux de travail des broches de connexion `sorafs_fetch` et `perform_fetch_probe`
   entrée کے پیچھے رکھا گیا ہے؛ mise en scène/production رنز میں اسے فعال کریں تاکہ chercher
   paquet de preuves manifestes کے ساتھ خودکار طور پر تیار ہو۔ لوکل exercices et اسکرپٹ
   Comment exporter des jetons de passerelle vers `PIN_FETCH_PROVIDERS`
   liste de fournisseurs séparés par des virgules سے سیٹ کیا جائے۔

## پروموشن، observabilité، اور rollback1. **پروموشن :** mise en scène et production et alias de production پروموشن کے لئے
   اسی manifeste/bundle کے ساتھ `manifest submit` دوبارہ چلائیں اور
   `--alias-namespace/--alias-name` et alias de production ici. اس سے QA منظور
   La broche de préparation est en cours de construction et la re-signe est terminée.
2. **Surveillance :** registre des broches
   (`docs/source/grafana_sorafs_pin_registry.json`) اور پورٹل مخصوص sondes
   (`docs/portal/docs/devportal/observability.md` دیکھیں) امپورٹ کریں۔ dérive de la somme de contrôle,
   sondes ayant échoué, pics de tentatives de preuve et alertes
3. **Rollback :** واپس جانے کے لئے پچھلا manifest دوبارہ submit کریں (یا موجودہ alias
   prendre sa retraite کریں) `sorafs_cli manifest submit --alias ... --retire` کے ذریعے۔ ہمیشہ
   dernier paquet en bon état connu et résumé CAR
   preuves d'annulation دوبارہ بن سکیں۔

## Modèle de flux de travail CI

Il s'agit d'un pipeline et d'un pipeline :

1. Build + Lint (`npm ci`, `npm run build`, génération de somme de contrôle).
2. Package (`car pack`) pour les manifestes de calcul
3. OIDC correspondant au signe de travail (`manifest sign`).
4. Audit des artefacts اپ لوڈ کریں (CAR, manifeste, liasse, plan, résumés).
5. registre des broches et soumettre la commande :
   - Demandes d'extraction → `docs-preview.sora`.
   - Tags/branches protégées → promotion des alias de production.
6. sortie سے پہلے sondes + portes de vérification de preuve چلائیں۔

`.github/workflows/docs-portal-sorafs-pin.yml` versions manuelles en cours de réalisation
مراحل جوڑتا ہے۔ flux de travail :- پورٹل build/test کرتا ہے،
- `scripts/sorafs-pin-release.sh` est un build de construction complet.
- GitHub OIDC prend en charge le bundle de manifestes/vérifie le fichier manifeste
- CAR/manifeste/bundle/plan/preuve résumés کو artefacts کے طور پر اپ لوڈ کرتا ہے، اور
- (اختیاری) secrets موجود ہوں تو manifeste + liaison d'alias soumettre کرتا ہے۔

le travail consiste à créer des secrets/variables du référentiel :

| Nom | Objectif |
|------|--------------|
| `DOCS_SORAFS_TORII_URL` | Hôte Torii et `/v1/sorafs/pin/register` pour les utilisateurs |
| `DOCS_SORAFS_SUBMITTED_EPOCH` | soumissions کے ساتھ ریکارڈ ہونے والا identifiant d'époque۔ |
| `DOCS_SORAFS_AUTHORITY` / `DOCS_SORAFS_PRIVATE_KEY` | soumission manifeste کے لئے pouvoir de signature۔ |
| `DOCS_SORAFS_ALIAS_NAMESPACE` / `DOCS_SORAFS_ALIAS_NAME` | alias tuple par `perform_submit` true pour manifest et bind pour |
| `DOCS_SORAFS_ALIAS_PROOF_B64` | Ensemble de preuves d'alias codés en base64 (liaison d'alias چھوڑنے کے لئے omettre کریں)۔ |
| `DOCS_ANALYTICS_*` | Points finaux d'analyse/de sonde et flux de travail plus réutilisation et plus |

Actions UI et déclencheur de workflow par exemple :

1. `alias_label` en option (`docs.sora.link`) et `proposal_alias` en option,
   et remplacement `release_tag` en option
2. artefacts pour `perform_submit` non coché (Torii tactile pour)
   Je veux activer l'alias configuré pour publier et publier`docs/source/sorafs_ci_templates.md` Un dépôt de pension pour un dépôt de garantie
assistants CI génériques pour le flux de travail de travail

## Liste de contrôle

-[ ] `npm run build`, `npm run test:*`, et `npm run check:links` گرین ہیں۔
-[ ] `build/checksums.sha256` et `build/release.json` artefacts میں محفوظ ہیں۔
- [ ] RCA, plan, manifeste, et résumé `artifacts/` کے تحت بنے ہیں۔
- [ ] Sigstore bundle + signature détachée لاگز کے ساتھ محفوظ ہیں۔
-[ ] `portal.manifest.submit.summary.json` et `portal.manifest.submit.response.json`
      soumissions کے وقت محفوظ ہوئے۔
- [ ] `portal.pin.report.json` (اور اختیاری `portal.pin.proposal.json`)
      RCA/objets manifestes کے ساتھ آرکائیو ہوئے۔
-[ ] `proof verify` et `manifest verify-signature` pour le client
- [ ] Tableaux de bord Grafana en haut + sondes Try-It en haut
- [ ] notes de restauration (ID de manifeste précédent + résumé d'alias)