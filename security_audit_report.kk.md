<!-- Auto-generated stub for Kazakh (kk) translation. Replace this content with the full translation. -->

---
lang: kk
direction: ltr
source: security_audit_report.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 4343a8f72823a2539a2e79e80a1c7162d455e03b450e2fd613f01ac5c61aec63
source_last_modified: "2026-03-26T13:01:48.980087+00:00"
translation_last_reviewed: 2026-04-02
translator: machine-google-reviewed
---

# Қауіпсіздік аудитінің есебі

Күні: 26.03.2026 ж

## Қорытынды

Бұл аудит ағымдағы ағаштағы ең қауіпті беттерге назар аударды: Torii HTTP/API/auth ағындары, P2P тасымалдауы, құпия өңдеу API интерфейстері, SDK тасымалдау қорғаушылары және тіркеме дезинфекциялық жол.

Мен шешуге болатын 6 мәселені таптым:

- 2 Ауырлығы жоғары нәтижелер
- 4 орташа ауырлықтағы қорытындылар

Ең маңызды мәселелер:

1. Torii қазіргі уақытта тасымалдаушы таңбалауыштарын, API таңбалауыштарын, оператор сеансын/жүктеу таңбалауыштарын және журналдарға қайта жіберілген mTLS маркерлерін көрсете алатын әрбір HTTP сұрауы үшін кіріс сұрау тақырыптарын тіркейді.
2. Бірнеше жалпыға қолжетімді Torii маршруттары мен SDK әлі де `private_key` бастапқы мәндерін серверге жіберуді қолдайды, сондықтан Torii қоңырау шалушы атынан қол қоя алады.
3. Бірнеше "құпия" жолдар кәдімгі сұрау органдары ретінде қарастырылады, соның ішінде құпия тұқым шығару және кейбір SDK-дағы канондық сұрау аутентификациясы.

## әдісі

- Torii, P2P, крипто/VM және SDK құпия өңдеу жолдарын статикалық шолу
- Мақсатты тексеру командалары:
  - `cargo check -p iroha_torii --lib --message-format short` -> өту
  - `cargo check -p iroha_p2p --message-format short` -> өту
  - `cargo test -p iroha_torii --lib confidential_derive_keyset_endpoint_roundtrip -- --nocapture` -> өту
  - `cargo deny check advisories bans sources --hide-inclusion-graph` -> өту, тек қайталанатын нұсқа туралы ескертулер
- Бұл жолда толтырылмаған:
  - толық жұмыс кеңістігін құрастыру/сынау/клип
  - Swift/Gradle сынақ жинақтары
  - CUDA/Metal орындау уақытын тексеру

## Нәтижелер

### SA-001 Жоғары: Torii жаһандық деңгейде сезімтал сұрау тақырыптарын тіркейдіӘсері: тасымалдаушы/API/оператор таңбалауыштарын және қатысты аутентификация материалын қолданба журналдарына жіберуі мүмкін.

Дәлел:

- `crates/iroha_torii/src/lib.rs:20752` `TraceLayer::new_for_http()` қосады
- `crates/iroha_torii/src/lib.rs:20753` `DefaultMakeSpan::default().include_headers(true)` қосады
- Сезімтал тақырып атаулары сол қызметтің басқа жерінде белсенді қолданылады:
  - `crates/iroha_torii/src/operator_auth.rs:40`
  - `crates/iroha_torii/src/operator_auth.rs:41`
  - `crates/iroha_torii/src/operator_auth.rs:42`
  - `crates/iroha_torii/src/operator_auth.rs:43`

Неліктен бұл маңызды:

- `include_headers(true)` толық кіріс тақырып мәндерін бақылау аралығына жазады.
- Torii `Authorization`, `x-api-token`, `x-iroha-operator-session`, `x-iroha-operator-token` және `x-forwarded-client-cert` сияқты тақырыптардағы аутентификация материалын қабылдайды.
- Осылайша, журналдың раковинасының бұзылуы, жөндеу журналының жинағы немесе қолдау жинағы тіркелгі деректерін ашу оқиғасына айналуы мүмкін.

