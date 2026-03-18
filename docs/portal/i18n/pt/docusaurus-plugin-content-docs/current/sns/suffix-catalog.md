---
id: suffix-catalog
lang: pt
direction: ltr
source: docs/portal/docs/sns/suffix-catalog.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
---

# Catalogo de sufixos do Sora Name Service

O roadmap do SNS acompanha cada sufixo aprovado (SN-1/SN-2). Esta pagina espelha
o catalogo fonte da verdade para que operadores que executam registrars, DNS
gateways ou tooling de wallets carreguem os mesmos parametros sem raspar docs de
status.

- **Snapshot:** [`docs/examples/sns/suffix_catalog_v1.json`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/examples/sns/suffix_catalog_v1.json)
- **Consumidores:** `iroha sns policy`, kits de onboarding SNS, dashboards KPI e
  scripts de release DNS/Gateway leem o mesmo bundle JSON.
- **Status:** `active` (registros permitidos), `paused` (temporariamente bloqueado),
  `revoked` (anunciado mas nao disponivel no momento).

## Esquema do catalogo

| Campo | Tipo | Descricao |
|-------|------|-----------|
| `suffix` | string | Sufixo legivel por humanos com ponto inicial. |
| `suffix_id` | `u16` | Identificador armazenado no ledger em `SuffixPolicyV1::suffix_id`. |
| `status` | enum | `active`, `paused` ou `revoked` descrevendo a prontidao de lancamento. |
| `steward_account` | string | Conta responsavel pela stewardship (corresponde aos hooks de politica do registrar). |
| `fund_splitter_account` | string | Conta que recebe pagamentos antes do roteamento conforme `fee_split`. |
| `payment_asset_id` | string | Ativo usado para settlement (`xor#sora` para a coorte inicial). |
| `min_term_years` / `max_term_years` | integer | Limites de termo de compra da politica. |
| `grace_period_days` / `redemption_period_days` | integer | Janelas de seguranca de renovacao aplicadas pelo Torii. |
| `referral_cap_bps` | integer | Carve-out maximo de referral permitido pela governanca (basis points). |
| `reserved_labels` | array | Objetos de label protegidos por governanca `{label, assigned_to, release_at_ms, note}`. |
| `pricing` | array | Objetos de tier com `label_regex`, `base_price`, `auction_kind` e limites de duracao. |
| `fee_split` | object | `{treasury_bps, steward_bps, referral_max_bps, escrow_bps}` divisao em basis points. |
| `policy_version` | integer | Contador monotono incrementado quando a governanca edita a politica. |

## Catalogo atual

| Sufixo | ID (`hex`) | Steward | Fund splitter | Status | Ativo de pagamento | Limite de referral (bps) | Termo (min - max anos) | Grace / Redemption (dias) | Tiers de preco (regex -> preco base / leilao) | Labels reservados | Divisao de fees (T/S/R/E bps) | Versao de politica |
|--------|------------|---------|---------------|--------|-------------------|--------------------------|------------------------|---------------------------|-----------------------------------------------|------------------|------------------------------|-------------------|
| `.sora` | `0x0001` | `i105...` | `i105...` | Ativo | `xor#sora` | 500 | 1-5 | 30 / 60 | `T0: ^[a-z0-9]{3,}$ -> 120 XOR (Vickrey)` | `treasury -> i105...` | `7000 / 3000 / 1000 / 0` | 1 |
| `.nexus` | `0x0002` | `i105...` | `i105...` | Pausado | `xor#sora` | 300 | 1-3 | 15 / 30 | `T0: ^[a-z0-9]{4,}$ -> 480 XOR (Vickrey)`<br>`T1: ^[a-z]{2}$ -> 4000 XOR (Dutch floor 500)` | `treasury -> i105...`, `guardian -> i105...` | `6500 / 2500 / 800 / 200` | 2 |
| `.dao` | `0x0003` | `i105...` | `i105...` | Revogado | `xor#sora` | 0 | 1-2 | 30 / 30 | `T0: ^[a-z0-9]{3,}$ -> 60 XOR (Vickrey)` | `dao (held for future release)` | `9000 / 1000 / 0 / 0` | 0 |

## Trecho JSON

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
      "payment_asset_id": "xor#sora",
      "referral_cap_bps": 500,
      "pricing": [
        {
          "tier_id": 0,
          "label_regex": "^[a-z0-9]{3,}$",
          "base_price": {"asset_id": "xor#sora", "amount": 120},
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

## Notas de automacao

1. Carregar o snapshot JSON e fazer hash/assinar antes de distribuir aos operadores.
2. Tooling do registrar deve expor `suffix_id`, limites de termo e precos do
   catalogo quando uma requisicao atingir `/v1/sns/*`.
3. Helpers DNS/Gateway leem os metadados de labels reservados ao gerar templates
   GAR para que respostas DNS continuem alinhadas aos controles de governanca.
4. Jobs de anexos KPI marcam exports de dashboards com metadados de sufixo para que
   alertas correspondam ao estado de lancamento registrado aqui.
