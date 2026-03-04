---
lang: fr
direction: ltr
source: docs/portal/docs/devportal/preview-host-exposure.ru.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# Руководство по экспозиции aperçu-хоста

La carte DOCS-SORA nécessite que l'aperçu public soit utilisé dans l'ensemble, la somme de contrôle vérifiée et la version vérifiée localement. Utilisez ce runbook après avoir intégré les rapports (et la mise en service) pour que vous puissiez utiliser l'hôte d'aperçu bêta dans cet ensemble.

## Travaux préliminaires

- L'intégration des rapports sur les enregistrements et les mises à jour dans le tracker de prévisualisation.
- L'image du portail est ensuite établie dans `docs/portal/build/` et la somme de contrôle est vérifiée (`build/checksums.sha256`).
- Aperçu de l'aperçu SoraFS (URL Torii, autorité, clé privée, époque d'ouverture) dans la configuration temporaire ou la configuration JSON, par exemple [`docs/examples/sorafs_preview_publish.json`](../../../examples/sorafs_preview_publish.json).
- Ouvrir le ticket pour la configuration DNS avec le nom d'hôte (`docs-preview.sora.link`, `docs.iroha.tech` et t.d.) et les contacts d'astreinte.

## Partie 1 - Ajouter et valider le bundle

```bash
cd docs/portal
export DOCS_RELEASE_TAG="preview-$(date -u +%Y%m%dT%H%M%SZ)"
npm ci
npm run build
./scripts/preview_verify.sh --build-dir build
```

Les preuves de script permettent de produire, si la somme de contrôle du manifeste est établie ou effectuée, qui permet d'auditer tous les éléments d'aperçu.

## Partie 2 - Ajouter les objets d'art SoraFS

Преобразуйте статический сайт в детерминированную пару CAR/manifest. `ARTIFACT_DIR` pour remplacer `docs/portal/artifacts/`.

```bash
./scripts/sorafs-pin-release.sh       --alias docs-preview.sora       --alias-namespace docs       --alias-name preview       --pin-label docs-preview       --skip-submit

node scripts/generate-preview-descriptor.mjs       --manifest artifacts/checksums.sha256       --archive artifacts/sorafs/portal.tar.gz       --out artifacts/sorafs/preview-descriptor.json
```

Utilisez `portal.car`, `portal.manifest.*`, le descripteur et la somme de contrôle du manifeste pour la vague d'aperçu.

## Partie 3 - Afficher l'alias d'aperçuSi vous utilisez Pin Helper ** sans ** `--skip-submit`, vous devrez ouvrir les fichiers de l'hôte. Avant de configurer la configuration JSON ou d'utiliser les indicateurs CLI :

```bash
./scripts/sorafs-pin-release.sh       --alias docs-preview.sora       --alias-namespace docs       --alias-name preview       --pin-label docs-preview       --config ~/secrets/sorafs_preview_publish.json
```

La commande recherche `portal.pin.report.json`, `portal.manifest.submit.summary.json` et `portal.submit.response.json`, qui doivent être téléchargés dans le paquet de preuves.

## Partie 4 - Planifier le basculement DNS

```bash
node scripts/generate-dns-cutover-plan.mjs       --dns-hostname docs.iroha.tech       --dns-zone sora.link       --dns-change-ticket DOCS-SORA-Preview       --dns-cutover-window "2026-03-05 18:00Z"       --dns-ops-contact "pagerduty:sre-docs"       --manifest artifacts/sorafs/portal.manifest.to       --cache-purge-endpoint https://cache.api/purge       --cache-purge-auth-env CACHE_PURGE_TOKEN       --out artifacts/sorafs/portal.dns-cutover.json
```

Si vous souhaitez utiliser JSON avec Ops, vous devez configurer la confidentialité DNS dans votre manifeste de résumé. Si vous utilisez le descripteur précédent pour la restauration historique, utilisez `--previous-dns-plan path/to/previous.json`.

## Partie 5 - Vérifier votre hôte

```bash
npm run probe:portal --       --base-url=https://docs-preview.sora.link       --expect-release="$DOCS_RELEASE_TAG"
```

La sonde fournit des balises de version, des fichiers CSP et des métadonnées. Utilisez une commande dans deux régions (ou utilisez curl) pour que les auditeurs se connectent au programme Edge Cache.

## Lot de preuves

Cliquez sur les objets d'art suivants dans la vague d'aperçu du ticket et sélectionnez-les dans les détails :| Artefact | Назначение |
|--------------|------------|
| `build/checksums.sha256` | Доказывает, что bundle соответствует CI build. |
| `artifacts/sorafs/portal.tar.gz` + `portal.manifest.to` | Charge utile Canon SoraFS + manifeste. |
| `portal.pin.report.json`, `portal.manifest.submit.summary.json`, `portal.submit.response.json` | Il s'agit de l'ouverture du manifeste et de la confidentialité de l'alias. |
| `artifacts/sorafs/portal.dns-cutover.json` | DNS métadonnées (étiquette, bouton, contacts), le marché de production (`Sora-Route-Binding`), l'emplacement `route_plan` (plan JSON + en-tête de blocs), le numéro de téléphone purge du cache et instructions de restauration pour Ops. |
| `artifacts/sorafs/preview-descriptor.json` | Descripteur complet, archive complète + somme de contrôle. |
| Nom `probe` | Il est à noter que l'hôte en direct publie la balise de version. |