Ұсынылатын түзету:

- Өндіріс ауқымына толық сұрау тақырыптарын қосуды тоқтатыңыз.
- Түзету үшін тақырып журналы әлі де қажет болса, қауіпсіздікке сезімтал тақырыптар үшін нақты редакциялауды қосыңыз.
- Деректер оң рұқсат етілген тізімде болмаса, сұрау/жауап журналын әдепкі бойынша құпия ретінде қарастырыңыз.

### SA-002 Жоғары: жалпы Torii API интерфейстері әлі де серверлік қол қою үшін өңделмеген жеке кілттерді қабылдайды

Әсері: API, SDK, прокси және сервер жады деңгейлерінде қажетсіз құпияны ашу арнасын жасай отырып, сервер олардың атынан қол қоюы үшін клиенттерге желі арқылы өңделмеген жеке кілттерді жіберу ұсынылады.

Дәлел:- Басқару маршрутының құжаттамасы серверлік қол қоюды ашық түрде жариялайды:
  - `crates/iroha_torii/src/gov.rs:495`
- Маршрутты іске асыру берілген жеке кілтті талдайды және сервер жағына қол қояды:
  - `crates/iroha_torii/src/gov.rs:1088`
  - `crates/iroha_torii/src/gov.rs:1091`
  - `crates/iroha_torii/src/gov.rs:1123`
  - `crates/iroha_torii/src/gov.rs:1125`
- SDK `private_key` JSON органдарына белсенді түрде серияланады:
  - `kotlin/core-jvm/src/main/java/org/hyperledger/iroha/sdk/client/OfflineToriiClient.kt:47`
  - `kotlin/core-jvm/src/main/java/org/hyperledger/iroha/sdk/client/OfflineToriiClient.kt:50`
  - `kotlin/core-jvm/src/main/java/org/hyperledger/iroha/sdk/client/OfflineToriiClient.kt:260`
  - `kotlin/core-jvm/src/main/java/org/hyperledger/iroha/sdk/client/OfflineToriiClient.kt:261`

Ескертулер:

- Бұл үлгі бір маршрут тобына оқшауланбаған. Ағымдағы тармақта басқару, желіден тыс қолма-қол ақша, жазылымдар және қолданбаға қатысты басқа DTOs бойынша бірдей ыңғайлылық үлгісі бар.
- Тек HTTPS тасымалдау тексерулері кездейсоқ ашық мәтінді тасымалдауды азайтады, бірақ олар сервер тарапынан құпия өңдеуді немесе журналға жазу/жадтың әсер ету қаупін шешпейді.

Ұсынылатын түзету:

- `private_key` шикі деректерін тасымалдайтын барлық сұрау DTOs ескірген.
- Клиенттерден жергілікті қол қоюды және қол қоюды немесе толық қол қойылған транзакцияларды/конверттерді ұсынуды талап ету.
- Үйлесімділік терезесінен кейін `private_key` мысалдарын OpenAPI/SDK файлдарынан жойыңыз.

### SA-003 Ортасы: құпия кілтті шығару құпия тұқымдық материалды Torii нөміріне жібереді және оны кері қайтарады.

Әсері: құпия кілтті шығару API тұқымдық материалды қалыпты сұрау/жауап пайдалы жүктеме деректеріне айналдырып, прокси-серверлер, аралық бағдарлама, журналдар, іздер, бұзылу есептері немесе клиентті дұрыс пайдаланбау арқылы тұқымның ашылуы мүмкіндігін арттырады.

Дәлел:- Сұраныс тұқымдық материалды тікелей қабылдайды:
  - `crates/iroha_torii/src/routing.rs:2736`
  - `crates/iroha_torii/src/routing.rs:2738`
  - `crates/iroha_torii/src/routing.rs:2740`
- Жауап схемасы тұқымды hex және base64-те қайталайды:
  - `crates/iroha_torii/src/routing.rs:2745`
  - `crates/iroha_torii/src/routing.rs:2746`
  - `crates/iroha_torii/src/routing.rs:2747`
