---
lang: ar
direction: rtl
source: docs/portal/docs/devportal/preview-host-exposure.fr.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
---

# Guide d'exposition de l'hote de preview

La feuille de route DOCS-SORA exige que chaque preview public s'appuie sur le meme bundle verifie par checksum que les relecteurs testent localement. Utilisez ce runbook apres l'onboarding des relecteurs (et le ticket d'approbation des invitations) pour mettre en ligne l'hote beta.

## Prerequis

- Vague d'onboarding des relecteurs approuvee et enregistree dans le tracker preview.
- Dernier build du portail present sous `docs/portal/build/` et checksum verifie (`build/checksums.sha256`).
- Identifiants SoraFS preview (URL Torii, autorite, cle privee, epoch soumis) stockes dans des variables d'environnement ou un config JSON tel que [`docs/examples/sorafs_preview_publish.json`](../../../examples/sorafs_preview_publish.json).
- Ticket de changement DNS ouvert avec l'hote souhaite (`docs-preview.sora.link`, `docs.iroha.tech`, etc.) plus contacts on-call.

## Etape 1 - Construire et verifier le bundle

```bash
cd docs/portal
export DOCS_RELEASE_TAG="preview-$(date -u +%Y%m%dT%H%M%SZ)"
npm ci
npm run build
./scripts/preview_verify.sh --build-dir build
```

Le script de verification refuse de continuer lorsque le manifeste de checksum manque ou est altere, ce qui garde chaque artefact de preview audite.

## Etape 2 - Packager les artefacts SoraFS

Convertissez le site statique en paire CAR/manifest deterministe. `ARTIFACT_DIR` est par defaut `docs/portal/artifacts/`.

```bash
./scripts/sorafs-pin-release.sh       --alias docs-preview.sora       --alias-namespace docs       --alias-name preview       --pin-label docs-preview       --skip-submit

node scripts/generate-preview-descriptor.mjs       --manifest artifacts/checksums.sha256       --archive artifacts/sorafs/portal.tar.gz       --out artifacts/sorafs/preview-descriptor.json
```

Joignez `portal.car`, `portal.manifest.*`, le descripteur et le manifeste de checksum au ticket de la vague preview.

## Etape 3 - Publier l'alias preview

Relancez le helper de pin **sans** `--skip-submit` lorsque vous etes pret a exposer l'hote. Fournissez soit le config JSON soit des flags CLI explicites:

```bash
./scripts/sorafs-pin-release.sh       --alias docs-preview.sora       --alias-namespace docs       --alias-name preview       --pin-label docs-preview       --config ~/secrets/sorafs_preview_publish.json
```

La commande ecrit `portal.pin.report.json`, `portal.manifest.submit.summary.json` et `portal.submit.response.json`, qui doivent accompagner le bundle d'evidence des invitations.

## Etape 4 - Generer le plan de bascule DNS

```bash
node scripts/generate-dns-cutover-plan.mjs       --dns-hostname docs.iroha.tech       --dns-zone sora.link       --dns-change-ticket DOCS-SORA-Preview       --dns-cutover-window "2026-03-05 18:00Z"       --dns-ops-contact "pagerduty:sre-docs"       --manifest artifacts/sorafs/portal.manifest.to       --cache-purge-endpoint https://cache.api/purge       --cache-purge-auth-env CACHE_PURGE_TOKEN       --out artifacts/sorafs/portal.dns-cutover.json
```

Partagez le JSON resultant avec Ops afin que la bascule DNS reference le digest exact du manifeste. Lorsqu'un descripteur precedent est reutilise comme source de rollback, ajoutez `--previous-dns-plan path/to/previous.json`.

## Etape 5 - Sondage de l'hote deploye

```bash
npm run probe:portal --       --base-url=https://docs-preview.sora.link       --expect-release="$DOCS_RELEASE_TAG"
```

La sonde confirme le tag de release servi, les headers CSP et les metadonnees de signature. Relancez la commande depuis deux regions (ou joignez une sortie curl) afin que les auditeurs voient que le cache edge est chaud.

## Bundle d'evidence

Incluez les artefacts suivants dans le ticket de la vague preview et referencez-les dans l'email d'invitation:

| Artefact | Objectif |
|----------|----------|
| `build/checksums.sha256` | Prouve que le bundle correspond au build CI. |
| `artifacts/sorafs/portal.tar.gz` + `portal.manifest.to` | Payload SoraFS canonique + manifeste. |
| `portal.pin.report.json`, `portal.manifest.submit.summary.json`, `portal.submit.response.json` | Montre que la soumission du manifeste + le binding d'alias ont reussi. |
| `artifacts/sorafs/portal.dns-cutover.json` | Metadonnees DNS (ticket, fenetre, contacts), resume de promotion de route (`Sora-Route-Binding`), pointeur `route_plan` (plan JSON + templates de header), infos de purge cache et instructions de rollback pour Ops. |
| `artifacts/sorafs/preview-descriptor.json` | Descripteur signe qui lie l'archive + checksum. |
| Sortie `probe` | Confirme que l'hote en ligne annonce le tag de release attendu. |
