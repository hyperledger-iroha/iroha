---
lang: ru
direction: ltr
source: docs/source/crypto/attachments/sm_openssl_provenance.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 95a34657b6064f925995a7e9f20145d14fda681f4af1f182418b9f624047e576
source_last_modified: "2026-01-03T18:07:57.073612+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

% SM OpenSSL/Tongsuo Снимок происхождения
% Создано: 30 января 2026 г.

# Сводка по окружающей среде

- И18НИ00000000Х: И18НИ00000001Х
- `openssl version -a`: сообщает `LibreSSL 3.3.6` (системный набор инструментов TLS в macOS).
- `cargo tree -p iroha_crypto --features "sm sm-ffi-openssl"`: см. `sm_iroha_crypto_tree.txt` для получения точного стека зависимостей Rust (крейт `openssl` v0.10.74, `openssl-sys` v0.9.x, исходные коды OpenSSL 3.x, доступные через крейт `openssl-src`; функция `vendored` включен в `crates/iroha_crypto/Cargo.toml` для детерминированных предварительных сборок).

# Примечания

- Ссылки локальной среды разработки на заголовки/библиотеки LibreSSL; промышленные предварительные сборки должны использовать OpenSSL >= 3.0.0 или Tongsuo 8.x. Замените системный набор инструментов или установите `OPENSSL_DIR`/`PKG_CONFIG_PATH` при создании окончательного пакета артефактов.
- Восстановите этот снимок в среде сборки выпуска, чтобы захватить точный хэш архива OpenSSL/Tongsuo (`openssl version -v`, `openssl version -b`, `openssl version -f`) и прикрепите воспроизводимый сценарий сборки/контрольную сумму. Для сборок поставщиков запишите версию/фиксацию крейта `openssl-src`, используемую Cargo (видна в `target/debug/build/openssl-sys-*/output`).
- Хосты Apple Silicon требуют `RUSTFLAGS=-Aunsafe-code` при запуске дымовой системы OpenSSL, чтобы компилировались заглушки ускорения AArch64 SM3/SM4 (встроенные функции недоступны в macOS). Сценарий `scripts/sm_openssl_smoke.sh` экспортирует этот флаг перед вызовом `cargo`, чтобы обеспечить согласованность CI и локальных запусков.
- Прикрепите исходный источник (например, `openssl-src-<ver>.tar.gz` SHA256) после закрепления конвейера упаковки; используйте тот же хэш в артефактах CI.