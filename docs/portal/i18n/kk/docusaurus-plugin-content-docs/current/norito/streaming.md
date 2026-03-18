---
lang: kk
direction: ltr
source: docs/portal/docs/norito/streaming.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# Norito Ағын

Norito Streaming сым пішімін, басқару жақтауларын және анықтамалық кодекті анықтайды
Torii және SoraNet арқылы тірі медиа ағындары үшін пайдаланылады. Канондық спецификация өмір сүреді
`norito_streaming.md` жұмыс кеңістігінің түбірінде; бұл бет бөліктерді тазартады
операторлар мен SDK авторлары конфигурацияның сенсорлық нүктелерімен қатар қажет.

## Сым пішімі және басқару жазықтығы

- **Манифесттер және фреймдер.** `ManifestV1` және `PrivacyRoute*` сегментті сипаттайды
  уақыт шкаласы, кесінді дескрипторлары және бағыт бойынша кеңестер. Басқару жақтаулары (`KeyUpdate`,
  `ContentKeyUpdate` және каденциялық кері байланыс) манифестпен қатар өмір сүреді
  Көрермендер декодтаудан бұрын міндеттемелерді тексере алады.
- **Базалық кодек.** `BaselineEncoder`/`BaselineDecoder` монотондылықты қамтамасыз етеді
  бөлік идентификаторлары, уақыт белгісінің арифметикасы және міндеттемені тексеру. Хосттар қоңырау шалуы керек
  `EncodedSegment::verify_manifest` көрермендерге немесе релелерге қызмет көрсету алдында.
- **Мүмкіндік биттері.** Мүмкіндік туралы келіссөздер `streaming.feature_bits` жарнамалайды
  (әдепкі `0b11` = бастапқы кері байланыс + құпиялылық маршрутының провайдері) осылайша реле және
  клиенттер мүмкіндіктерді анықтаусыз сәйкестендірмей құрбыларынан бас тарта алады.

## Кілттер, люкс және каденция

- **Сәйкестік талаптары.** Ағынды басқару фреймдеріне әрқашан қол қойылады
  Ed25519. Арнайы кілттер арқылы жеткізілуі мүмкін
  `streaming.identity_public_key`/`streaming.identity_private_key`; әйтпесе
  түйін идентификациясы қайта пайдаланылады.
- **HPKE жиынтықтары.** `KeyUpdate` ең төменгі жалпы жинақты таңдайды; №1 люкс
  міндетті (`AuthPsk`, `Kyber768`, `HKDF-SHA3-256`, `ChaCha20-Poly1305`), бар
  қосымша `Kyber1024` жаңарту жолы. Люкс таңдауы мына жерде сақталады
  сеанс және әрбір жаңартуда расталады.
- **Айналу.** Баспагерлер 64МБ немесе 5 минут сайын `KeyUpdate` қолтаңбасын шығарады.
  `key_counter` қатаң түрде өсуі керек; регрессия – қиын қате.
  `ContentKeyUpdate` астына оралған топтық мазмұн кілтін таратады.
  келісілген HPKE жиынтығы және идентификатор + жарамдылық бойынша гейтс сегментінің шифрын шешу
  терезе.
- **Лездік суреттер.** `StreamingSession::snapshot_state` және
  `restore_from_snapshot` тұрақты `{session_id, key_counter, suite, sts_root,
  каденс күйі}` under `streaming.session_store_dir` (әдепкі
  `./storage/streaming`). Тасымалдау кілттері қалпына келтіру кезінде қайта алынады, сондықтан бұзылады
  сеанс құпияларын ашпаңыз.

## Орындалу уақытының конфигурациясы

- **Кілт материалы.** Арнайы кілттермен қамтамасыз етіңіз
  `streaming.identity_public_key`/`streaming.identity_private_key` (Ed25519)
  мультихэш) және қосымша Kyber материалы арқылы
  `streaming.kyber_public_key`/`streaming.kyber_secret_key`. Төртеуі де болуы керек
  әдепкі мәндерді қайта анықтау кезінде бар; `streaming.kyber_suite` қабылдайды
  `mlkem512|mlkem768|mlkem1024` (бүркеншік аттар `kyber512/768/1024`, әдепкі
  `mlkem768`).
- **Кодек қоршаулары.** Құрылым оны қоспайынша, CABAC өшірілген күйде қалады;
  жинақталған rANS `ENABLE_RANS_BUNDLES=1` талап етеді. арқылы орындау
  `streaming.codec.{entropy_mode,bundle_width,bundle_accel}` және қосымша
  `streaming.codec.rans_tables_path` теңшелетін кестелерді беру кезінде. Жинақталған
- **SoraNet маршруттары.** `streaming.soranet.*` анонимді тасымалдауды басқарады:
  `exit_multiaddr` (әдепкі `/dns/torii/udp/9443/quic`), `padding_budget_ms`
  (әдепкі 25 мс), `access_kind` (`authenticated` және `read-only`), қосымша
  `channel_salt`, `provision_spool_dir` (әдепкі
  `./storage/streaming/soranet_routes`), `provision_spool_max_bytes` (әдепкі 0,
  шектеусіз), `provision_window_segments` (әдепкі 4) және
  `provision_queue_capacity` (әдепкі 256).
- **Синхрондау қақпасы.** `streaming.sync` аудиовизуал үшін дрейфті орындауды қосады
  ағындар: `enabled`, `observe_only`, `ewma_threshold_ms` және `hard_cap_ms`
  уақыттың ауытқуы үшін сегменттер қабылданбаған кезде басқарады.

## Валидация және қондырғылар

- Канондық тип анықтамалары мен көмекшілері тұрады
  `crates/iroha_crypto/src/streaming.rs`.
- Интеграциялық қамту HPKE қол алысуын, мазмұн кілтін таратуды жүзеге асырады,
  және суреттің өмірлік циклі (`crates/iroha_crypto/tests/streaming_handshake.rs`).
  Ағынды тексеру үшін `cargo test -p iroha_crypto streaming_handshake` іске қосыңыз
  жергілікті жер беті.
- Орналасуға, қателерді өңдеуге және болашақ жаңартуларға терең бойлау үшін оқыңыз
  Репозиторий түбіріндегі `norito_streaming.md`.