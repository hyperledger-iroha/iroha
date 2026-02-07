---
lang: ru
direction: ltr
source: docs/source/crypto/dependency_audits.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 04e4cf26ed0ce9f9782be8aae9d16425a7a87fdbd1986cbcbca68a27ba0a3afe
source_last_modified: "2026-01-03T18:07:57.038859+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# Аудит криптозависимости

## Стрибог (ящик `streebog`)

- **Версия в дереве:** `0.11.0-rc.2` поставляется под номером `vendor/streebog` (используется, когда функция `gost` включена).
- **Потребитель:** `crates/iroha_crypto::signature::gost` (HMAC-Streebog DRBG + хеширование сообщений).
- **Статус:** Только кандидат на выпуск. Ни один крейт, не являющийся RC, в настоящее время не предлагает требуемую поверхность API.
  поэтому мы зеркалируем крейт в дереве для возможности аудита, одновременно отслеживая исходную версию финальной версии.
- **Просмотр контрольных точек:**
  - Проверка хэш-вывода с помощью пакета Wycheproof и приборов TC26 через
    `cargo test -p iroha_crypto --features gost` (см. `crates/iroha_crypto/tests/gost_wycheproof.rs`).
  - `cargo bench -p iroha_crypto --bench gost_sign --features gost`
    Ed25519/Secp256k1 проверяется рядом с каждой кривой TC26 с текущей зависимостью.
  - `cargo run -p iroha_crypto --bin gost_perf_check --features gost`
    сравнивает более свежие измерения с зарегистрированными медианами (используйте `--summary-only` в CI, добавьте
    `--write-baseline crates/iroha_crypto/benches/gost_perf_baseline.json` при перебазировке).
  - `scripts/gost_bench.sh` оборачивает бенч+проверяет расход; передайте `--write-baseline`, чтобы обновить JSON.
    См. `docs/source/crypto/gost_performance.md` для сквозного рабочего процесса.
- **Устранения последствий:** `streebog` вызывается только через детерминированные оболочки, обнуляющие ключи;
  подписывающая сторона хеджирует одноразовые номера с помощью энтропии ОС, чтобы избежать катастрофического сбоя ГСЧ.
- **Дальнейшие действия:** Следите за выпуском RustCrypto streebog `0.11.x`; как только метка приземлится, обработайте
  обновление как стандартное улучшение зависимостей (проверка контрольной суммы, просмотр различий, запись происхождения и
  уроните продаваемое зеркало).