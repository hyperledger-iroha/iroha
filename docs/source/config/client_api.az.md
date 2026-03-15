---
lang: az
direction: ltr
source: docs/source/config/client_api.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: fa548ec31fe928decc5c23719472618ff97f4eb45b084f9f9084df82b96cfac6
source_last_modified: "2025-12-29T18:16:35.933651+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

## Müştəri API Konfiqurasiya Referansı

Bu sənəd Torii müştəri ilə üzbəüz konfiqurasiya düymələrini izləyir.
`iroha_config::parameters::user::Torii` vasitəsilə səthlər. Aşağıdakı bölmə
NRPC-1 üçün təqdim edilən Norito-RPC nəqliyyat nəzarətlərinə diqqət yetirir; gələcək
müştəri API parametrləri bu faylı genişləndirməlidir.

### `torii.transport.norito_rpc`

| Açar | Növ | Defolt | Təsvir |
|-----|------|---------|-------------|
| `enabled` | `bool` | `true` | İkili Norito dekodlamasına imkan verən master keçid. `false` zaman, Torii, `403 norito_rpc_disabled` ilə hər Norito-RPC sorğusunu rədd edir. |
| `stage` | `string` | `"disabled"` | Yayım səviyyəsi: `disabled`, `canary` və ya `ga`. Mərhələlər qəbul qərarlarını və `/rpc/capabilities` çıxışını idarə edir. |
| `require_mtls` | `bool` | `false` | Norito-RPC nəqli üçün mTLS tətbiq edir: `true`, Torii, mTLS marker başlığı daşımayan Norito-RPC sorğularını rədd etdikdə (məsələn, I10000X). Bayraq `/rpc/capabilities` vasitəsilə üzə çıxır ki, SDK-lar yanlış konfiqurasiya edilmiş mühitlərdə xəbərdarlıq edə bilsin. |
| `allowed_clients` | `array<string>` | `[]` | Kanarya icazə siyahısı. `stage = "canary"` olduqda, yalnız bu siyahıda olan `X-API-Token` başlığını daşıyan sorğular qəbul edilir. |

Misal konfiqurasiya:

```toml
[torii.transport.norito_rpc]
enabled = true
require_mtls = true
stage = "canary"
allowed_clients = ["alpha-canary-token", "beta-canary-token"]
```

Mərhələ semantikası:

- **diaktiv** — Norito-RPC hətta `enabled = true` olsa belə əlçatan deyil. Müştərilər
  `403 norito_rpc_disabled` qəbul edin.
- **canary** — Sorğulara birinə uyğun gələn `X-API-Token` başlığı daxil edilməlidir
  `allowed_clients`. Bütün digər sorğular `403-ü alır
  norito_rpc_canary_denied`.
- **ga** — Norito-RPC hər bir autentifikasiya edilmiş zəng edən üçün əlçatandır (şərtdən asılı olaraq
  adi tarif və pre-auth limitləri).

Operatorlar bu dəyərləri `/v2/config` vasitəsilə dinamik olaraq yeniləyə bilərlər. Hər dəyişiklik
dərhal `/rpc/capabilities`-də əks olunur, SDK-lara və müşahidə olunmağa imkan verir
canlı nəqliyyat vəziyyətini göstərmək üçün panellər.