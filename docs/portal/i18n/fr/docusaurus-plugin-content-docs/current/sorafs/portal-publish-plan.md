---
id: portal-publish-plan
lang: fr
direction: ltr
source: docs/portal/docs/sorafs/portal-publish-plan.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
---

:::note Source canonique
Reflète `docs/source/sorafs/portal_publish_plan.md`. Mettez a jour les deux copies lorsque le workflow change.
:::

L'item de roadmap DOCS-7 exige que chaque artefact docs (build du portail, spec OpenAPI,
SBOMs) passe par le pipeline de manifests SoraFS et soit servi via `docs.sora` avec les
headers `Sora-Proof`. Cette checklist assemble les helpers existants pour que Docs/DevRel,
Storage et Ops puissent effectuer la release sans devoir parcourir plusieurs runbooks.

## 1. Build et packaging des payloads

Lancez le helper de packaging (des options de skip existent pour les dry-runs) :

```bash
./ci/package_docs_portal_sorafs.sh \
  --out artifacts/devportal/sorafs/$(date -u +%Y%m%dT%H%M%SZ) \
  --sign \
  --sigstore-provider=github-actions \
  --sigstore-audience=sorafs-devportal \
  --proof
```

- `--skip-build` reutilise `docs/portal/build` si CI l'a deja produit.
- Ajoutez `--skip-sbom` lorsque `syft` n'est pas disponible (ex. repetition air-gapped).
- Le script lance les tests du portail, emet des paires CAR + manifest pour `portal`,
  `openapi`, `portal-sbom` et `openapi-sbom`, verifie chaque CAR lorsque `--proof`
  est active, et depose des bundles Sigstore lorsque `--sign` est active.
- Structure de sortie :

```json
{
  "generated_at": "2026-02-19T13:00:12Z",
  "output_dir": "artifacts/devportal/sorafs/20260219T130012Z",
  "artifacts": [
    {
      "name": "portal",
      "car": ".../portal.car",
      "plan": ".../portal.plan.json",
      "car_summary": ".../portal.car.json",
      "manifest": ".../portal.manifest.to",
      "manifest_json": ".../portal.manifest.json",
      "proof": ".../portal.proof.json",
      "bundle": ".../portal.manifest.bundle.json",
      "signature": ".../portal.manifest.sig"
    }
  ]
}
```

Conservez tout le dossier (ou un symlink via `artifacts/devportal/sorafs/latest`) pour
que les relecteurs de gouvernance puissent tracer les artefacts de build.

## 2. Pin des manifests et aliases

Utilisez `sorafs_cli manifest submit` pour pousser les manifests vers Torii et lier les aliases.
Definissez `${SUBMITTED_EPOCH}` sur l'epoch de consensus la plus recente (depuis
`curl -s "${TORII_URL}/v2/status" | jq '.sumeragi.epoch'` ou votre dashboard).

```bash
OUT="artifacts/devportal/sorafs/20260219T130012Z"
TORII_URL="https://torii.stg.sora.net/"
AUTHORITY="i105..."
KEY_FILE="secrets/docs-admin.key"
ALIAS_PROOF="secrets/docs.alias.proof"
SUBMITTED_EPOCH="$(curl -s ${TORII_URL}/v2/status | jq '.sumeragi.epoch')"

cargo run -p sorafs_orchestrator --bin sorafs_cli -- \
  manifest submit \
  --manifest="${OUT}/portal.manifest.to" \
  --chunk-plan="${OUT}/portal.plan.json" \
  --torii-url="${TORII_URL}" \
  --submitted-epoch="${SUBMITTED_EPOCH}" \
  --authority="${AUTHORITY}" \
  --private-key-file "${KEY_FILE}" \
  --alias-namespace docs \
  --alias-name portal \
  --alias-proof "${ALIAS_PROOF}" \
  --summary-out "${OUT}/portal.manifest.submit.json" \
  --response-out "${OUT}/portal.manifest.response.json"
```

