---
id: portal-publish-plan
lang: pt
direction: ltr
source: docs/portal/docs/sorafs/portal-publish-plan.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
---

:::note Fonte canonica
Espelha `docs/source/sorafs/portal_publish_plan.md`. Atualize ambas as copias quando o workflow mudar.
:::

O item de roadmap DOCS-7 exige que todo artefato de docs (build do portal, spec OpenAPI,
SBOMs) passe pelo pipeline de manifests do SoraFS e seja servido via `docs.sora` com headers
`Sora-Proof`. Esta checklist conecta os helpers existentes para que Docs/DevRel, Storage e Ops
possam executar a release sem procurar em varios runbooks.

## 1. Build e empacotamento de payloads

Execute o helper de empacotamento (existem opcoes de skip para dry-runs):

```bash
./ci/package_docs_portal_sorafs.sh \
  --out artifacts/devportal/sorafs/$(date -u +%Y%m%dT%H%M%SZ) \
  --sign \
  --sigstore-provider=github-actions \
  --sigstore-audience=sorafs-devportal \
  --proof
```

- `--skip-build` reutiliza `docs/portal/build` se o CI ja produziu.
- Adicione `--skip-sbom` quando `syft` nao estiver disponivel (por exemplo, ensaio air-gapped).
- O script executa os testes do portal, gera pares CAR + manifest para `portal`,
  `openapi`, `portal-sbom` e `openapi-sbom`, verifica cada CAR quando `--proof`
  esta ativado, e grava bundles Sigstore quando `--sign` esta ativado.
- Estrutura de saida:

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

Mantenha toda a pasta (ou um symlink via `artifacts/devportal/sorafs/latest`) para que
os revisores de governanca possam rastrear os artefatos de build.

## 2. Pin de manifests e aliases

Use `sorafs_cli manifest submit` para enviar manifests ao Torii e vincular aliases.
Defina `${SUBMITTED_EPOCH}` como a epoca de consenso mais recente (de
`curl -s "${TORII_URL}/v1/status" | jq '.sumeragi.epoch'` ou seu dashboard).

```bash
OUT="artifacts/devportal/sorafs/20260219T130012Z"
TORII_URL="https://torii.stg.sora.net/"
AUTHORITY="soraカタカナ..."
KEY_FILE="secrets/docs-admin.key"
ALIAS_PROOF="secrets/docs.alias.proof"
SUBMITTED_EPOCH="$(curl -s ${TORII_URL}/v1/status | jq '.sumeragi.epoch')"

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

- Repita para `openapi.manifest.to` e os manifests de SBOM (omita flags de alias para bundles SBOM
  a menos que a governanca atribua um namespace).
- Alternativa: `iroha app sorafs pin register` funciona com o digest do resumo de submit se o
  binario ja estiver instalado.
- Verifique o estado do registry com
  `iroha app sorafs pin list --alias docs:portal --format json | jq`.
- Dashboards para observar: `sorafs_pin_registry.json` (metricas `torii_sorafs_replication_*`).

## 3. Headers e proofs do gateway

Gere o bloco de headers HTTP + metadata de binding:

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

- O template inclui headers `Sora-Name`, `Sora-CID`, `Sora-Proof`,
  `Sora-Proof-Status` mais a CSP/HSTS/Permissions-Policy padrao.
- Use `--rollback-manifest-json` para gerar um conjunto de headers de rollback.

Antes de expor o trafego, execute:

```bash
./ci/check_sorafs_gateway_probe.sh -- \
  --gateway "https://docs.sora/.well-known/sorafs/manifest" \
  --report-json artifacts/sorafs_gateway_probe/docs.json

scripts/sorafs_gateway_self_cert.sh \
  --manifest "${OUT}/portal.manifest.json" \
  --headers "${OUT}/portal.gateway.headers.txt" \
  --output artifacts/sorafs_gateway_self_cert/docs
```

- O probe exige frescor de assinatura GAR, politica de alias e impressao digital TLS.
- O harness self-cert baixa o manifest com `sorafs_fetch` e guarda logs de replay do CAR;
  mantenha os outputs para evidencia de auditoria.

## 4. Guardrails de DNS e telemetria

1. Atualize o esqueleto DNS para que a governanca possa provar o binding:

   ```bash
   scripts/sns_zonefile_skeleton.py \
     --manifest "${OUT}/portal.manifest.json" \
     --out artifacts/sorafs/portal.dns-cutover.json
   ```

2. Monitore durante o rollout:

   - `torii_sorafs_alias_cache_refresh_total`
   - `torii_sorafs_gateway_refusals_total{profile="docs"}`
   - `torii_sorafs_fetch_duration_ms` / `_failures_total`

   Dashboards: `sorafs_gateway_observability.json`,
   `sorafs_fetch_observability.json` e o board do pin registry.

3. Faca smoke das regras de alerta (`scripts/telemetry/test_sorafs_fetch_alerts.sh`) e
   capture logs/screenshots para o arquivo de release.

## 5. Bundle de evidencias

Inclua o seguinte no ticket de release ou pacote de governanca:

- `artifacts/devportal/sorafs/<stamp>/` (CARs, manifests, SBOMs, proofs,
  bundles Sigstore, resumos de submit).
- Outputs do gateway probe + self-cert
  (`artifacts/sorafs_gateway_probe/<stamp>/`,
  `artifacts/sorafs_gateway_self_cert/<stamp>/`).
- Esqueleto DNS + templates de headers (`portal.gateway.headers.txt`,
  `portal.gateway.plan.json`, `portal.dns-cutover.json`).
- Screenshots de dashboard + acknowledgements de alertas.
- Atualizacao de `status.md` referenciando o digest do manifest e o horario de
  binding do alias.

Seguir esta checklist entrega DOCS-7: os payloads do portal/OpenAPI/SBOM sao
empacotados de forma deterministica, fixados com aliases, protegidos por headers
`Sora-Proof` e monitorados end-to-end pela stack de observabilidade existente.