- өңдеуші нақты түрде қайта кодтайды және тұқымды қайтарады:
  - `crates/iroha_torii/src/routing.rs:2797`
  - `crates/iroha_torii/src/routing.rs:2801`
  - `crates/iroha_torii/src/routing.rs:2802`
  - `crates/iroha_torii/src/routing.rs:2804`
- Swift SDK мұны кәдімгі желі әдісі ретінде көрсетеді және жауап үлгісінде жаңғырық тұқымын сақтайды:
  - `IrohaSwift/Sources/IrohaSwift/ToriiClient.swift:4716`
  - `IrohaSwift/Sources/IrohaSwift/ToriiClient.swift:4717`
  - `IrohaSwift/Sources/IrohaSwift/ToriiClient.swift:4718`
  - `IrohaSwift/Sources/IrohaSwift/ToriiClient.swift:11912`
  - `IrohaSwift/Sources/IrohaSwift/ToriiClient.swift:11926`

Ұсынылатын түзету:

- CLI/SDK кодындағы жергілікті кілт туындысын таңдап, қашықтан шығару жолын толығымен алып тастаңыз.
- Егер бағыт қалуы керек болса, жауапта тұқымды ешқашан қайтармаңыз және барлық көлік қорғауыштарында және телеметрия/каротаж жолдарында тұқым беретін денелерді сезімтал деп белгілеңіз.

### SA-004 Ортасы: SDK тасымалдау сезімталдығын анықтауда `private_key` емес құпия материал үшін соқыр нүктелер бар

Әсері: Кейбір SDK өңделмеген `private_key` сұраулары үшін HTTPS күшіне енеді, бірақ бәрібір басқа қауіпсіздікке сезімтал сұрау материалдарының қауіпті HTTP арқылы немесе сәйкес келмейтін хосттарға өтуіне мүмкіндік береді.

Дәлел:- Swift канондық сұрау аутентификация тақырыптарын сезімтал ретінде қарастырады:
  - `IrohaSwift/Sources/IrohaSwift/TransportSecurity.swift:4`
  - `IrohaSwift/Sources/IrohaSwift/TransportSecurity.swift:7`
  - `IrohaSwift/Sources/IrohaSwift/TransportSecurity.swift:8`
  - `IrohaSwift/Sources/IrohaSwift/TransportSecurity.swift:9`
  - `IrohaSwift/Sources/IrohaSwift/TransportSecurity.swift:10`
- Бірақ Swift әлі күнге дейін `"private_key"` жүйесінде денеге сәйкес келеді:
  - `IrohaSwift/Sources/IrohaSwift/TransportSecurity.swift:18`
  - `IrohaSwift/Sources/IrohaSwift/TransportSecurity.swift:66`
  - `IrohaSwift/Sources/IrohaSwift/TransportSecurity.swift:69`
- Котлин тек `authorization` және `x-api-token` тақырыптарын таниды, содан кейін сол `"private_key"` корпусының эвристикасына қайтады:
  - `kotlin/core-jvm/src/main/java/org/hyperledger/iroha/sdk/client/TransportSecurity.kt:50`
  - `kotlin/core-jvm/src/main/java/org/hyperledger/iroha/sdk/client/TransportSecurity.kt:53`
  - `kotlin/core-jvm/src/main/java/org/hyperledger/iroha/sdk/client/TransportSecurity.kt:58`
  - `kotlin/core-jvm/src/main/java/org/hyperledger/iroha/sdk/client/TransportSecurity.kt:61`
- Java/Android бірдей шектеулерге ие:
  - `java/iroha_android/src/main/java/org/hyperledger/iroha/android/client/TransportSecurity.java:24`
  - `java/iroha_android/src/main/java/org/hyperledger/iroha/android/client/TransportSecurity.java:25`
  - `java/iroha_android/src/main/java/org/hyperledger/iroha/android/client/TransportSecurity.java:100`
  - `java/iroha_android/src/main/java/org/hyperledger/iroha/android/client/TransportSecurity.java:104`
