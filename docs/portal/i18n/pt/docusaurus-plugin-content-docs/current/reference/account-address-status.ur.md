---
lang: pt
direction: ltr
source: docs/portal/docs/reference/account-address-status.ur.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: status do endereço da conta
título: اکاؤنٹ ایڈریس تعمیل
description: Fixação ADDR-2
---

pacote ADDR-2 canônico (`fixtures/account/address_vectors.json`) IH58 (preferencial), compactado (`sora`, segundo melhor; meia largura / largura total), multiassinatura, اور negativo fixtures کو captura کرتا ہے۔ ہر SDK + Torii superfície اسی JSON پر انحصار کرتی ہے تاکہ codec drift پروڈکشن تک پہنچنے سے پہلے پکڑا جا سکے۔ یہ صفحہ اندرونی status brief (`docs/source/account_address_status.md` ریپوزٹری روٹ میں) کو espelho کرتا ہے تاکہ leitores de portal بغیر mono-repo میں کھوج لگائے fluxo de trabalho

## Pacote کو regenerar یا verificar کریں

```bash
# Refresh the canonical fixture (writes fixtures/account/address_vectors.json)
cargo xtask address-vectors --out fixtures/account/address_vectors.json

# Fail fast if the committed file is stale
cargo xtask address-vectors --verify
```

Bandeiras:

- `--stdout` — inspeção ad-hoc کیلئے JSON کو stdout پر emit کرتا ہے۔
- `--out <path>` — Caminho do caminho para o valor do arquivo (مثلا لوکل diff کے وقت)۔
- `--verify` — cópia de trabalho کو تازہ gerar شدہ conteúdo کے ساتھ comparar کرتا ہے (`--stdout` کے ساتھ combinar نہیں ہو سکتا)۔

Fluxo de trabalho de CI **Desvio de vetor de endereço** `cargo xtask address-vectors --verify`
ہر بار چلاتا ہے جب fixture, gerador, یا docs بدلیں تاکہ revisores کو فوراً alerta کیا جا سکے۔

## Fixture کون استعمال کرتا ہے؟

| Superfície | Validação |
|--------|------------|
| Modelo de dados Rust | `crates/iroha_data_model/tests/account_address_vectors.rs` |
| Torii (servidor) | `crates/iroha_torii/tests/account_address_vectors.rs` |
| SDK JavaScript | `javascript/iroha_js/test/address.test.js` |
| SDK rápido | `IrohaSwift/Tests/IrohaSwiftTests/AccountAddressTests.swift` |
| SDK Android | `java/iroha_android/src/test/java/org/hyperledger/iroha/android/address/AccountAddressTests.java` |

ہر aproveitar bytes canônicos + IH58 + codificações compactadas (`sora`, segunda melhor) کا ida e volta کرتا ہے اور casos negativos کیلئے códigos de erro estilo Norito کو fixture کے ساتھ correspondência کرتا ہے۔

## Automação چاہئے؟

Liberar atualizações de acessórios de ferramentas کو `scripts/account_fixture_helper.py` کے ذریعے script کر سکتی ہے، جو copiar/colar کے بغیر busca de pacote canônico یا verificar کرتا ہے:

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

O auxiliar `--source` substitui a variável de ambiente `IROHA_ACCOUNT_FIXTURE_URL`. سکیں۔ O `--metrics-out` é um resumo canônico SHA-256 (`account_address_fixture_remote_info`) que ajuda o `account_address_fixture_check_status{target="…"}`. Prometheus textfile collectors اور Grafana dashboard `account_address_fixture_status` ہر surface کی sync ثابت کر سکیں۔ جب کوئی alvo `0` رپورٹ کرے تو alerta کریں۔ automação multi-superfície کیلئے wrapper `ci/account_fixture_metrics.sh` استعمال کریں (repetível `--target label=path[::source]` قبول کرتا ہے) تاکہ coletor de arquivo de texto do exportador de nó de equipes de plantão کیلئے ایک consolidado `.prom` فائل publicar کر سکیں۔

## مکمل breve چاہئے؟

Status de conformidade ADDR-2 (proprietários, plano de monitoramento, itens de ação abertos) کی مکمل تفصیل
RFC de estrutura de endereço (`docs/account_structure.md`) RFC de estrutura de endereço (`docs/account_structure.md`) اس صفحہ کو lembrete operacional rápido کے طور پر استعمال کریں؛ تفصیلی رہنمائی کیلئے repo docs دیکھیں۔