---
lang: es
direction: ltr
source: docs/source/sorafs/gar_enforcement_receipts.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 5fc9902196b64a9e4928178a618e75833a25ec1cace9817cac58c4f699bb114f
source_last_modified: "2025-11-02T04:40:40.177754+00:00"
translation_last_reviewed: "2026-01-30"
---

# Receipts de enforcement GAR

El item del roadmap **SNNet-15G1 — GAR enforcement receipts & audits** requiere
que cada accion de gateway (purge, freeze, cambio de rate-limit, legal hold, …)
emita un artefacto determinista para que auditores puedan rastrear decisiones de
politica sin re-ejecutar logs. El tipo
`iroha_data_model::sorafs::gar::GarEnforcementReceiptV1` captura ese artefacto y
ahora es el payload canonico intercambiado entre CDN, automatizacion DNS y
tooling de governance.

## Overview del schema

| Campo | Descripcion |
|-------|-------------|
| `receipt_id` | Identificador de 16 bytes (ULID/guid) que correlaciona el receipt con telemetria y runbooks. |
| `gar_name` | Label SoraDNS registrado (p. ej., `docs.sora`). |
| `canonical_host` | Host exacto afectado por la accion (p. ej., `docs.gateway.sora.net`). |
| `action` | Enum `GarEnforcementActionV1` que describe la operacion (`purge_static_zone`, `ttl_override`, `rate_limit_override`, `geo_fence`, `legal_hold`, `moderation`, `audit_notice` o un slug `custom` provisto por el caller). |
| `triggered_at_unix` | Timestamp Unix (segundos) cuando comenzo el enforcement. |
| `expires_at_unix` | Timestamp opcional de expiracion para acciones temporales. |
| `policy_version` | Label opcional de release/manifiesto (p. ej., `2026-q2`). |
| `policy_digest` | Digest BLAKE3 opcional del blob exacto de politica. |
| `operator` | Account ID del guardian/operador que ejecuto el cambio. |
| `reason` | Razon legible para humanos expuesta en dashboards/runbooks. |
| `notes` | Nota opcional libre para investigaciones posteriores. |
| `evidence_uris` | Referencias a artefactos de soporte (logs, manifiestos CAR, dashboards). |
| `labels` | Tags legibles por maquina (ticket guardian, slug de incidente, nombre de drill, etc.). |

Todos los campos opcionales default a `None`/vectores vacios para que los
receipts se emitan incluso cuando un GAR no tenga digest publicado.

## Ejemplo de payload JSON / Norito

```json
{
  "receipt_id": "30313233343536373839616263646566",
  "gar_name": "docs.sora",
  "canonical_host": "docs.gateway.sora.net",
  "action": { "kind": "rate_limit_override" },
  "triggered_at_unix": 1747483200,
  "expires_at_unix": 1747569600,
  "policy_version": "2026-q2",
  "policy_digest": "abababababababababababababababababababababababababababababababab",
  "operator": "i105...",
  "reason": "Guardian freeze window",
  "notes": "Escalated during SNNet-15 drill",
  "evidence_uris": [
    "sora://gar/receipts/docs/0123",
    "https://ops.sora.net/incidents/SN15-0001"
  ],
  "labels": [
    "guardian-freeze",
    "sn15-drill"
  ]
}
```

## Uso

- La automatizacion CDN/DNS emite un `GarEnforcementReceiptV1` cada vez que un
  operador aplica una accion de politica GAR. El receipt se persiste on-chain o
  se archiva en el bundle de evidencia de governance.
- Observabilidad exporta los receipts via los dashboards SNS existentes para que
  auditores filtren por `labels`, `gar_name` u `operator`.
- Helpers CLI/SDK de governance consumen el mismo tipo Norito, asegurando que
  bundles de evidencia, reportes de transparencia y tests GAR automatizados
  sigan en sync.

### Helper CLI (`sorafs gar receipt`)

Generar artefactos JSON + Norito directo desde el CLI:

```bash
cargo run --bin iroha -- sorafs gar receipt \
  --gar-name docs.sora \
  --canonical-host docs.gateway.sora.net \
  --action rate-limit-override \
  --operator i105... \
  --reason "Guardian freeze window" \
  --policy-version 2026-q2 \
  --policy-digest abab...abab \
  --label guardian-freeze \
  --label sn15-drill \
  --evidence-uri sora://gar/receipts/docs/0123 \
  --evidence-uri https://ops.sora.net/incidents/SN15-0001 \
  --json-out artifacts/gar/docs_receipt.json \
  --norito-out artifacts/gar/docs_receipt.to
```

Los flags aceptan RFC 3339 (`--triggered-at 2026-05-10T10:15:00Z`) o timestamps
`@unix`, slugs personalizados de accion GAR
(`--action custom --custom-action-slug purge-l7`) y digests de politica
opcionales. Cuando `--json-out` / `--norito-out` se omiten, el comando imprime
el payload JSON a stdout mientras permite a callers pipear los bytes Norito a
herramientas downstream.

Ver `crates/iroha_data_model/src/sorafs/gar.rs` para la definicion canonica
Norito, y el checklist SNNet-15G1 en `roadmap.md` para requisitos de
automatizacion relacionados.

### Helper de export de auditoria (`cargo xtask soranet-gar-export`)

Bundlear receipts y archivos ACK en un solo reporte JSON/Markdown para paquetes
de governance:

```bash
cargo xtask soranet-gar-export \
  --pop soranet-pop-m0 \
  --json-out artifacts/soranet/gateway/soranet-pop-m0/gar_receipts_summary.json \
  --markdown-out artifacts/soranet/gateway/soranet-pop-m0/gar_receipts_summary.md
```

Flags:
- `--pop <label>` selecciona defaults bajo `artifacts/soranet/gateway/<pop>/`
  (`gar_receipts/`, `gar_acks/`, y rutas de resumen) cuando no se proveen paths
  explicitos.
- `--receipts-dir` / `--acks-dir` overridean los roots de receipt/ack (se aceptan
  receipts JSON o Norito).
- `--json-out <path|->` escribe el resumen combinado (stdout cuando `-`).
- `--markdown-out <path>` emite una tabla ligera para revision humana.
- `--now <unix>` computa edad de receipts y ack lag en el resumen (opcional).