- Kotlin/Java канондық сұрауға қол қоюшылар өздерінің көлік қорғаушылары сезімтал ретінде жіктелмейтін қосымша аутентификация тақырыптарын жасайды:
  - `kotlin/core-jvm/src/main/java/org/hyperledger/iroha/sdk/client/CanonicalRequestSigner.kt:17`
  - `kotlin/core-jvm/src/main/java/org/hyperledger/iroha/sdk/client/CanonicalRequestSigner.kt:18`
  - `kotlin/core-jvm/src/main/java/org/hyperledger/iroha/sdk/client/CanonicalRequestSigner.kt:51`
  - `java/iroha_android/src/main/java/org/hyperledger/iroha/android/client/CanonicalRequestSigner.java:25`
  - `java/iroha_android/src/main/java/org/hyperledger/iroha/android/client/CanonicalRequestSigner.java:26`
  - `java/iroha_android/src/main/java/org/hyperledger/iroha/android/client/CanonicalRequestSigner.java:27`
  - `java/iroha_android/src/main/java/org/hyperledger/iroha/android/client/CanonicalRequestSigner.java:28`

Ұсынылатын түзету:

- Эвристикалық денені сканерлеуді нақты сұрау классификациясымен ауыстырыңыз.
- Канондық аутентификация тақырыптарын, тұқымдық/құпия фразалар өрістерін, қол қойылған мутация тақырыптарын және кез келген болашақ құпияны білдіретін өрістерді ішкі жол сәйкестігі бойынша емес, келісім бойынша сезімтал ретінде қарастырыңыз.
- Swift, Kotlin және Java тілдерінде сезімталдық ережелерін туралаңыз.

### SA-005 Ортасы: "құмсалғыш" қосымшасы тек ішкі процесс және `setrlimit`Әсері: тіркеме дезинфекциялау құралы "құмсалғышқа салынған" ретінде сипатталады және хабарланады, бірақ іске асыру ресурс шектеулері бар ағымдағы екілік файлдың шанышқысы/орындалуы ғана. Талдаушы немесе мұрағаттық эксплойт Torii сияқты бірдей пайдаланушымен, файлдық жүйе көрінісімен және қоршаған желі/процесс артықшылықтарымен орындалады.

Дәлел:

- Сыртқы жол бала уылдырық шашқаннан кейін нәтижені құмсалғыш ретінде белгілейді:
  - `crates/iroha_torii/src/zk_attachments.rs:756`
  - `crates/iroha_torii/src/zk_attachments.rs:760`
  - `crates/iroha_torii/src/zk_attachments.rs:776`
  - `crates/iroha_torii/src/zk_attachments.rs:782`
- Бала ағымдағы орындалатын файлға әдепкі бойынша:
  - `crates/iroha_torii/src/zk_attachments.rs:913`
  - `crates/iroha_torii/src/zk_attachments.rs:919`
- Ішкі процесс анық түрде `AttachmentSanitizerMode::InProcess` түріне ауысады:
  - `crates/iroha_torii/src/zk_attachments.rs:1794`
  - `crates/iroha_torii/src/zk_attachments.rs:1803`
- Қолданылатын жалғыз қатайту - CPU/мекенжай кеңістігі `setrlimit`:
  - `crates/iroha_torii/src/zk_attachments.rs:1845`
  - `crates/iroha_torii/src/zk_attachments.rs:1850`
  - `crates/iroha_torii/src/zk_attachments.rs:1851`
  - `crates/iroha_torii/src/zk_attachments.rs:1872`

Ұсынылатын түзету:

- Нақты ОЖ құм жәшігін (мысалы, аттар кеңістігі/seccomp/landlock/түрме стиліндегі оқшаулау, артықшылықты жоғалту, желісіз, шектелген файлдық жүйе) іске қосыңыз немесе нәтижені `sandboxed` ретінде белгілеуді тоқтатыңыз.
- Шынайы оқшаулау болғанша, ағымдағы дизайнды API, телеметрия және құжаттардағы «құмсалғыш» емес, «ішкі процесті оқшаулау» ретінде қарастырыңыз.

