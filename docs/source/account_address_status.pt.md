---
lang: pt
direction: ltr
source: docs/source/account_address_status.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 9c1214f4d0ad86449c0ef4b8f8cbaa38fe265bab4afcc2930cd30a57c089e6d7
source_last_modified: "2025-11-15T05:05:33.914289+00:00"
translation_last_reviewed: 2026-01-01
---

## Status de conformidade do endereco de conta (ADDR-2)

Status: Aceito 2026-03-30  
Responsaveis: Equipe de modelo de dados / Guilda de QA  
Referencia do roadmap: ADDR-2 - Dual-Format Compliance Suite

### 1. Visao geral

- Fixture: `fixtures/account/address_vectors.json` (I105 + multisig casos positivos/negativos).
- Escopo: payloads V1 deterministas cobrindo implicit-default, Local-12, Global registry e controladores multisig com taxonomia completa de erros.
- Distribuicao: compartilhado entre Rust data-model, Torii, SDKs JS/TS, Swift e Android; o CI falha se algum consumidor desvia.
- Fonte da verdade: o gerador fica em `crates/iroha_data_model/src/account/address/compliance_vectors.rs` e e exposto via `cargo xtask address-vectors`.

### 2. Regeneracao e verificacao

```bash
# Write/update the canonical fixture
cargo xtask address-vectors --out fixtures/account/address_vectors.json

# Verify the committed fixture matches the generator
cargo xtask address-vectors --verify
```

Opcoes:

- `--out <path>` - substituicao opcional ao produzir bundles ad hoc (padrao `fixtures/account/address_vectors.json`).
- `--stdout` - emite JSON para stdout em vez de gravar em disco.
- `--verify` - compara o arquivo atual com o conteudo recem gerado (falha rapido com drift; incompativel com `--stdout`).

### 3. Matriz de artefatos

| Superficie | Aplicacao | Notas |
|---------|-------------|-------|
| Rust data-model | `crates/iroha_data_model/tests/account_address_vectors.rs` | Analisa o JSON, reconstrui payloads canonicos e verifica conversoes I105/canonical + erros estruturados. |
| Torii | `crates/iroha_torii/tests/account_address_vectors.rs` | Valida codecs do lado servidor para que Torii rejeite payloads I105 malformados de forma determinista. |
| JavaScript SDK | `javascript/iroha_js/test/address.test.js` | Replica fixtures V1 (I105/fullwidth) e confirma codigos de erro estilo Norito para cada caso negativo. |
| Swift SDK | `IrohaSwift/Tests/IrohaSwiftTests/AccountAddressTests.swift` | Exercita decodificacao I105, payloads multisig e exposicao de erros em plataformas Apple. |
| Android SDK | `java/iroha_android/src/test/java/org/hyperledger/iroha/android/address/AccountAddressTests.java` | Garante que os bindings Kotlin/Java fiquem alinhados ao fixture canonico. |

### 4. Monitoramento e pendencias

- Relatorio de status: este documento e referenciado em `status.md` e no roadmap para que revisoes semanais verifiquem a saude do fixture.
- Resumo no portal de desenvolvedores: veja **Reference -> Account address compliance** no portal de docs (`docs/portal/docs/reference/account-address-status.md`).
- Prometheus e dashboards: quando voce verificar uma copia do SDK, execute o helper com `--metrics-out` (e opcionalmente `--metrics-label`) para que o textfile collector do Prometheus ingira `account_address_fixture_check_status{target=...}`. O dashboard Grafana **Account Address Fixture Status** (`dashboards/grafana/account_address_fixture_status.json`) mostra contagens pass/fail por superficie e expoe o digest SHA-256 canonico como evidencia de auditoria. Alertar quando qualquer target reportar `0`.
- Torii metrics: `torii_address_domain_total{endpoint,domain_kind}` agora emite para cada account literal parseado com exito, espelhando `torii_address_invalid_total`/`torii_address_local8_total`. Alertar sobre trafego `domain_kind="local12"` em producao e espelhar os contadores no dashboard SRE `address_ingest` para que a aposentadoria do Local-12 tenha evidencia auditavel.
- Fixture helper: `scripts/account_fixture_helper.py` baixa ou verifica o JSON canonico para que a automacao de releases do SDK possa obter/verificar o bundle sem copiar/colar manual, enquanto opcionalmente escreve metricas do Prometheus. Exemplo:

  ```bash
  # Write the latest fixture to a custom path (defaults to fixtures/account/address_vectors.json)
  python3 scripts/account_fixture_helper.py fetch --output path/to/sdk/address_vectors.json

  # Fail if an SDK copy drifts from the canonical remote (accepts file:// or HTTPS sources)
  python3 scripts/account_fixture_helper.py check --target path/to/sdk/address_vectors.json --quiet

  # Emit Prometheus textfile metrics for dashboards/alerts (writes remote/local digests as labels)
  python3 scripts/account_fixture_helper.py check \
    --target path/to/sdk/address_vectors.json \
    --metrics-out /var/lib/node_exporter/textfile_collector/address_fixture.prom \
    --metrics-label android
  ```

  O helper escreve `account_address_fixture_check_status{target="android"} 1` quando o target coincide, alem dos gauges `account_address_fixture_remote_info` / `account_address_fixture_local_info` que expoem digests SHA-256. Arquivos ausentes reportam `account_address_fixture_local_missing`.
  Automation wrapper: chame `ci/account_fixture_metrics.sh` a partir de cron/CI para emitir um textfile consolidado (padrao `artifacts/account_fixture/address_fixture.prom`). Passe entradas repetidas de `--target label=path` (opcionalmente adicione `::https://mirror/...` por target para sobrescrever a fonte) para que o Prometheus raspe um unico arquivo cobrindo cada copia SDK/CLI. O workflow do GitHub `address-vectors-verify.yml` ja executa este helper contra o fixture canonico e envia o artefato `account-address-fixture-metrics` para ingestao SRE.
