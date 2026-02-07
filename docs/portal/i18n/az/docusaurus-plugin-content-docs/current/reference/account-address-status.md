---
id: account-address-status
lang: az
direction: ltr
source: docs/portal/docs/reference/account-address-status.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
title: Account address compliance
description: Summary of the ADDR-2 fixture workflow and how SDK teams stay in sync.
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

Kanonik ADDR-2 paketi (`fixtures/account/address_vectors.json`) çəkir
IH58 (üstünlük verilir), sıxılmış (`sora`, ikinci ən yaxşı; yarım/tam eni), çox imzalı və mənfi qurğular.
Hər SDK + Torii səthi eyni JSON-a əsaslanır ki, biz istənilən kodek aşkarlaya bilək
istehsala çatmazdan qabaq sürünür. Bu səhifə daxili status brifinqini əks etdirir
(kök deposunda `docs/source/account_address_status.md`) belə portal
oxucular mono-repo qazmadan iş prosesinə istinad edə bilərlər.

## Paketi bərpa edin və ya yoxlayın

```bash
# Refresh the canonical fixture (writes fixtures/account/address_vectors.json)
cargo xtask address-vectors --out fixtures/account/address_vectors.json

# Fail fast if the committed file is stale
cargo xtask address-vectors --verify
```

Bayraqlar:

- `--stdout` — ad-hoc yoxlama üçün JSON-u stdout-a buraxın.
- `--out <path>` — fərqli yola yazın (məsələn, yerli dəyişiklikləri fərqləndirərkən).
- `--verify` — işçi nüsxəni təzə yaradılmış məzmunla müqayisə edin (ola bilməz
  `--stdout` ilə birləşdirilə bilər).

CI iş axını **Ünvan Vektor Drifti** `cargo xtask address-vectors --verify` ilə işləyir
armatur, generator və ya sənədlər dərhal rəyçiləri xəbərdar etmək üçün dəyişdikdə.

## Aparatı kim istehlak edir?

| Səthi | Doğrulama |
|---------|------------|
| Rust data-model | `crates/iroha_data_model/tests/account_address_vectors.rs` |
| Torii (server) | `crates/iroha_torii/tests/account_address_vectors.rs` |
| JavaScript SDK | `javascript/iroha_js/test/address.test.js` |
| Swift SDK | `IrohaSwift/Tests/IrohaSwiftTests/AccountAddressTests.swift` |
| Android SDK | `java/iroha_android/src/test/java/org/hyperledger/iroha/android/address/AccountAddressTests.java` |

Hər bir gediş-gəliş kanonik bayt + IH58 + sıxılmış (`sora`, ikinci ən yaxşı) kodlaşdırma və
Norito tipli xəta kodlarının neqativ hallar üçün fikstürə uyğun olduğunu yoxlayır.

## Avtomatlaşdırma lazımdır?

Buraxılış alətləri köməkçi ilə fiksasiya yeniləmələrini yaza bilər
`scripts/account_fixture_helper.py`, kanonikləri əldə edir və ya təsdiqləyir
kopyalama/yapışdırma addımları olmadan paket:

```bash
# Download to a custom path (defaults to fixtures/account/address_vectors.json)
python3 scripts/account_fixture_helper.py fetch --output path/to/sdk/address_vectors.json

# Verify that a local copy matches the canonical source (HTTPS or file://)
python3 scripts/account_fixture_helper.py check --target path/to/sdk/address_vectors.json --quiet

# Emit Prometheus textfile metrics for dashboards/alerts
python3 scripts/account_fixture_helper.py check \
  --target path/to/sdk/address_vectors.json \
  --metrics-out /var/lib/node_exporter/textfile_collector/address_fixture.prom \
  --metrics-label android
```

Köməkçi `--source` və ya `IROHA_ACCOUNT_FIXTURE_URL`-i ləğv edir
mühit dəyişəni, beləliklə SDK CI işləri onların üstünlük verdiyi güzgüyə işarə edə bilər.
`--metrics-out` verildikdə köməkçi yazır
Kanonik ilə birlikdə `account_address_fixture_check_status{target=\"…\"}`
SHA-256 həzm (`account_address_fixture_remote_info`) belə ki, Prometheus mətn faylı
kollektorlar və Grafana tablosuna `account_address_fixture_status` sübut edə bilər
hər səth sinxron qalır. Hədəf `0` məlumat verdikdə xəbərdar olun. üçün
çox səthli avtomatlaşdırma `ci/account_fixture_metrics.sh` sarğı istifadə edin
(təkrarlanan `--target label=path[::source]` qəbul edir) beləliklə, çağırış komandaları dərc edə bilər
node-exporter mətn faylı kollektoru üçün birləşdirilmiş `.prom` faylı.

## Tam qısa məlumat lazımdır?

Tam ADDR-2 uyğunluq statusu (sahiblər, monitorinq planı, açıq fəaliyyət elementləri)
boyunca depo daxilində `docs/source/account_address_status.md`-də yaşayır
Ünvan Strukturu RFC (`docs/account_structure.md`) ilə. Bu səhifəni a kimi istifadə edin
sürətli əməliyyat xatırlatma; ətraflı təlimat üçün repo sənədlərinə müraciət edin.