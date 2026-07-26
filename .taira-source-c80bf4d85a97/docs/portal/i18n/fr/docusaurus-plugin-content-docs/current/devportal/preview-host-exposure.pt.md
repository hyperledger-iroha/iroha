---
lang: fr
direction: ltr
source: docs/portal/docs/devportal/preview-host-exposure.pt.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# Guide d'exposition de l'hôte de l'aperçu

La feuille de route DOCS-SORA exige que tout aperçu public utilise le même bundle vérifié par la somme de contrôle que les réviseurs exercent localement. Utilisez ce runbook après l'intégration des réviseurs (et le ticket d'approbation des invités) pour les héberger en ligne.

## Pré-requis

- Une fois l'intégration des réviseurs approuvés et enregistrés dans le suivi de l'aperçu.
- Ultimo build do portal présente le `docs/portal/build/` et la somme de contrôle vérifiée (`build/checksums.sha256`).
- Les informations d'identification de prévisualisation SoraFS (URL Torii, autorisée, privée, époque envoyée) sont associées à des variables ambiantes ou à une configuration JSON comme [`docs/examples/sorafs_preview_publish.json`](../../../examples/sorafs_preview_publish.json).
- Ticket de gestion DNS ouvert avec le nom d'hôte souhaité (`docs-preview.sora.link`, `docs.iroha.tech`, etc.) avec des contacts d'astreinte.

## Étape 1 - Construire et vérifier le bundle

```bash
cd docs/portal
export DOCS_RELEASE_TAG="preview-$(date -u +%Y%m%dT%H%M%SZ)"
npm ci
npm run build
./scripts/preview_verify.sh --build-dir build
```

Le script de vérification est récusé en continu lorsque le manifeste de somme de contrôle est ausente ou adultéré, en tenant compte de chaque artéfact de prévisualisation audité.

## Passo 2 - Empacotar os artefatos SoraFS

Convertir le site statique en un CAR/manifeste déterministe. `ARTIFACT_DIR` prend en charge et `docs/portal/artifacts/`.

```bash
./scripts/sorafs-pin-release.sh       --alias docs-preview.sora       --alias-namespace docs       --alias-name preview       --pin-label docs-preview       --skip-submit

node scripts/generate-preview-descriptor.mjs       --manifest artifacts/checksums.sha256       --archive artifacts/sorafs/portal.tar.gz       --out artifacts/sorafs/preview-descriptor.json
```

Annexe `portal.car`, `portal.manifest.*`, le descripteur et le manifeste de somme de contrôle sur le ticket pour l'aperçu.

## Étape 3 - Publier l'alias de l'aperçuRéexécutez l'assistant de broche **sem** `--skip-submit` lorsque vous êtes prêt à exporter l'hôte. Forneca o config JSON ou flags CLI explicitos :

```bash
./scripts/sorafs-pin-release.sh       --alias docs-preview.sora       --alias-namespace docs       --alias-name preview       --pin-label docs-preview       --config ~/secrets/sorafs_preview_publish.json
```

La commande est `portal.pin.report.json`, `portal.manifest.submit.summary.json` et `portal.submit.response.json`, qui doit accompagner le paquet de preuves des invités.

## Passo 4 - Gérer le plan de cour DNS

```bash
node scripts/generate-dns-cutover-plan.mjs       --dns-hostname docs.iroha.tech       --dns-zone sora.link       --dns-change-ticket DOCS-SORA-Preview       --dns-cutover-window "2026-03-05 18:00Z"       --dns-ops-contact "pagerduty:sre-docs"       --manifest artifacts/sorafs/portal.manifest.to       --cache-purge-endpoint https://cache.api/purge       --cache-purge-auth-env CACHE_PURGE_TOKEN       --out artifacts/sorafs/portal.dns-cutover.json
```

Comparez le résultat JSON avec les opérations pour modifier la référence DNS ou le résumé extrait du manifeste. Pour réutiliser un descripteur antérieur à l'origine de la restauration, ajoutez `--previous-dns-plan path/to/previous.json`.

## Étape 5 - Tester l'hôte implanté

```bash
npm run probe:portal --       --base-url=https://docs-preview.sora.link       --expect-release="$DOCS_RELEASE_TAG"
```

La sonde confirme la balise de service de publication, les en-têtes CSP et les métadonnées d'extinction. Répétez la commande à partir de deux régions (ou annexe à ladite boucle) pour que les auditeurs voient que le cache Edge est à ce moment-là.

## Bundle de preuves

Inclut les articles suivants sans ticket pour l'avant-première et les références dans l'e-mail de convocation :| Artefato | Proposé |
|--------------|---------------|
| `build/checksums.sha256` | Vérifiez que le bundle correspond à la build du CI. |
| `artifacts/sorafs/portal.tar.gz` + `portal.manifest.to` | Charge utile canonico SoraFS + manifeste. |
| `portal.pin.report.json`, `portal.manifest.submit.summary.json`, `portal.submit.response.json` | Montrez que l'envoi du manifeste + l'alias contraignant pour le forum conclu. |
| `artifacts/sorafs/portal.dns-cutover.json` | Métadados DNS (ticket, janvier, contacts), CV de promotion de rotation (`Sora-Route-Binding`), pont `route_plan` (plan JSON + modèles d'en-tête), informations de purge du cache et instructions de restauration pour les opérations. |
| `artifacts/sorafs/preview-descriptor.json` | Descripteur assinado que liga o archive + checksum. |
| Saïda do `probe` | Confirmez que l'hôte a annoncé en direct le tag de sortie attendu. |