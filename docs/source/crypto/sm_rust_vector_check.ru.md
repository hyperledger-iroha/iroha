---
lang: ru
direction: ltr
source: docs/source/crypto/sm_rust_vector_check.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: ce2f95b8b287c18c39232418333fbefdd300c030391be9dbfa4e29a3fd5f3e14
source_last_modified: "2026-01-03T18:07:57.109606+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

//! Примечания по проверке векторов SM2 Приложение D с использованием крейтов RustCrypto.

# SM2 Приложение D Векторная проверка (RustCrypto)

В этом пошаговом руководстве описаны шаги, которые мы использовали для проверки (и отладки) примера GM/T 0003 Приложение D с использованием крейта RustCrypto `sm2`. Канонические данные примера 1 приложения (идентификатор `ALICE123@YAHOO.COM`, сообщение `"message digest"` и опубликованное `(r, s)`) теперь записываются в `crates/iroha_crypto/tests/fixtures/sm_known_answers.toml`. OpenSSL/Tongsuo/gmssl с радостью проверяет подпись (см. `sm_vectors.md`), но `sm2 v0.13.3` RustCrypto по-прежнему отклоняет точку с `signature::Error`, поэтому четность CLI подтверждена, в то время как жгут Rust остается в ожидании исправления восходящего потока.

## Временный ящик

```bash
cargo new /tmp/sm2_verify --bin
cd /tmp/sm2_verify
```

`Cargo.toml`:

```toml
[package]
name = "sm2_verify"
version = "0.1.0"
edition = "2024"

[dependencies]
hex = "0.4"
sm2 = "0.13.3"
```

`src/main.rs`:

```rust
use hex::FromHex;
use sm2::dsa::{signature::Verifier, Signature, VerifyingKey};

fn main() {
    let distid = "ALICE123@YAHOO.COM";
    let sig_bytes = <Vec<u8>>::from_hex(
        "40f1ec59f793d9f49e09dcef49130d4194f79fb1eed2caa55bacdb49c4e755d16fc6dac32c5d5cf10c77dfb20f7c2eb667a457872fb09ec56327a67ec7deebe7",
    )
    .expect("signature hex");
    let sig_array = <[u8; 64]>::try_from(sig_bytes.as_slice()).unwrap();
    let signature = Signature::from_bytes(&sig_array).unwrap();

    let public_key = <Vec<u8>>::from_hex(
        "040ae4c7798aa0f119471bee11825be46202bb79e2a5844495e97c04ff4df2548a7c0240f88f1cd4e16352a73c17b7f16f07353e53a176d684a9fe0c6bb798e857",
    )
    .expect("public key hex");

    // This still returns Err with RustCrypto 0.13.3 – track upstream.
    let verifying_key = VerifyingKey::from_sec1_bytes(distid, &public_key).unwrap();

    verifying_key
        .verify(b"message digest", &signature)
        .expect("signature verified");
}
```

## Выводы

- Проверка на соответствие каноническому примеру 1 приложения 1 `(r, s)` в настоящее время завершается неудачей, поскольку `sm2::VerifyingKey::from_sec1_bytes` возвращает `signature::Error`; отслеживать исходную/основную причину (вероятно, из-за несоответствия параметров кривой в текущей версии крейта).
- Пакет компилируется без ошибок с помощью `sm2 v0.13.3` и станет автоматическим регрессионным тестом, как только RustCrypto (или исправленная вилка) примет пару точка/сигнатура из примера 1 приложения.
- Проверка OpenSSL/Tongsuo/gmssl прошла успешно с помощью команд `sm_vectors.md`; В LibreSSL (по умолчанию для macOS) по-прежнему отсутствует поддержка SM2/SM3, отсюда и локальный пробел.

## Следующие шаги

1. Повторите тестирование, как только `sm2` предоставит API, который принимает точку примера 1 приложения (или после того, как восходящий поток подтвердит параметры кривой), чтобы жгут проводов мог проходить локально.
2. Поддерживайте проверку работоспособности CLI (OpenSSL/Tongsuo/gmssl) в конвейерах CI, чтобы защитить канонический пример приложения до тех пор, пока не появится исправление RustCrypto.
3. Добавьте этот жгут в пакет регрессии Iroha после того, как проверки четности RustCrypto и OpenSSL пройдут успешно.