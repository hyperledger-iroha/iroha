---
id: account-address-status
lang: kk
direction: ltr
source: docs/portal/docs/reference/account-address-status.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
title: Account address compliance
description: Summary of the ADDR-2 fixture workflow and how SDK teams stay in sync.
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

The canonical ADDR-2 bundle (`fixtures/account/address_vectors.json`) captures
I105 and i105-default (`sora`; half/full width), multisignature, and negative fixtures.
Every SDK + Torii surface relies on the same JSON so we can detect any codec
drift before it hits production. This page mirrors the internal status brief
(`docs/source/account_address_status.md` in the root repository) so portal
readers can reference the workflow without digging through the mono-repo.

## Regenerate or verify the bundle

```bash
# Refresh the canonical fixture (writes fixtures/account/address_vectors.json)
cargo xtask address-vectors --out fixtures/account/address_vectors.json

# Fail fast if the committed file is stale
cargo xtask address-vectors --verify
```

Жалаулар:

- `--stdout` — emit the JSON to stdout for ad-hoc inspection.
- `--out <path>` — write to a different path (e.g., when diffing changes locally).
- `--verify` — жұмыс көшірмесін жаңадан жасалған мазмұнмен салыстырыңыз (мүмкін емес
  be combined with `--stdout`).

CI жұмыс процесі **Address Vector Drift** `cargo xtask address-vectors --verify` іске қосады
any time the fixture, generator, or docs change to alert reviewers immediately.

## Who consumes the fixture?

| Беттік | Валидация |
|---------|------------|
| Rust деректер үлгісі | `crates/iroha_data_model/tests/account_address_vectors.rs` |
| Torii (server) | `crates/iroha_torii/tests/account_address_vectors.rs` |
| JavaScript SDK | `javascript/iroha_js/test/address.test.js` |
| Swift SDK | `IrohaSwift/Tests/IrohaSwiftTests/AccountAddressTests.swift` |
| Android SDK | `java/iroha_android/src/test/java/org/hyperledger/iroha/android/address/AccountAddressTests.java` |

Each harness round-trips canonical bytes + I105 encodings and
Norito стиліндегі қате кодтары теріс жағдайларға арналған арматураға сәйкес келетінін тексереді.

## Автоматтандыру керек пе?

Шығару құралдары көмекші арқылы арматураны жаңартуды сценарий жасай алады
`scripts/account_fixture_helper.py`, which fetches or verifies the canonical
bundle without copy/paste steps:

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

Көмекші `--source` қайта анықтауды немесе `IROHA_ACCOUNT_FIXTURE_URL` қабылдайды
ортаның айнымалысы, сондықтан SDK CI тапсырмалары өздерінің қалаған айнасын көрсете алады.
`--metrics-out` берілгенде көмекші жазады
`account_address_fixture_check_status{target=\"…\"}` along with the canonical
SHA-256 дайджесті (`account_address_fixture_remote_info`), сондықтан Prometheus мәтіндік файлы
коллекторлары мен Grafana бақылау тақтасы `account_address_fixture_status` дәлелдей алады
every surface remains in sync. Alert whenever a target reports `0`. үшін
көп бетті автоматтандыру `ci/account_fixture_metrics.sh` қаптамасын пайдаланады
(қайталанған `--target label=path[::source]` қабылдайды) сондықтан шақыру бойынша командалар жариялай алады
түйінді экспорттаушы мәтіндік файл жинағышына арналған бір біріктірілген `.prom` файлы.

## Толық қысқаша ақпарат керек пе?

Толық ADDR-2 сәйкестік күйі (иелер, бақылау жоспары, ашық әрекет элементтері)
бойымен репозиторий ішінде `docs/source/account_address_status.md` тұрады
with the Address Structure RFC (`docs/account_structure.md`). Бұл бетті a
quick operational reminder; defer to the repo docs for in-depth guidance.