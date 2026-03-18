---
lang: ru
direction: ltr
source: docs/dependency_audit.md
status: complete
translator: manual
source_hash: 9746f44dbe6c09433ead16647429ad48bba54ecf9c3271e71fad6cb91a212d65
source_last_modified: "2025-11-02T04:40:28.811390+00:00"
translation_last_reviewed: 2025-11-14
---

<!-- Русский перевод docs/dependency_audit.md (Dependency Audit Summary) -->

//! Сводка аудита зависимостей

Дата: 2025‑09‑01

Область: проверка всего workspace — всех crates, объявленных в Cargo.toml и
зафиксированных в Cargo.lock. Использовался `cargo-audit` с базой RustSec
вместе с ручной проверкой легитимности crates и выбора “основных crates” для
алгоритмов.

Инструменты/команды:
- `cargo tree -d --workspace --locked --offline` – просмотр дублирующихся
  версий.
- `cargo audit` – сканирование Cargo.lock на наличие известных
  уязвимостей и yanked‑crates.

Найденные security‑advisories (сейчас 0 vuln; 2 предупреждения):
- crossbeam-channel — RUSTSEC‑2025‑0024  
  - Исправлено: обновление до `0.5.15` в `crates/ivm/Cargo.toml`.

- устаревший codec в pprof — RUSTSEC‑2024‑0437  
  - Исправлено: переключение `pprof` на `prost-codec` в
    `crates/iroha_torii/Cargo.toml`.

- ring — RUSTSEC‑2025‑0009  
  - Исправлено: обновлена QUIC/TLS‑стек (`quinn 0.11`, `rustls 0.23`,
    `tokio-rustls 0.26`) и WebSocket‑стек (`tungstenite/tokio-tungstenite 0.24`).
    Лок `ring` принудительно зафиксирован на `0.17.12` с помощью
    `cargo update -p ring --precise 0.17.12`.

Оставшиеся advisories: нет. Оставшиеся предупреждения: `backoff` (больше не
поддерживается), `derivative` (больше не поддерживается).

Оценка легитимности и “основных crates” (ключевые моменты):
- Хеширование: `sha2` (RustCrypto), `blake2` (RustCrypto), `tiny-keccak`
  (широко используется) — каноничные варианты.
- AEAD/симметричное шифрование: `aes-gcm`, `chacha20poly1305`, traits `aead`
  (RustCrypto) — каноничные.
- Подписи/ECC: `ed25519-dalek`, `x25519-dalek` (проект dalek), `k256`
  (RustCrypto), `secp256k1` (биндинги libsecp) — все легитимны; желательно
  выбрать одну основную реализацию secp256k1 (`k256` на чистом Rust или
  `secp256k1` поверх libsecp), чтобы уменьшить поверхность.
- BLS12‑381/ZK: `blstrs`, семейство `halo2_*` — распространённые библиотеки в
  production‑ZK‑экосистемах; легитимны.
- PQ: `pqcrypto-dilithium`, `pqcrypto-traits` — легитимные референсные crates.
- TLS: `rustls`, `tokio-rustls`, `hyper-rustls` — современный каноничный TLS‑стек
  в Rust.
- Noise: `snow` — каноничная реализация протокола Noise.
- Сериализация: `parity-scale-codec` — каноничный codec для SCALE. Serde
  удалён из production‑зависимостей по всему workspace; derive’ы и writer’ы
  Norito покрывают все runtime‑пути. Оставшиеся упоминания Serde есть только в
  исторической документации, защитных скриптах или test‑only‑allowlist’ах.
- FFI/библиотеки: `libsodium-sys-stable`, `openssl` — легитимны; в
  production‑пути рекомендуется использовать Rustls вместо OpenSSL (как уже
  делает текущий код).
- pprof 0.13.0 (crates.io) — upstream‑фикс влит; использовать официальный
  релиз с `prost-codec` + frame‑pointer’ом, чтобы избежать устаревшего codec’а.

Рекомендации:
- Разобраться с предупреждениями:
  - Рассмотреть замену `backoff` на `retry`/`futures-retry` или локальный
    helper для экспоненциального backoff’а.
  - Заменить derive’ы `derivative` на ручные impl’ы или `derive_more`, где
    это уместно.
- Среднесрочно: по возможности унифицировать использование `k256` или
  `secp256k1`, чтобы уменьшить количество дублирующихся реализаций
  (оставлять обе только при реальной необходимости).
- Среднесрочно: проверить происхождение `poseidon-primitives 0.2.0` в
  контексте ZK‑использования; по возможности перейти на Poseidon‑реализацию,
  нативную для Arkworks/Halo2, чтобы избежать параллельных экосистем.

Заметки:
- `cargo tree -d` показывает ожидаемые дубли major‑версий (`bitflags` 1/2,
  несколько `ring`); само по себе это не является угрозой безопасности, но
  увеличивает поверхность сборки.
- Typosquat‑пакетов не обнаружено; все названия и источники соответствуют
  известным crates экосистемы или внутренним workspace‑модулям.
- Экспериментально: в `iroha_crypto` добавлен feature
  `bls-backend-blstrs` для начала миграции BLS на backend, основанный только
  на `blstrs` (убирает зависимость от arkworks при активной feature). По
  умолчанию остаётся `w3f-bls`, чтобы не менять поведение/кодировки. План
  выравнивания:
  - Нормализовать сериализацию секретного ключа к каноничному 32‑байтному
    little‑endian выходу, который понимают и `w3f-bls`, и `blstrs`
    (`Scalar::to_bytes_le`), отказавшись от легаси‑helper’а со смешанным
    endianness.
  - Добавить явный wrapper для компрессии публичного ключа, использующий
    `blstrs::G1Affine::to_compressed`, и ввести проверку совместимости с
    w3f‑кодировкой, чтобы гарантировать идентичные wire‑bytes.
  - Добавить fixtures для round‑trip’а в
    `crates/iroha_crypto/tests/bls_backend_compat.rs`, где ключи
    генерируются один раз и сверяются между backend’ами для `SecretKey`,
    `PublicKey` и агрегированных подписей.
  - Защитить новый backend feature‑флагом `bls-backend-blstrs` в CI, при этом
    сохраняя тесты совместимости активными и для backend’а по умолчанию,
    чтобы улавливать регрессии до переключения.

Дальнейшие действия (предлагаемые задачи):
- Сохранить Serde‑guardrails в CI
  (`scripts/check_no_direct_serde.sh`, `scripts/deny_serde_json.sh`), чтобы
  не допустить появления новых production‑зависимостей от Serde.

Тестирование, выполненное в рамках аудита:
- 
