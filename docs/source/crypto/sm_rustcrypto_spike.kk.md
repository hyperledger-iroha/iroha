---
lang: kk
direction: ltr
source: docs/source/crypto/sm_rustcrypto_spike.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 1f133d9489c4bcfae2212e6c5dc098f39c3dea3e5cd42855ba76e8c9b73b4d03
source_last_modified: "2025-12-29T18:16:35.946614+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

//! RustCrypto SM интеграциясының өсуіне арналған ескертпелер.

# RustCrypto SM Spike жазбалары

## Мақсат
RustCrypto қолданбасының `sm2`, `sm3` және `sm4` жәшіктерін енгізетінін растаңыз (плюс `rfc6979`, `ccm`, ```toml
[features]
sm = ["dep:sm2", "dep:sm3", "dep:sm4", "dep:rfc6979"]

[dependencies]
sm2 = { version = "0.13", optional = true, default-features = false, features = ["std"] }
sm3 = { version = "0.5.0-rc.1", optional = true }
sm4 = { version = "0.5.1", optional = true }
rfc6979 = { version = "0.4", optional = true, default-features = false }
```) `iroha_crypto` жәшігі және мүмкіндік жалауын кеңірек жұмыс кеңістігіне жалғамас бұрын қолайлы құрастыру уақытын береді.

## Ұсынылған тәуелділік картасы

| Қорап | Ұсынылған нұсқа | Ерекшеліктер | Ескертпелер |
|-------|-------------------|----------|-------|
| `sm2` | `0.13` (RustCrypto/қолтаңбалар) | `std` | `elliptic-curve` түріне байланысты; MSRV жұмыс кеңістігіне сәйкес келетінін тексеріңіз. |
| `sm3` | `0.5.0-rc.1` (RustCrypto/хэштер) | әдепкі | API параллельдері `sha2`, бар `digest` белгілерімен біріктірілген. |
| `sm4` | `0.5.1` (RustCrypto/блок-шифрлар) | әдепкі | Шифрлық белгілермен жұмыс істейді; AEAD қаптамалары кейінірек өсуге қалдырылды. |
| `rfc6979` | `0.4` | әдепкі | Детерминирленген емес туынды үшін қайта пайдалану. |

*Нұсқалар 2024-12 жылғы ағымдағы шығарылымдарды көрсетеді; қону алдында `cargo search` арқылы растаңыз.*

## Манифест өзгерістері (жоба)

```toml
[features]
sm = ["dep:sm2", "dep:sm3", "dep:sm4", "dep:rfc6979"]

[dependencies]
sm2 = { version = "0.13", optional = true, default-features = false, features = ["std"] }
sm3 = { version = "0.5.0-rc.1", optional = true }
sm4 = { version = "0.5.1", optional = true }
rfc6979 = { version = "0.4", optional = true, default-features = false }
```

Бақылау: `iroha_crypto` (қазіргі уақытта `0.13.8`) нұсқаларына сәйкес келу үшін `elliptic-curve` түйреуіші.

## Spike бақылау тізімі
- [x] `crates/iroha_crypto/Cargo.toml` үшін қосымша тәуелділіктер мен мүмкіндіктерді қосыңыз.
- [x] Сымдарды растау үшін толтырғыш құрылымдары бар `cfg(feature = "sm")` артында `signature::sm` модулін жасаңыз.
- [x] компиляцияны растау үшін `cargo check -p iroha_crypto --features sm` іске қосыңыз; жазбаны құрастыру уақыты және жаңа тәуелділік саны (`cargo tree --features sm`).
- [x] `cargo check -p iroha_crypto --features sm --locked` арқылы тек std қалпын растаңыз; `no_std` құрастыруларына енді қолдау көрсетілмейді.
- [x] `docs/source/crypto/sm_program.md` ішіндегі файл нәтижелері (уақыттар, тәуелділік тармағының дельтасы).

## Түсіру үшін бақылаулар
- Қосымша компиляция уақыты базалық деңгейге қарсы.
- `cargo builtinsize` көмегімен екілік өлшем әсері (өлшенетін болса).
- Кез келген MSRV немесе мүмкіндік қайшылықтары (мысалы, `elliptic-curve` кіші нұсқаларымен).
- Жоғары патчтарды қажет ететін ескертулер шығарылды (қауіпті код, const-fn қақпасы).

## Күтудегі элементтер
- Жұмыс кеңістігінің тәуелділік графигін толтырмас бұрын Crypto WG мақұлдауын күтіңіз.
- Қарап шығу үшін жәшіктерді жеткізу керек пе немесе crates.io сайтына сену керек пе (айна қажет болуы мүмкін) растаңыз.
- Бақылау тізімі аяқталды деп белгілемес бұрын, `Cargo.lock` жаңартуын `sm_lock_refresh_plan.md` үшін координациялаңыз.
- Құлыптау файлы мен тәуелділік тармағын қалпына келтіру үшін мақұлдау берілгеннен кейін `scripts/sm_lock_refresh.sh` пайдаланыңыз.

## 19.01.2025 Spike Log
- Қосымша тәуелділіктер (`sm2 0.13`, `sm3 0.5.0-rc.1`, `sm4 0.5.1`, `rfc6979 0.4`) және `iroha_crypto` ішіндегі `sm` мүмкіндік жалаушалары қосылды.
- Компиляция кезінде хэштеу/блоктау шифры API интерфейстерін орындау үшін stubbed `signature::sm` модулі.
- `cargo check -p iroha_crypto --features sm --locked` енді тәуелділік графигін шешеді, бірақ `Cargo.lock` жаңарту талабымен тоқтатылады; репозиторий саясаты құлыптау файлын өңдеуге тыйым салады, сондықтан компиляцияны орындау рұқсат етілген құлыпты жаңартуды үйлестіргенше күтілмейді.## 12.02.2026 Spike журналы
- Алдыңғы құлыптау файлын блоктаушы шешілді—тәуелділіктер әлдеқашан түсірілген — сондықтан `cargo check -p iroha_crypto --features sm --locked` сәтті аяқталды (dev Mac жүйесінде суық құрастыру 7,9 секунд; қосымша қайта іске қосу 0,23 секунд).
- `cargo check -p iroha_crypto --no-default-features --features "std sm" --locked` тек `std` конфигурацияларында (`no_std` жолы қалмайды) қосымша мүмкіндікті құрастыруды растай отырып, 1,0 секундта өтеді.
- `sm` мүмкіндігі қосылған тәуелділік дельтасы 11 жәшікті ұсынады: `base64ct`, `ghash`, `opaque-debug`, `pem-rfc7468`, `pem-rfc7468`, ```toml
[features]
sm = ["dep:sm2", "dep:sm3", "dep:sm4", "dep:rfc6979"]

[dependencies]
sm2 = { version = "0.13", optional = true, default-features = false, features = ["std"] }
sm3 = { version = "0.5.0-rc.1", optional = true }
sm4 = { version = "0.5.1", optional = true }
rfc6979 = { version = "0.4", optional = true, default-features = false }
``` `primeorder`, `sm2`, `sm3`, `sm4` және `sm4-gcm`. (`rfc6979` базалық графиктің бөлігі болды.)
- Пайдаланылмаған NEON саясат көмекшілері үшін құрастыру ескертулері сақталады; өлшеуді тегістеу орындалу уақыты сол код жолдарын қайта қосқанша сол күйінде қалдырыңыз.