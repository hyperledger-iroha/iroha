---
lang: fr
direction: ltr
source: docs/portal/docs/devportal/preview-host-exposure.es.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# Guide d'exposition de l'hôte de prévisualisation

La feuille de route DOCS-SORA exige que chaque aperçu public utilise le même bundle vérifié par la somme de contrôle que les réviseurs vérifient localement. Utilisez ce runbook après avoir complété l'intégration des réviseurs (et le ticket d'approbation des invitations) pour mettre en ligne l'hôte de la version bêta.

## Conditions préalables

- Ola de onboarding des réviseurs approuvés et enregistrés dans le tracker de prévisualisation.
- Ultimo build del portal présenté en `docs/portal/build/` et somme de contrôle vérifiée (`build/checksums.sha256`).
- Informations d'identification d'aperçu SoraFS (URL de Torii, autorité, clé privée, époque envoyée) enregistrées dans les variables d'origine ou dans une configuration JSON comme [`docs/examples/sorafs_preview_publish.json`] (../../../examples/sorafs_preview_publish.json).
- Ticket de changement DNS ouvert avec le nom d'hôte souhaité (`docs-preview.sora.link`, `docs.iroha.tech`, etc.) avec des contacts d'astreinte.

## Étape 1 - Construire et vérifier le bundle

```bash
cd docs/portal
export DOCS_RELEASE_TAG="preview-$(date -u +%Y%m%dT%H%M%SZ)"
npm ci
npm run build
./scripts/preview_verify.sh --build-dir build
```

Le script de vérification ne continue pas lorsque le manifeste de somme de contrôle échoue ou est manipulé, en gardant audité chaque artefact de prévisualisation.

## Paso 2 - Empaqueter les artefacts SoraFS

Convertir le site statique en un par CAR/manifeste déterministe. `ARTIFACT_DIR` par défaut est `docs/portal/artifacts/`.

```bash
./scripts/sorafs-pin-release.sh       --alias docs-preview.sora       --alias-namespace docs       --alias-name preview       --pin-label docs-preview       --skip-submit

node scripts/generate-preview-descriptor.mjs       --manifest artifacts/checksums.sha256       --archive artifacts/sorafs/portal.tar.gz       --out artifacts/sorafs/preview-descriptor.json
```

Complément `portal.car`, `portal.manifest.*`, le descripteur et le manifeste de somme de contrôle pour le ticket de l'aperçu.## Paso 3 - Publier l'alias de l'aperçu

Répétez l'assistant de broche **sin** `--skip-submit` lorsque cette liste est pour exposer l'hôte. Proportionner la configuration JSON ou les indicateurs CLI explicites :

```bash
./scripts/sorafs-pin-release.sh       --alias docs-preview.sora       --alias-namespace docs       --alias-name preview       --pin-label docs-preview       --config ~/secrets/sorafs_preview_publish.json
```

Le commandant décrit `portal.pin.report.json`, `portal.manifest.submit.summary.json` et `portal.submit.response.json`, qui doivent voyager avec le paquet de preuves d'invitation.

## Étape 4 - Générer le plan de route DNS

```bash
node scripts/generate-dns-cutover-plan.mjs       --dns-hostname docs.iroha.tech       --dns-zone sora.link       --dns-change-ticket DOCS-SORA-Preview       --dns-cutover-window "2026-03-05 18:00Z"       --dns-ops-contact "pagerduty:sre-docs"       --manifest artifacts/sorafs/portal.manifest.to       --cache-purge-endpoint https://cache.api/purge       --cache-purge-auth-env CACHE_PURGE_TOKEN       --out artifacts/sorafs/portal.dns-cutover.json
```

Comparez le JSON résultant avec Ops pour que le changement DNS référence le résumé du manifeste exact. Lorsque cela réutilise un descripteur antérieur comme source de restauration, il s'appelle `--previous-dns-plan path/to/previous.json`.

## Paso 5 - Probar el host desplegado

```bash
npm run probe:portal --       --base-url=https://docs-preview.sora.link       --expect-release="$DOCS_RELEASE_TAG"
```

La sonde confirme l'étiquette du service de publication, les en-têtes CSP et les métadonnées de l'entreprise. Répétez la commande des deux régions (ou ajoutez la sortie de boucle) pour que les auditeurs veillent à ce que le cache du bord soit chaud.

## Bundle de preuves

Incluez les objets suivants dans le billet de l'ola de prévisualisation et les références dans l'e-mail d'invitation :| Artefact | Proposé |
|--------------|---------------|
| `build/checksums.sha256` | Il est évident que le bundle coïncide avec la build de CI. |
| `artifacts/sorafs/portal.tar.gz` + `portal.manifest.to` | Charge utile canonique SoraFS + manifeste. |
| `portal.pin.report.json`, `portal.manifest.submit.summary.json`, `portal.submit.response.json` | Il faut que l'envoi du manifeste + la liaison de l'alias soient complets. |
| `artifacts/sorafs/portal.dns-cutover.json` | Métadonnées DNS (ticket, vente, contacts), résumé de la promotion de route (`Sora-Route-Binding`), le pointeur `route_plan` (plan JSON + étiquettes d'en-tête), informations de purge du cache et instructions de restauration pour les opérations. |
| `artifacts/sorafs/preview-descriptor.json` | Descripteur confirmé qui ajoute l'archive + somme de contrôle. |
| Sortie de `probe` | Confirmez que l'hôte annonce en vivo le tag de sortie attendu. |