---
lang: pt
direction: ltr
source: docs/portal/docs/reference/account-address-status.ar.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: status do endereço da conta
título: امتثال عنوان الحساب
description: ملخص سير عمل fixture ADDR-2 وكيف تبقى فرق SDK متزامنة.
---

O modelo ADDR-2 (`fixtures/account/address_vectors.json`) possui fixtures IH58 e compactados (`sora`, segundo melhor; meia/largura total) e multiassinatura e negativo. Você pode usar o SDK + Torii para obter o JSON que você precisa e o codec no qual você está trabalhando. Faça o download do cartão de crédito (`docs/source/account_address_status.md` no site da empresa) Você pode usar o mono-repo para fazer isso.

## اعادة توليد او التحقق من الحزمة

```bash
# Refresh the canonical fixture (writes fixtures/account/address_vectors.json)
cargo xtask address-vectors --out fixtures/account/address_vectors.json

# Fail fast if the committed file is stale
cargo xtask address-vectors --verify
```

Bandeiras:

- `--stdout` — Insira JSON no stdout do arquivo.
- `--out <path>` — é um código de erro (não importa o valor do arquivo).
- `--verify` — Não há nenhum problema com o `--stdout`.

Use CI **Address Vector Drift** como `cargo xtask address-vectors --verify`
Não há nenhum dispositivo elétrico e um dispositivo de fixação para o carro.

## من يستهلك fixture؟

| السطح | التحقق |
|--------|------------|
| Modelo de dados Rust | `crates/iroha_data_model/tests/account_address_vectors.rs` |
| Torii (servidor) | `crates/iroha_torii/tests/account_address_vectors.rs` |
| SDK JavaScript | `javascript/iroha_js/test/address.test.js` |
| SDK rápido | `IrohaSwift/Tests/IrohaSwiftTests/AccountAddressTests.swift` |
| SDK Android | `java/iroha_android/src/test/java/org/hyperledger/iroha/android/address/AccountAddressTests.java` |

كل arnês يجري ida e volta للبايتات القياسية + IH58 + الترميزات المضغوطة ويتحقق من ان اكواد الخطأ بنمط Norito تطابق fixture للحالات السلبية.

## هل تحتاج الى اتمتة؟

يمكن لادوات الاصدار برمجة تحديثات fixture عبر المساعد
`scripts/account_fixture_helper.py`;

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

Substituir o `--source` e substituir o `IROHA_ACCOUNT_FIXTURE_URL` por meio do SDK do CI الاشارة الى المرآة المفضلة. Você pode usar `--metrics-out` para `account_address_fixture_check_status{target="…"}` com resumo do SHA-256 (`account_address_fixture_remote_info`) para coletar coletores de arquivos de texto Prometheus e Grafana `account_address_fixture_status` podem ser usados para evitar problemas. O alvo é `0`. Para obter mais informações sobre o `ci/account_fixture_metrics.sh` (não use o `--target label=path[::source]`) O arquivo de texto `.prom` é um arquivo de texto do node-exporter.

## هل تحتاج الملخص الكامل؟

حالة الامتثال الكاملة لـ ADDR-2 (proprietários وخطة المراقبة وبنود العمل المفتوحة)
O endereço `docs/source/account_address_status.md` é baseado na estrutura de endereço RFC (`docs/account_structure.md`). استخدم هذه الصفحة كتذكير تشغيلي سريع؛ وارجع الى وثائق المستودع للارشاد المفصل.