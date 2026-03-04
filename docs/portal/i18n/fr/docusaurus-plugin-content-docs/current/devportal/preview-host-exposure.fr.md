---
lang: fr
direction: ltr
source: docs/portal/docs/devportal/preview-host-exposure.fr.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# Guide d'exposition de l'hôte de prévisualisation

La feuille de route DOCS-SORA exige que chaque aperçu public s'appuie sur le meme bundle verifie par checksum que les rélecteurs testent localement. Utilisez ce runbook après l'onboarding des rélecteurs (et le ticket d'approbation des invitations) pour mettre en ligne l'hôte bêta.

## Prérequis

- Vague d'onboarding des rélecteurs approuvés et enregistrés dans le tracker preview.
- Dernier build du portail présent sous `docs/portal/build/` et checksum verifie (`build/checksums.sha256`).
- Identifiants SoraFS preview (URL Torii, autorité, clé privée, époque soumise) stockés dans des variables d'environnement ou un config JSON tel que [`docs/examples/sorafs_preview_publish.json`](../../../examples/sorafs_preview_publish.json).
- Ticket de changement DNS ouvert avec l'hôte souhaite (`docs-preview.sora.link`, `docs.iroha.tech`, etc.) plus contacts d'astreinte.

## Etape 1 - Construire et vérifier le bundle

```bash
cd docs/portal
export DOCS_RELEASE_TAG="preview-$(date -u +%Y%m%dT%H%M%SZ)"
npm ci
npm run build
./scripts/preview_verify.sh --build-dir build
```

Le script de vérification refuse de continuer lorsque le manifeste de checksum manque ou est modifié, ce qui garde chaque artefact de prévisualisation audité.

## Etape 2 - Packager les artefacts SoraFS

Convertissez le site statique en paire CAR/manifeste déterministe. `ARTIFACT_DIR` est par défaut `docs/portal/artifacts/`.

```bash
./scripts/sorafs-pin-release.sh       --alias docs-preview.sora       --alias-namespace docs       --alias-name preview       --pin-label docs-preview       --skip-submit

node scripts/generate-preview-descriptor.mjs       --manifest artifacts/checksums.sha256       --archive artifacts/sorafs/portal.tar.gz       --out artifacts/sorafs/preview-descriptor.json
```

Joignez `portal.car`, `portal.manifest.*`, le descripteur et le manifeste de checksum au ticket de la vague aperçu.

## Etape 3 - Aperçu Publier l'aliasRelancez le helper de pin **sans** `--skip-submit` lorsque vous êtes prêt à exposer l'hôte. Fournissez soit le config JSON soit des flags CLI explicites :

```bash
./scripts/sorafs-pin-release.sh       --alias docs-preview.sora       --alias-namespace docs       --alias-name preview       --pin-label docs-preview       --config ~/secrets/sorafs_preview_publish.json
```

La commande écrite `portal.pin.report.json`, `portal.manifest.submit.summary.json` et `portal.submit.response.json`, qui doivent accompagner le bundle d'evidence des invitations.

## Etape 4 - Générer le plan de bascule DNS

```bash
node scripts/generate-dns-cutover-plan.mjs       --dns-hostname docs.iroha.tech       --dns-zone sora.link       --dns-change-ticket DOCS-SORA-Preview       --dns-cutover-window "2026-03-05 18:00Z"       --dns-ops-contact "pagerduty:sre-docs"       --manifest artifacts/sorafs/portal.manifest.to       --cache-purge-endpoint https://cache.api/purge       --cache-purge-auth-env CACHE_PURGE_TOKEN       --out artifacts/sorafs/portal.dns-cutover.json
```

Partagez le JSON résultant avec Ops afin que la bascule DNS reference le digest exact du manifeste. Lorsqu'un descripteur précédent est réutilisé comme source de rollback, ajoutez `--previous-dns-plan path/to/previous.json`.

## Etape 5 - Sondage de l'hôte déployé

```bash
npm run probe:portal --       --base-url=https://docs-preview.sora.link       --expect-release="$DOCS_RELEASE_TAG"
```

La sonde confirme le tag de release servi, les en-têtes CSP et les métadonnées de signature. Relancez la commande depuis deux régions (ou joignez une sortie curl) afin que les auditeurs voient que le cache edge est chaud.

## Bundle de preuves

Incluez les artefacts suivants dans le ticket de la vague aperçu et référencez-les dans l'email d'invitation :| Artefact | Objectif |
|----------|----------|
| `build/checksums.sha256` | Prouvez que le bundle correspond au build CI. |
| `artifacts/sorafs/portal.tar.gz` + `portal.manifest.to` | Charge utile SoraFS canonique + manifeste. |
| `portal.pin.report.json`, `portal.manifest.submit.summary.json`, `portal.submit.response.json` | Montre que la soumission du manifeste + le contraignant d'alias ont reussi. |
| `artifacts/sorafs/portal.dns-cutover.json` | Metadonnées DNS (ticket, fenêtre, contacts), CV de promotion de route (`Sora-Route-Binding`), pointeur `route_plan` (plan JSON + templates de header), infos de purge cache et instructions de rollback pour Ops. |
| `artifacts/sorafs/preview-descriptor.json` | Descripteur signe qui lie l'archive + checksum. |
| Sortie `probe` | Confirmez que l'hôte en ligne annonce le tag de release attendu. |