- Repetez pour `openapi.manifest.to` et les manifests SBOM (omettre les flags alias pour les
  bundles SBOM sauf si la gouvernance assigne un namespace).
- Alternative : `iroha app sorafs pin register` fonctionne avec le digest du resume de submit si
  le binaire est deja installe.
- Verifiez l'etat du registry avec
  `iroha app sorafs pin list --alias docs:portal --format json | jq`.
- Dashboards a surveiller : `sorafs_pin_registry.json` (metriques `torii_sorafs_replication_*`).

## 3. Headers et proofs de gateway

Generez le bloc d'entetes HTTP + la metadata de binding :

```bash
iroha app sorafs gateway route-plan \
  --manifest-json "${OUT}/portal.manifest.json" \
  --hostname docs.sora \
  --alias docs:portal \
  --route-label docs-portal-20260219 \
  --proof-status ok \
  --headers-out "${OUT}/portal.gateway.headers.txt" \
  --out "${OUT}/portal.gateway.plan.json"
```

- Le template inclut les headers `Sora-Name`, `Sora-CID`, `Sora-Proof`,
  `Sora-Proof-Status` plus CSP/HSTS/Permissions-Policy par defaut.
- Utilisez `--rollback-manifest-json` pour generer un set d'entetes de rollback.

Avant d'exposer le trafic, executez :

```bash
./ci/check_sorafs_gateway_probe.sh -- \
  --gateway "https://docs.sora/.well-known/sorafs/manifest" \
  --report-json artifacts/sorafs_gateway_probe/docs.json

scripts/sorafs_gateway_self_cert.sh \
  --manifest "${OUT}/portal.manifest.json" \
  --headers "${OUT}/portal.gateway.headers.txt" \
  --output artifacts/sorafs_gateway_self_cert/docs
```

- Le probe exige la fraicheur des signatures GAR, la politique d'alias et les empreintes TLS.
- Le harness self-cert telecharge le manifest via `sorafs_fetch` et stocke les logs de replay CAR ;
  conservez les sorties pour l'evidence d'audit.

## 4. Garde-fous DNS et telemetrie

1. Rafraichissez le squelette DNS afin que la gouvernance puisse prouver le binding :

   ```bash
   scripts/sns_zonefile_skeleton.py \
     --manifest "${OUT}/portal.manifest.json" \
     --out artifacts/sorafs/portal.dns-cutover.json
   ```

2. Surveillez pendant le rollout :

   - `torii_sorafs_alias_cache_refresh_total`
   - `torii_sorafs_gateway_refusals_total{profile="docs"}`
   - `torii_sorafs_fetch_duration_ms` / `_failures_total`

   Dashboards : `sorafs_gateway_observability.json`,
   `sorafs_fetch_observability.json` et le board pin registry.

3. Faites un smoke des regles d'alertes (`scripts/telemetry/test_sorafs_fetch_alerts.sh`) et
   capturez logs/captures d'ecran pour l'archive release.

## 5. Bundle de preuves

Incluez les elements suivants dans le ticket de release ou le paquet de gouvernance :

- `artifacts/devportal/sorafs/<stamp>/` (CARs, manifests, SBOMs, proofs,
  bundles Sigstore, resumés de submit).
- Sorties gateway probe + self-cert
  (`artifacts/sorafs_gateway_probe/<stamp>/`,
  `artifacts/sorafs_gateway_self_cert/<stamp>/`).
- Squelette DNS + templates d'entetes (`portal.gateway.headers.txt`,
  `portal.gateway.plan.json`, `portal.dns-cutover.json`).
- Captures de dashboard + accusés d'alertes.
- Mise a jour `status.md` referencant le digest du manifest et l'heure de
  binding de l'alias.

Suivre cette checklist delivre DOCS-7 : les payloads portail/OpenAPI/SBOM sont
packagés de facon deterministe, pins avec des aliases, proteges par des headers
`Sora-Proof` et surveilles end-to-end via la stack d'observabilite existante.
