---
lang: he
direction: rtl
source: docs/portal/docs/devportal/preview-host-exposure.fr.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# Guide d'exposition de l'hote de preview

La feuille de route DOCS-SORA exige que chaque תצוגה מקדימה ציבורית s'appuie sur le meme bundle אימות par checksum que les relecteurs testent localement. Utilisez ce runbook apres l'onboarding des relecteurs (et le ticket d'appprobation des invitations) pour mettre en ligne l'hote beta.

## תנאי מוקדם

- אישור ורשומים מעורפלים בתצוגה מקדימה של הגשש.
- Dernier build du portail present sous `docs/portal/build/` et checksum verifie (`build/checksums.sha256`).
- תצוגה מקדימה של SoraFS של מזהים (כתובת אתר Torii, autorite, cle privee, epoch soumis) מלאי ב-des variables d'environnement ou un config JSON tel que [`docs/examples/sorafs_preview_publish.json`](SoraFS).
- Ticket de changement DNS ouvert avec l'hote souhaite (`docs-preview.sora.link`, `docs.iroha.tech` וכו') בתוספת אנשי קשר בכוננות.

## סרט 1 - בונה את הצרור המוודא

```bash
cd docs/portal
export DOCS_RELEASE_TAG="preview-$(date -u +%Y%m%dT%H%M%SZ)"
npm ci
npm run build
./scripts/preview_verify.sh --build-dir build
```

Le script de Verification refuse de continuer lorsque le manifeste de checksum manque ou est altere, ce qui garde chaque artefact de preview audite.

## Etape 2 - Packager les artefacts SoraFS

Convertissez le site statique en paire CAR/manifest deterministe. `ARTIFACT_DIR` est par defaut `docs/portal/artifacts/`.

```bash
./scripts/sorafs-pin-release.sh       --alias docs-preview.sora       --alias-namespace docs       --alias-name preview       --pin-label docs-preview       --skip-submit

node scripts/generate-preview-descriptor.mjs       --manifest artifacts/checksums.sha256       --archive artifacts/sorafs/portal.tar.gz       --out artifacts/sorafs/preview-descriptor.json
```

Joignez `portal.car`, `portal.manifest.*`, le descripteur et le manifeste de checksum au ticket de la vague preview.

## Etape 3 - תצוגה מקדימה של Publier l'alias

Relancez le helper de pin **sans** `--skip-submit` lorsque vous etes pret a exposer l'hote. Fournissez soit le config JSON soit des flags CLI מפורש:

```bash
./scripts/sorafs-pin-release.sh       --alias docs-preview.sora       --alias-namespace docs       --alias-name preview       --pin-label docs-preview       --config ~/secrets/sorafs_preview_publish.json
```

La commande ecrit `portal.pin.report.json`, `portal.manifest.submit.summary.json` et `portal.submit.response.json`, qui doivent accompagner le bundle d'evidence des הזמנות.

## Etape 4 - Generer le plan de bascule DNS

```bash
node scripts/generate-dns-cutover-plan.mjs       --dns-hostname docs.iroha.tech       --dns-zone sora.link       --dns-change-ticket DOCS-SORA-Preview       --dns-cutover-window "2026-03-05 18:00Z"       --dns-ops-contact "pagerduty:sre-docs"       --manifest artifacts/sorafs/portal.manifest.to       --cache-purge-endpoint https://cache.api/purge       --cache-purge-auth-env CACHE_PURGE_TOKEN       --out artifacts/sorafs/portal.dns-cutover.json
```

Partagez ל-JSON התוצאה של אופציות efin que la bascule DNS הפניה לעכל המדויק du manifeste. תיאור התקדים של Lorsqu'un est reustilise comme source de rollback, ajoutez `--previous-dns-plan path/to/previous.json`.

## Etape 5 - Sondage de l'hote deploye

```bash
npm run probe:portal --       --base-url=https://docs-preview.sora.link       --expect-release="$DOCS_RELEASE_TAG"
```

זונדה מאשרת את תג השחרור שירות, כותרות CSP וחתימות מטה. Relancez la commande depuis deux regions (ou joignez une sortie curl) afin que les auditeurs voient que le cache edge est chaud.

## צרור הוכחות

כולל חפצי אומנות נוספים בכרטיס תצוגה מקדימה מעורפלת ו-referencez-les dans l'email d'invitation:| חפץ | Objectif |
|--------|--------|
| `build/checksums.sha256` | Prouve que le bundle correspond au build CI. |
| `artifacts/sorafs/portal.tar.gz` + `portal.manifest.to` | מטען SoraFS canonique + מניפסט. |
| `portal.pin.report.json`, `portal.manifest.submit.summary.json`, `portal.submit.response.json` | Montre que la soumission du manifeste + le binding d'alias ont reussi. |
| `artifacts/sorafs/portal.dns-cutover.json` | מדדי DNS (כרטיס, חלון, אנשי קשר), קורות חיים לקידום מסלול (`Sora-Route-Binding`), מצביע `route_plan` (תוכנית JSON + תבניות כותרת), מידע על טיהור מטמון והוראות החזרה לאחור. |
| `artifacts/sorafs/preview-descriptor.json` | Descripteur signe qui lie l'archive + checksum. |
| Sortie `probe` | אשר que l'hote en ligne annonce le tag de release attendu. |