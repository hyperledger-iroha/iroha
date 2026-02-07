---
lang: fr
direction: ltr
source: docs/portal/docs/devportal/preview-host-exposure.ur.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# پریویو ہوسٹ ایکسپوژر گائیڈ

DOCS-SORA est un ensemble de paquets vérifiés par somme de contrôle et un bundle vérifié par somme de contrôle. لوکل طور پر چلاتے ہیں۔ ریویور آن بورڈنگ (اور invitation منظوری ٹکٹ) مکمل ہونے کے بعد اس runbook کو استعمال کریں تاکہ بیٹا پریویو ہوسٹ آن لائن لایا جا سکے۔

## پیشگی شرائط

- ریویور آن بورڈنگ ویو منظور اور پریویو ٹریکر میں درج ہو چکی ہو۔
- Vous avez construit la build `docs/portal/build/` avec la somme de contrôle et la somme de contrôle (`build/checksums.sha256`).
- SoraFS code source (Torii URL, autorité, clé privée, époque) et configuration JSON محفوظ ہوں جیسے [`docs/examples/sorafs_preview_publish.json`](../../../examples/sorafs_preview_publish.json).
- Nom d'hôte (`docs-preview.sora.link`, `docs.iroha.tech` et) et serveur DNS de garde et de garde. ہوں۔

## مرحلہ 1 - bundle بنائیں اور verify کریں

```bash
cd docs/portal
export DOCS_RELEASE_TAG="preview-$(date -u +%Y%m%dT%H%M%SZ)"
npm ci
npm run build
./scripts/preview_verify.sh --build-dir build
```

vérifier le manifeste de contrôle de la somme de contrôle پریویو آرٹیفیکٹ آڈٹ میں رہتا ہے۔

## مرحلہ 2 - SoraFS artefacts پیک کریں

اسٹیٹک سائٹ کو déterministe CAR/manifeste جوڑی میں تبدیل کریں۔ `ARTIFACT_DIR` en ligne et `docs/portal/artifacts/` en ligne

```bash
./scripts/sorafs-pin-release.sh       --alias docs-preview.sora       --alias-namespace docs       --alias-name preview       --pin-label docs-preview       --skip-submit

node scripts/generate-preview-descriptor.mjs       --manifest artifacts/checksums.sha256       --archive artifacts/sorafs/portal.tar.gz       --out artifacts/sorafs/preview-descriptor.json
```

Il s'agit d'un descripteur `portal.car`, `portal.manifest.*`, d'un manifeste de somme de contrôle et d'un manifeste de somme de contrôle.

## مرحلہ 3 - پریویو alias شائع کریںIl s'agit d'un assistant de broche `--skip-submit` pour un assistant de broche `--skip-submit`. La configuration JSON et les indicateurs CLI sont les suivants :

```bash
./scripts/sorafs-pin-release.sh       --alias docs-preview.sora       --alias-namespace docs       --alias-name preview       --pin-label docs-preview       --config ~/secrets/sorafs_preview_publish.json
```

یہ کمانڈ `portal.pin.report.json`, `portal.manifest.submit.summary.json` et `portal.submit.response.json` لکھتی ہے، pour inviter un ensemble de preuves کے ساتھ ہونی چاہئیں۔

## مرحلہ 4 - DNS basculement ici

```bash
node scripts/generate-dns-cutover-plan.mjs       --dns-hostname docs.iroha.tech       --dns-zone sora.link       --dns-change-ticket DOCS-SORA-Preview       --dns-cutover-window "2026-03-05 18:00Z"       --dns-ops-contact "pagerduty:sre-docs"       --manifest artifacts/sorafs/portal.manifest.to       --cache-purge-endpoint https://cache.api/purge       --cache-purge-auth-env CACHE_PURGE_TOKEN       --out artifacts/sorafs/portal.dns-cutover.json
```

Il s'agit d'un JSON et d'Ops qui sont en train de créer un résumé DNS et un résumé du manifeste. La restauration du descripteur est terminée et le descripteur est utilisé pour `--previous-dns-plan path/to/previous.json`.

## مرحلہ 5 - ڈیپلائےڈ ہوسٹ کو sonde کریں

```bash
npm run probe:portal --       --base-url=https://docs-preview.sora.link       --expect-release="$DOCS_RELEASE_TAG"
```

sonde, balise de version, en-têtes CSP, métadonnées de signature et métadonnées Le cache de sortie de boucle (sortie curl est disponible) Le cache périphérique est installé ہے۔

## Lot de preuves

Il existe de nombreux artefacts et des objets similaires:| Artefact | مقصد |
|--------------|------|
| `build/checksums.sha256` | Il s'agit d'un bundle CI build pour un projet complet |
| `artifacts/sorafs/portal.tar.gz` + `portal.manifest.to` | charge utile canonique SoraFS + manifeste. |
| `portal.pin.report.json`, `portal.manifest.submit.summary.json`, `portal.submit.response.json` | soumission manifeste et liaison d'alias کی کامیابی دکھاتا ہے۔ |
| `artifacts/sorafs/portal.dns-cutover.json` | Métadonnées DNS (pour les utilisateurs) et promotion de la route (`Sora-Route-Binding`) avec pointeur `route_plan` (JSON + modèles d'en-tête) et purge du cache pour Ops کے لئے rollback ہدایات۔ |
| `artifacts/sorafs/preview-descriptor.json` | Ajouter un descripteur et une archive + une somme de contrôle |
| Sortie `probe` | Il s'agit d'une balise de sortie pour votre compte. |