---
lang: es
direction: ltr
source: docs/portal/docs/sns/suffix-catalog.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: d23c9d6a77942b5918b933631b890addbc0ecdfef51e9ff427a4069d2cc37902
source_last_modified: "2025-11-15T16:27:31.089720+00:00"
translation_last_reviewed: 2026-01-01
---

# Catalogo de sufijos del Sora Name Service

El roadmap de SNS rastrea cada sufijo aprobado (SN-1/SN-2). Esta pagina refleja
el catalogo fuente de verdad para que los operadores que ejecutan registrars,
gateways DNS o tooling de wallets carguen los mismos parametros sin extraer de
docs de estado.

- **Snapshot:** [`docs/examples/sns/suffix_catalog_v1.json`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/examples/sns/suffix_catalog_v1.json)
- **Consumidores:** `iroha sns policy`, kits de onboarding SNS, dashboards KPI y
  scripts de release DNS/Gateway leen el mismo bundle JSON.
- **Estados:** `active` (registros permitidos), `paused` (temporalmente limitado),
  `revoked` (anunciado pero no disponible actualmente).

## Esquema del catalogo

| Campo | Tipo | Descripcion |
|-------|------|-------------|
| `suffix` | string | Sufijo legible por humanos con punto inicial. |
| `suffix_id` | `u16` | Identificador almacenado en el ledger como `SuffixPolicyV1::suffix_id`. |
| `status` | enum | `active`, `paused` o `revoked` que describen la preparacion de lanzamiento. |
| `steward_account` | string | Cuenta responsable del stewardship (coincide con hooks de politica del registrar). |
| `fund_splitter_account` | string | Cuenta que recibe pagos antes de enrutar segun `fee_split`. |
| `payment_asset_id` | string | Activo usado para settlement (`61CtjvNd9T3THAR65GsMVHr82Bjc` para la cohorte inicial). |
| `min_term_years` / `max_term_years` | integer | Limites del termino de compra desde la politica. |
| `grace_period_days` / `redemption_period_days` | integer | Ventanas de seguridad de renovacion aplicadas por Torii. |
| `referral_cap_bps` | integer | Maximo carve-out de referidos permitido por gobernanza (basis points). |
| `reserved_labels` | array | Objetos de etiqueta protegidos por gobernanza `{label, assigned_to, release_at_ms, note}`. |
| `pricing` | array | Objetos de tier con `label_regex`, `base_price`, `auction_kind` y limites de duracion. |
| `fee_split` | object | `{treasury_bps, steward_bps, referral_max_bps, escrow_bps}` division en basis points. |
| `policy_version` | integer | Contador monotono incrementado cuando la gobernanza edita la politica. |

## Catalogo actual

| Sufijo | ID (`hex`) | Steward | Fund splitter | Estado | Activo de pago | Limite de referidos (bps) | Termino (min - max anos) | Grace / Redencion (dias) | Tiers de precio (regex -> precio base / subasta) | Etiquetas reservadas | Division de fees (T/S/R/E bps) | Version de politica |
|--------|------------|---------|---------------|--------|---------------|---------------------------|--------------------------|--------------------------|-------------------------------------------------|---------------------|-------------------------------|--------------------|
| `.sora` | `0x0001` | `i105...` | `i105...` | Activo | `61CtjvNd9T3THAR65GsMVHr82Bjc` | 500 | 1-5 | 30 / 60 | `T0: ^[a-z0-9]{3,}$ -> 120 XOR (Vickrey)` | `treasury -> i105...` | `7000 / 3000 / 1000 / 0` | 1 |
| `.nexus` | `0x0002` | `i105...` | `i105...` | Pausado | `61CtjvNd9T3THAR65GsMVHr82Bjc` | 300 | 1-3 | 15 / 30 | `T0: ^[a-z0-9]{4,}$ -> 480 XOR (Vickrey)`<br>`T1: ^[a-z]{2}$ -> 4000 XOR (Dutch floor 500)` | `treasury -> i105...`, `guardian -> i105...` | `6500 / 2500 / 800 / 200` | 2 |
| `.dao` | `0x0003` | `i105...` | `i105...` | Revocado | `61CtjvNd9T3THAR65GsMVHr82Bjc` | 0 | 1-2 | 30 / 30 | `T0: ^[a-z0-9]{3,}$ -> 60 XOR (Vickrey)` | `dao (held for future release)` | `9000 / 1000 / 0 / 0` | 0 |

## Extracto JSON

```json
{
  "version": 1,
  "generated_at": "2026-05-01T00:00:00Z",
  "suffixes": [
    {
      "suffix": ".sora",
      "suffix_id": 1,
      "status": "active",
      "fund_splitter_account": "i105...",
      "payment_asset_id": "61CtjvNd9T3THAR65GsMVHr82Bjc",
      "referral_cap_bps": 500,
      "pricing": [
        {
          "tier_id": 0,
          "label_regex": "^[a-z0-9]{3,}$",
          "base_price": {"asset_id": "61CtjvNd9T3THAR65GsMVHr82Bjc", "amount": 120},
          "auction_kind": "vickrey_commit_reveal",
          "min_duration_years": 1,
          "max_duration_years": 5
        }
      ],
      "...": "see docs/examples/sns/suffix_catalog_v1.json for the full record"
    }
  ]
}
```

## Notas de automatizacion

1. Cargar el snapshot JSON y hacer hash/firmarlo antes de distribuirlo a operadores.
2. El tooling del registrar debe exponer `suffix_id`, limites de termino y pricing
   del catalogo cuando una solicitud llegue a `/v1/sns/*`.
3. Helpers de DNS/Gateway leen metadata de etiquetas reservadas al generar templates
   GAR para que las respuestas DNS sigan alineadas con controles de gobernanza.
4. Los jobs de anexos KPI etiquetan exports de dashboards con metadata de sufijo
   para que las alertas coincidan con el estado de lanzamiento registrado aqui.
