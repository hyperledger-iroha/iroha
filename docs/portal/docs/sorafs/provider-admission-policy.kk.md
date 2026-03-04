---
lang: kk
direction: ltr
source: docs/portal/docs/sorafs/provider-admission-policy.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 17fcb22d5be25f601d4096c3a3488b7be2dd92dcf27019b678634590cd3bdde4
source_last_modified: "2025-12-29T18:16:35.197199+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

> [`docs/source/sorafs/provider_admission_policy.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sorafs/provider_admission_policy.md) нұсқасынан бейімделген.

№ SoraFS Провайдерді қабылдау және жеке басын куәландыру саясаты (SF-2b жобасы)

Бұл жазба **SF-2b** үшін орындалатын нәтижелерді қамтиды: анықтау және
қабылдау жұмыс процесін, жеке басын куәландыратын талаптарды және аттестаттауды қамтамасыз ету
SoraFS сақтау провайдерлеріне арналған пайдалы жүктемелер. Ол жоғары деңгейдегі процесті кеңейтеді
SoraFS Architecture RFC-де сипатталған және қалған жұмысты келесіге бөледі
бақыланатын инженерлік тапсырмалар.

## Саясаттың мақсаттары

- Тек тексерілген операторлар `ProviderAdvertV1` жазбаларын жариялай алатынына көз жеткізіңіз.
  желі қабылдайды.
- Әрбір жарнама кілтін үкімет бекіткен жеке басын куәландыратын құжатқа байланыстырыңыз,
  расталған соңғы нүктелер және ең аз үлес қосу.
- Torii, шлюздер және детерминирленген тексеру құралдарын қамтамасыз етіңіз.
  `sorafs-node` бірдей тексерулерді орындайды.
- Детерминизмді бұзбай жаңартуды және төтенше жағдайды жоюды қолдау
  құралдың эргономикасы.

## Сәйкестік және ставка талаптары

| Талап | Сипаттама | Жеткізу мүмкіндігі |
|-------------|-------------|-------------|
| Жарнаманың негізгі шығу тегі | Провайдерлер әрбір жарнамаға қол қоятын Ed25519 пернелер жұбын тіркеуі керек. Қабылдау жинағы басқару қолтаңбасымен бірге ашық кілтті сақтайды. | `ProviderAdmissionProposalV1` схемасын `advert_key` (32 байт) арқылы кеңейтіп, оған тізілімнен (`sorafs_manifest::provider_admission`) сілтеме жасаңыз. |
| Үлес көрсеткіші | Қабылдау үшін белсенді стекинг пулын көрсететін нөлден басқа `StakePointer` қажет. | `sorafs_manifest::provider_advert::StakePointer::validate()` ішінде валидацияны және CLI/тесттерде беттік қателерді қосыңыз. |
| Юрисдикция тегтері | Провайдерлер юрисдикцияны жариялайды + заңды байланыс. | Ұсыныс схемасын `jurisdiction_code` (ISO 3166-1 альфа-2) және қосымша `contact_uri` арқылы кеңейтіңіз. |
| Соңғы нүктені аттестаттау | Әрбір жарнамаланған соңғы нүкте mTLS немесе QUIC сертификатының есебімен қамтамасыз етілуі керек. | `EndpointAttestationV1` Norito пайдалы жүктемесін анықтаңыз және қабылдау жинағындағы соңғы нүктеге сақтаңыз. |

## Қабылдау жұмыс процесі

1. **Ұсынысты құру**
   - CLI: `cargo run -p sorafs_manifest --bin sorafs_manifest_stub -- provider-admission proposal …` қосыңыз
     `ProviderAdmissionProposalV1` + аттестаттау бумасын шығару.
   - Тексеру: талап етілетін өрістерді қамтамасыз етіңіз, ставка > 0, `profile_id` ішіндегі канондық chunker дескрипті.
2. **Басқаруды растау**
   - Кеңес бар `blake3("sorafs-provider-admission-v1" || canonical_bytes)` қол қояды
     конверт құралдары (`sorafs_manifest::governance` модулі).
   - Конверт `governance/providers/<provider_id>/admission.json` дейін сақталады.
3. **Тізілімді енгізу**
   - Ортақ тексерушіні енгізу (`sorafs_manifest::provider_admission::validate_envelope`)
     Torii/шлюздар/CLI қайта пайдалану.
   - Дайджест немесе жарамдылық мерзімі конверттен өзгеше болатын жарнамаларды қабылдамау үшін Torii қабылдау жолын жаңартыңыз.
4. **Жаңарту және күшін жою**
   - Қосымша соңғы нүкте/қатысу жаңартулары бар `ProviderAdmissionRenewalV1` қосыңыз.
   - Қайтару себебін жазатын және басқару оқиғасын итеретін `--revoke` CLI жолын көрсетіңіз.

## Іске асыру міндеттері

| Аудан | Тапсырма | Ие(лер) | Күй |
|------|------|----------|--------|
| Схема | `crates/sorafs_manifest/src/provider_admission.rs` астында `ProviderAdmissionProposalV1`, `ProviderAdmissionEnvelopeV1`, `EndpointAttestationV1` (Norito) анықтаңыз. Валидация көмекшілерімен `sorafs_manifest::provider_admission` жүйесінде жүзеге асырылды.【F:crates/sorafs_manifest/src/provider_admission.rs#L1】 | Сақтау / Басқару | ✅ Аяқталды |
| CLI құралдары | `sorafs_manifest_stub` ішкі пәрмендерімен кеңейтіңіз: `provider-admission proposal`, `provider-admission sign`, `provider-admission verify`. | Құралдар WG | ✅ |

CLI ағыны енді аралық сертификат бумаларын (`--endpoint-attestation-intermediate`) қабылдайды,
канондық ұсыныс/конверт байттары және `sign`/`verify` кезінде кеңес қолдарын тексереді. Операторлар жасай алады
тікелей жарнама органдарын беріңіз немесе қол қойылған жарнамаларды қайта пайдаланыңыз және қолтаңба файлдары жұптастыру арқылы жеткізілуі мүмкін
`--council-signature-public-key` `--council-signature-file` көмегімен автоматтандырудың ыңғайлылығы үшін.

### CLI анықтамасы

Әрбір пәрменді `cargo run -p sorafs_manifest --bin sorafs_manifest_stub -- provider-admission …` арқылы іске қосыңыз.

- `proposal`
  - Қажетті жалаушалар: `--provider-id=<hex32>`, `--chunker-profile=<namespace.name@semver>`,
    `--stake-pool-id=<hex32>`, `--stake-amount=<amount>`, `--advert-key=<hex32>`,
    `--jurisdiction-code=<ISO3166-1>` және кем дегенде бір `--endpoint=<kind:host>`.
  - Соңғы нүктеге арналған аттестация `--endpoint-attestation-attested-at=<secs>` күтеді,
    `--endpoint-attestation-expires-at=<secs>`, арқылы сертификат
    `--endpoint-attestation-leaf=<path>` (плюс қосымша `--endpoint-attestation-intermediate=<path>`
    әрбір тізбек элементі үшін) және кез келген келісілген ALPN идентификаторлары
    (`--endpoint-attestation-alpn=<token>`). QUIC соңғы нүктелері тасымалдау есептерін бере алады
    `--endpoint-attestation-report[-hex]=…`.
  - Шығару: канондық Norito ұсыныс байттары (`--proposal-out`) және JSON қорытындысы
    (әдепкі stdout немесе `--json-out`).
- `sign`
  - Енгізулер: ұсыныс (`--proposal`), қол қойылған хабарландыру (`--advert`), қосымша хабарландыру нысаны
    (`--advert-body`), сақтау дәуірі және кем дегенде бір кеңес қолы. Қол қоюға болады
    кірістірілген (`--council-signature=<signer_hex:signature_hex>`) немесе біріктіру арқылы файлдар арқылы
    `--council-signature-public-key` `--council-signature-file=<path>` бар.
  - Дайджест байланыстарын көрсететін расталған конверт (`--envelope-out`) және JSON есебін жасайды,
    қол қоюшылар саны және енгізу жолдары.
- `verify`
  - Қосымша сәйкес ұсынысты тексере отырып, бар конвертті (`--envelope`) растайды,
    хабарландыру немесе жарнама органы. JSON есебі дайджест мәндерін, қолтаңбаны тексеру күйін,
    және қандай қосымша артефактілер сәйкес келеді.
- `renewal`
  - Жаңадан бекітілген конвертті бұрын ратификацияланған дайджестпен байланыстырады. талап етеді
    `--previous-envelope=<path>` және мұрагері `--envelope=<path>` (екеуі де Norito пайдалы жүктемелері).
    CLI профиль бүркеншік аттары, мүмкіндіктер және жарнама кілттері болған кезде өзгеріссіз қалатынын тексереді
    үлесті, соңғы нүктелерді және метадеректерді жаңартуға мүмкіндік береді. Канондықты шығарады
    `ProviderAdmissionRenewalV1` байт (`--renewal-out`) және JSON қорытындысы.
- `revoke`
  - Конверті қажет провайдер үшін `ProviderAdmissionRevocationV1` шұғыл бумасын шығарады
    қайтарып алу. `--envelope=<path>`, `--reason=<text>`, кем дегенде біреуі қажет
    `--council-signature`, және қосымша `--revoked-at`/`--notes`. CLI қол қояды және растайды
    қайтарып алу дайджесті, Norito пайдалы жүктемесін `--revocation-out` арқылы жазады және JSON есебін басып шығарады
    дайджест пен қолтаңбаларды жинау.
| Тексеру | Torii, шлюздер және `sorafs-node` пайдаланатын ортақ растаушыны іске қосыңыз. Бірлік + CLI интеграция сынақтарын қамтамасыз етіңіз.【F:crates/sorafs_manifest/src/provider_admission.rs#L1】【F:crates/iroha_torii/src/sorafs/admission.rs#L1】 | Networking TL / Сақтау | ✅ Аяқталды |
| Torii интеграциясы | Тақырып тексерушісін Torii жарнаманы енгізу, саясаттан тыс жарнамаларды қабылдамау, телеметрияны шығару. | Networking TL | ✅ Аяқталды | Torii енді басқару конверттерін жүктейді (`torii.sorafs.admission_envelopes_dir`), қабылдау кезінде дайджест/қолтаңба сәйкестіктерін тексереді және рұқсатты анықтайды телеметрия.【F:crates/iroha_torii/src/sorafs/admission.rs#L1】【F:crates/iroha_torii/src/sorafs/discovery.rs#L1】【F:crates/iroha_torii/src/sorafs/api. |#L1
| Жаңарту | Жаңарту/қайтару схемасын + CLI көмекшілерін қосыңыз, өмірлік цикл нұсқаулығын құжаттарда жариялаңыз (төмендегі runbook және CLI пәрмендерін қараңыз). `provider-admission renewal`/`revoke`).【crates/sorafs_car/src/bin/sorafs_manifest_stub/provider_admission.rs#L477】【docs/source/sorafs/provider_admission_policy.md: |20.md Сақтау / Басқару | ✅ Аяқталды |
| Телеметрия | `provider_admission` бақылау тақталарын және ескертулерді анықтаңыз (жаңарту жоқ, конверттің жарамдылық мерзімі). | Бақылау мүмкіндігі | 🟠 Орындалуда | `torii_sorafs_admission_total{result,reason}` есептегіші бар; бақылау тақталары/ескертулер күтілуде.【F:crates/iroha_telemetry/src/metrics.rs#L3798】【F:docs/source/telemetry.md#L614】 |
### Жаңарту және күшін жою Runbook

#### Жоспарланған жаңарту (үлес/топология жаңартулары)
1. `provider-admission proposal` және `provider-admission sign` арқылы мұрагер ұсыныс/жарнама жұбын құрастырыңыз, `--retention-epoch` көбейтіңіз және талап етілетін үлес/соңғы нүктелерді жаңартыңыз.
2. Орындау  
   ```bash
   cargo run -p sorafs_manifest --bin sorafs_manifest_stub -- provider-admission \
     renewal \
     --previous-envelope=governance/providers/<id>/envelope.to \
     --envelope=governance/providers/<id>/envelope_next.to \
     --renewal-out=governance/providers/<id>/renewal.to \
     --json-out=governance/providers/<id>/renewal.json \
     --notes="stake top-up 2025-03"
   ```
   Пәрмен өзгермеген мүмкіндік/профиль өрістерін арқылы тексереді
   `AdmissionRecord::apply_renewal`, `ProviderAdmissionRenewalV1` шығарады және дайджесттерді басып шығарады
   басқару журналы.【crates/sorafs_car/src/bin/sorafs_manifest_stub/provider_admission.rs#L477】【F:crates/sorafs_manifest/src/provider_admission.rs#L422】
3. `torii.sorafs.admission_envelopes_dir` ішіндегі алдыңғы конвертті ауыстырыңыз, Norito/JSON жаңартуын басқару репозиторийіне тапсырыңыз және `docs/source/sorafs/migration_ledger.md` параметріне жаңарту хэш + сақтау дәуірін қосыңыз.
4. Операторларға жаңа конверттің тірі екендігі туралы хабарлаңыз және қабылдауды растау үшін `torii_sorafs_admission_total{result="accepted",reason="stored"}` бақылаңыз.
5. `cargo run -p sorafs_car --bin provider_admission_fixtures --features cli` арқылы канондық құрылғыларды қайта жасаңыз және бекітіңіз; CI (`ci/check_sorafs_fixtures.sh`) Norito шығыстарының тұрақтылығын растайды.

#### Төтенше жағдайда жою
1. Бұзылған конвертті анықтаңыз және кері қайтарып алуды беріңіз:
   ```bash
   cargo run -p sorafs_manifest --bin sorafs_manifest_stub -- provider-admission \
     revoke \
     --envelope=governance/providers/<id>/envelope.to \
     --reason="endpoint compromise" \
     --revoked-at=$(date +%s) \
     --notes="incident-456" \
     --council-signature=<signer_hex:signature_hex> \
     --revocation-out=governance/providers/<id>/revocation.to \
     --json-out=governance/providers/<id>/revocation.json
   ```
   CLI `ProviderAdmissionRevocationV1` қол қояды, қолтаңбалар жиынтығын арқылы тексереді
   `verify_revocation_signatures` және қайтарып алу дайджесті туралы хабарлайды.【crates/sorafs_car/src/bin/sorafs_manifest_stub/provider_admission.rs#L593】【F:crates/sorafs_manifest/src/provider_admission.rs#48】L
2. `torii.sorafs.admission_envelopes_dir` ішінен конвертті алып тастаңыз, Norito/JSON күшін жоюды рұқсат беру кэштеріне таратыңыз және басқару хаттамасына хэш себебін жазыңыз.
3. Кэштер жойылған жарнаманы тастайтынын растау үшін `torii_sorafs_admission_total{result="rejected",reason="admission_missing"}` қараңыз; қайтарып алу артефактілерін оқиға ретроспективасында сақтаңыз.

## Тестілеу және телеметрия- Қабылдау ұсыныстары мен конверттер үшін алтын қондырғыларды қосыңыз
  `fixtures/sorafs_manifest/provider_admission/`.
- Ұсыныстарды қалпына келтіру және конверттерді тексеру үшін CI (`ci/check_sorafs_fixtures.sh`) кеңейтіңіз.
- Жасалған қондырғыларға канондық дайджесттері бар `metadata.json` кіреді; төменгі сынақтар растайды
  `proposal_digest_hex` == `ca8e73a1f319ae83d7bd958ccb143f9b790c7e4d9c8dfe1f6ad37fa29facf936`.
- Интеграциялық сынақтарды қамтамасыз ету:
  - Torii қабылдау конверттері жоқ немесе мерзімі өтіп кеткен хабарландыруларды қабылдамайды.
  - CLI ұсыныс → конверт → тексеру.
  - Басқаруды жаңарту провайдер идентификаторын өзгертпестен соңғы нүкте аттестациясын айналдырады.
- Телеметриялық талаптар:
  - Torii ішіндегі `provider_admission_envelope_{accepted,rejected}` есептегіштерін шығарыңыз. ✅ `torii_sorafs_admission_total{result,reason}` енді қабылданған/қабылданбаған нәтижелерді көрсетеді.
  - Бақылау мүмкіндігінің бақылау тақталарына жарамдылық мерзімі туралы ескертулерді қосыңыз (жаңарту 7 күн ішінде қажет).

## Келесі қадамдар

1. ✅ Norito схемасының өзгерістері аяқталды және валидация көмекшілері
   `sorafs_manifest::provider_admission`. Ешқандай мүмкіндік жалаушалары қажет емес.
2. ✅ CLI жұмыс үрдістері (`proposal`, `sign`, `verify`, `renewal`, `revoke`) біріктіру сынақтары арқылы құжатталады және орындалады; басқару сценарийлерін runbook бағдарламасымен синхрондаңыз.
3. ✅ Torii қабылдау/табу конверттерді жұтып, қабылдау/қабылдамау үшін телеметриялық есептегіштерді көрсетіңіз.
4. Бақыланатындыққа назар аударыңыз: қабылдау тақталарын/ескертулерді аяқтаңыз, осылайша жеті күн ішінде жаңартулар ескертулерді арттырады (`torii_sorafs_admission_total`, жарамдылық мерзімінің көрсеткіштері).