### SA-006 Ортасы: Қосымша P2P TLS/QUIC тасымалдаулары сертификатты тексеруді өшіредіӘсер: `quic` немесе `p2p_tls` қосылғанда, арна шифрлауды қамтамасыз етеді, бірақ қашықтағы соңғы нүктенің түпнұсқалығын растамайды. Белсенді шабуылдаушы операторлар TLS/QUIC-пен байланыстыратын қалыпты қауіпсіздік күтулерін жеңе отырып, арнаны жібере немесе тоқтата алады.

Дәлел:

- QUIC рұқсат беретін сертификатты тексеруді нақты құжаттайды:
  - `crates/iroha_p2p/src/transport.rs:12`
  - `crates/iroha_p2p/src/transport.rs:13`
  - `crates/iroha_p2p/src/transport.rs:14`
  - `crates/iroha_p2p/src/transport.rs:15`
- QUIC тексерушісі сервер сертификатын сөзсіз қабылдайды:
  - `crates/iroha_p2p/src/transport.rs:33`
  - `crates/iroha_p2p/src/transport.rs:35`
  - `crates/iroha_p2p/src/transport.rs:44`
  - `crates/iroha_p2p/src/transport.rs:112`
  - `crates/iroha_p2p/src/transport.rs:114`
  - `crates/iroha_p2p/src/transport.rs:115`
- TLS-over-TCP тасымалдауы дәл осылай жасайды:
  - `crates/iroha_p2p/src/transport.rs:229`
  - `crates/iroha_p2p/src/transport.rs:232`
  - `crates/iroha_p2p/src/transport.rs:241`
  - `crates/iroha_p2p/src/transport.rs:279`
  - `crates/iroha_p2p/src/transport.rs:281`
  - `crates/iroha_p2p/src/transport.rs:282`

Ұсынылатын түзету:

- Не тең дәрежелі сертификаттарды тексеріңіз немесе жоғары деңгейлі қол қойылған қол алысу мен тасымалдау сеансы арасында нақты арна байланыстыруын қосыңыз.
- Ағымдағы әрекет әдейі болса, операторлар оны толық TLS тең аутентификациясы деп қателеспеуі үшін мүмкіндіктің атын аутентификацияланбаған шифрланған тасымалдау ретінде өзгертіңіз/құжаттаңыз.

## Ұсынылған қалпына келтіру тәртібі1. Тақырып журналын өзгерту немесе өшіру арқылы SA-001 түзетіңіз.
2. SA-002 үшін тасымалдау жоспарын жасаңыз және жіберіңіз, осылайша өңделмеген жеке кілттер API шекарасын кесіп өтуді тоқтатады.
3. Қашықтағы құпия кілттерді шығару жолын алып тастаңыз немесе тарылтыңыз және тұқым беретін денелерді сезімтал деп жіктеңіз.
4. Swift/Kotlin/Java бойынша SDK тасымалдау сезімталдық ережелерін туралаңыз.
5. Тіркеменің санитариясына нақты құмсалғыш қажет пе немесе атын өзгерту/көлемді өзгерту қажет пе екенін шешіңіз.
6. Операторлар аутентификацияланған TLS күтетін тасымалдауларды қоспас бұрын P2P TLS/QUIC қауіп үлгісін нақтылаңыз және қатайтыңыз.

## Тексеру жазбалары

- `cargo check -p iroha_torii --lib --message-format short` өтті.
- `cargo check -p iroha_p2p --message-format short` өтті.
- `cargo deny check advisories bans sources --hide-inclusion-graph` құм жәшігінен тыс жұмыс істегеннен кейін өтті; ол қайталанатын нұсқа ескертулерін шығарды, бірақ `advisories ok, bans ok, sources ok` хабарлады.
- Құпия туынды-кілттер жинағы бағытына бағытталған Torii сынағы осы аудит кезінде басталды, бірақ есеп жазылғанға дейін аяқталмады; қорытындыға қарамастан, тікелей дереккөзді тексеру арқылы расталады.