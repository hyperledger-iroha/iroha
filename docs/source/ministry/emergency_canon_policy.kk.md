---
lang: kk
direction: ltr
source: docs/source/ministry/emergency_canon_policy.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: afd5db8a761f8cc56dcd8f67f047f06b45c3211738fedb6dfff326d84b4e8a68
source_last_modified: "2026-01-22T14:45:02.098548+00:00"
translation_last_reviewed: 2026-02-07
title: Emergency Canon & TTL Policy
summary: Reference implementation notes for roadmap item MINFO-6a covering denylist tiers, TTL enforcement, and governance evidence requirements.
translator: machine-google-reviewed
---

# Төтенше жағдай Canon және TTL саясаты (MINFO-6a)

Жол картасының анықтамасы: **MINFO-6a — Төтенше жағдайдағы канон және TTL саясаты**.

Бұл құжат қазір Torii және CLI нұсқаларында жеткізілетін бас тарту тізімінің деңгейі ережелерін, TTL орындауын және басқару міндеттемелерін анықтайды. Операторлар жаңа жазбаларды жарияламас бұрын немесе төтенше жағдайларды шақырмас бұрын осы ережелерді сақтауы керек.

## Деңгейлік анықтамалар

| Деңгей | Әдепкі TTL | Қарау терезесі | Талаптар |
|------|-------------|---------------|--------------|
| Стандартты | 180 күн (`torii.sorafs_gateway.denylist.standard_ttl`) | жоқ | `issued_at` қамтамасыз етілуі керек. `expires_at` қабылданбауы мүмкін; Torii әдепкі бойынша `issued_at + standard_ttl` болып табылады және ұзағырақ терезелерді қабылдамайды. |
| Төтенше жағдай | 30 күн (`torii.sorafs_gateway.denylist.emergency_ttl`) | 7 күн (`torii.sorafs_gateway.denylist.emergency_review_window`) | Алдын ала мақұлданған канонға сілтеме жасайтын бос емес `emergency_canon` белгісін қажет етеді (мысалы, `csam-hotline`). `issued_at` + `expires_at` 30 күндік терезенің ішіне түсуі керек және тексеру дәлелдері автоматты түрде жасалған соңғы мерзімге сілтеме жасауы керек (`issued_at + review_window`). |
| Тұрақты | Жарамдылық мерзімі жоқ | жоқ | Басқарудың басым көпшілігінің шешімдері үшін сақталған. Жазбаларда бос емес `governance_reference` сілтемесі болуы керек (дауыс идентификаторы, манифест хэші, т.б.). `expires_at` қабылданбады. |

Әдепкі мәндер `torii.sorafs_gateway.denylist.*` арқылы конфигурацияланатын болып қалады және `iroha_cli` Torii файлды қайта жүктегенге дейін жарамсыз жазбаларды ұстау шектеулерін көрсетеді.

## Жұмыс барысы

1. **Метадеректерді дайындаңыз:** `policy_tier`, `issued_at`, `expires_at` (қолданылатын кезде) және `emergency_canon`/`governance_reference` (әр JSON8070 жазбасының ішінде) қамтиды.
2. **Жергілікті түрде растау:** `iroha app sorafs gateway lint-denylist --path <denylist.json>` іске қосыңыз, осылайша CLI деңгейге тән TTL және талап етілетін өрістерді файл орындалмай тұрып немесе бекітеді.
3. **Дәлелдерді жариялау:** аудиторлар шешімді қадағалай алуы үшін GAR істер жинағына (күн тәртібі пакеті, референдум хаттамалары және т.б.) жазбада келтірілген канон идентификаторын немесе басқару анықтамасын тіркеңіз.
4. **Төтенше жазбаларды қарап шығыңыз:** төтенше жағдайлар ережелерінің мерзімі 30 күн ішінде автоматты түрде аяқталады. Операторлар 7 күндік терезеде постфакто шолуды аяқтап, нәтижені министрлік трекеріне/SoraFS дәлелдер қоймасына жазуы керек.
5. **Torii қайта жүктеңіз:** тексерілгеннен кейін `torii.sorafs_gateway.denylist.path` арқылы жоққа шығару жолын орналастырыңыз және Torii қайта іске қосыңыз/қайта жүктеңіз; орындау уақыты жазбаларды қабылдаудан бұрын бірдей шектеулерді жүзеге асырады.

## Құралдар және сілтемелер

- Орындау уақытының саясатын орындау `sorafs::gateway::denylist` (`crates/iroha_torii/src/sorafs/gateway/denylist.rs`) ішінде өмір сүреді және жүктеуші енді `torii.sorafs_gateway.denylist.*` кірістерін талдау кезінде деңгейлік метадеректерді қолданады.
- CLI валидациясы `GatewayDenylistRecord::validate` (`crates/iroha_cli/src/commands/sorafs.rs`) ішіндегі орындау уақытының семантикасын көрсетеді. TTL параметрлері конфигурацияланған терезеден асқанда немесе міндетті канон/басқару сілтемелері жоқ болғанда линтер сәтсіз аяқталады.
- Конфигурациялау тұтқалары `torii.sorafs_gateway.denylist` (`crates/iroha_config/src/parameters/{defaults,actual.rs,user.rs}`) астында анықталған, сондықтан басқару әртүрлі шектеулерді бекітетін болса, операторлар TTL мәндерін өзгерте/қарау мерзімдерін қарай алады.
- Жалпыға ортақ бас тарту тізімі (`docs/source/sorafs_gateway_denylist_sample.json`) енді барлық үш деңгейді суреттейді және жаңа жазбалар үшін канондық үлгі ретінде пайдаланылуы керек.Бұл қоршаулар төтенше жағдайлар тізімін кодтау, шектелмеген TTL-лерді болдырмау және тұрақты блоктар үшін айқын басқару дәлелдерін мәжбүрлеу арқылы жол картасының **MINFO-6a** тармағын қанағаттандырады.

## Тіркеуді автоматтандыру және дәлелдеме экспорты

Төтенше жағдайдағы канондық мақұлдаулар детерминирленген тізілім суретін және а
diff бумасы Torii бас тарту тізімін жүктегенге дейін. Астындағы құрал
`xtask/src/sorafs.rs` плюс CI сымы `ci/check_sorafs_gateway_denylist.sh`
бүкіл жұмыс процесін қамтиды.

### Канондық бума генерациясы

1. Жұмыста өңделмеген жазбаларды кезеңге қойыңыз (әдетте басқарма қарастыратын файл).
   каталог.
2. JSON файлын канонизациялау және жабу арқылы:
   ```bash
   cargo xtask sorafs-gateway denylist pack \
     --input path/to/denylist.json \
     --out artifacts/ministry/denylist_registry/$(date +%Y%m%dT%H%M%SZ) \
     --label ministry-emergency \
     --force
   ```
   Пәрмен қол қойылған қолайлы `.json` бумасын, Norito `.to` шығарады
   конверт және басқаруды тексерушілер күткен Merkle-root мәтіндік файлы.
   Каталогты `artifacts/ministry/denylist_registry/` (немесе сіздің
   таңдалған дәлелдер шелегі) сондықтан `scripts/ministry/transparency_release.py` мүмкін
   оны кейінірек `--artifact denylist_bundle=<path>` арқылы алыңыз.
3. Жасалған `checksums.sha256` итермес бұрын буманың жанында ұстаңыз.
   SoraFS/GAR дейін. CI `ci/check_sorafs_gateway_denylist.sh` бірдей жаттығулар жасайды
   Құралдың жұмыс істеуіне кепілдік беру үшін `pack` көмекшісі үлгіден бас тарту тізіміне қарсы.
   әрбір шығарылым.

### Айырма + аудит жинағы

1. Жаңа жинақты алдыңғы өндіріс суретімен салыстырыңыз
   xtask diff көмекшісі:
   ```bash
   cargo xtask sorafs-gateway denylist diff \
     --old artifacts/ministry/denylist_registry/2026-05-01/denylist_old.json \
     --new artifacts/ministry/denylist_registry/2026-05-14/denylist_new.json \
     --report-json artifacts/ministry/denylist_registry/2026-05-14/denylist_diff.json
   ```
   JSON есебі барлық толықтырулар/жоюлар тізімі және дәлелдемелерді көрсетеді
   `MinistryDenylistChangeV1` арқылы тұтынылатын құрылым (сілтеме
   `docs/source/sorafs_gateway_self_cert.md` және сәйкестік жоспары).
2. Әрбір канон сұрауына `denylist_diff.json` тіркеңіз (ол қанша екенін дәлелдейді.
   жазбалар түртілді, қай деңгей өзгерді және қандай дәлелдер хэш карталары
   канондық бума).
3. Айырмашылықтар автоматты түрде жасалғанда (CI немесе босату құбырлары), экспорттаңыз
   `denylist_diff.json` жолы `--artifact denylist_diff=<path>` арқылы, сондықтан
   мөлдірлік манифесті оны тазартылған көрсеткіштермен бірге жазады. Дәл сол CI
   көмекші CLI жиынтық қадамын іске қосатын `--evidence-out <path>` қабылдайды және
   алынған JSON файлын кейінірек жариялау үшін сұралған орынға көшіреді.

### Жариялану және ашықтық1. Тоқсандық мөлдірлік каталогына бума + дифференциялық артефактілерді тастаңыз
   (`artifacts/ministry/transparency/<YYYY-Q>/denylist/`). Мөлдірлік
   босату көмекшісі оларды қамтуы мүмкін:
   ```bash
   scripts/ministry/transparency_release.py \
     --quarter 2026-Q3 \
     --output-dir artifacts/ministry/transparency/2026-Q3 \
     --sanitized artifacts/ministry/transparency/2026-Q3/sanitized_metrics.json \
     --dp-report artifacts/ministry/transparency/2026-Q3/dp_report.json \
     --artifact denylist_bundle=artifacts/ministry/denylist_registry/2026-05-14/denylist_new.json \
     --artifact denylist_diff=artifacts/ministry/denylist_registry/2026-05-14/denylist_diff.json
   ```
2. Тоқсандық есепте жасалған топтамаға/айырмаға сілтеме жасаңыз
   (`docs/source/ministry/reports/<YYYY-Q>.md`) және бірдей жолдарды файлға тіркеңіз
   GAR дауыс пакеті аудиторлар дәлелдемелерді қолжетімсіз қайталай алады
   ішкі CI. `ci/check_sorafs_gateway_denylist.sh --evidence-out \
   artefacts/ministry/denylist_registry//denylist_evidence.json` қазір
   буманы/дифф/дәлелдерді құрғақ іске қосады (`iroha_cli қолданбасының sorafs шлюзіне қоңырау шалу)
   дәлелі (капот астында) осылайша автоматтандыру түйіндемені бірге сақтай алады
   канондық байламдар.
3. Жарияланғаннан кейін басқарудың пайдалы жүктемесін арқылы бекітіңіз
   `cargo xtask ministry-transparency anchor` (автоматты түрде шақырылады
   `transparency_release.py`, `--governance-dir` берілгенде) сондықтан
   жоққа шығару тізілімінің дайджесті мөлдірлікпен бірдей DAG ағашында пайда болады
   босату.

Осы процестен кейін «тізілімді автоматтандыру және дәлелдемелерді экспорттау» жабылады.
алшақтық `roadmap.md:450` деп аталады және әрбір төтенше жағдайды қамтамасыз етеді
шешім қайталанатын артефактілермен, JSON айырмашылықтарымен және мөлдірлік журналымен келеді
жазбалар.

### TTL және Canon Evidence Helper

Бума/айырма жұбы жасалғаннан кейін, түсіру үшін CLI дәлел көмекшісін іске қосыңыз
TTL қорытындылары мен төтенше жағдайларды тексеру мерзімдері, ол басқару талап етеді:

```bash
iroha app sorafs gateway evidence \
  --denylist artifacts/ministry/denylist_registry/2026-05-14/denylist.json \
  --out artifacts/ministry/denylist_registry/2026-05-14/denylist_evidence.json \
  --label csam-canon-2026-05
```

Пәрмен JSON көзін хэштейді, әрбір жазбаны тексереді және жинақты шығарады
қысқаша мазмұны бар:

- `kind` және ең ерте/соңғы саясат деңгейіне арналған жалпы жазбалар
  уақыт белгілері байқалады.
- Әрбір төтенше жағдайды өзімен бірге санайтын `emergency_reviews[]` тізімі
  дескриптор, тиімді жарамдылық мерзімі, максималды рұқсат етілген TTL және есептелген
  `review_due_by` соңғы мерзімі.

Аудиторлар үшін `denylist_evidence.json` пакетін/диффті бірге тіркеңіз
CLI қайта іске қоспай, TTL сәйкестігін растаңыз. Қазірдің өзінде жасалған CI тапсырмалары
дестелер көмекшіні шақырып, дәлел артефакті жариялай алады (мысалы
`ci/check_sorafs_gateway_denylist.sh --evidence-out <path>` қоңырау шалу), қамтамасыз ету
әрбір канон сұрауы дәйекті қорытындымен келеді.

### Merkle тізілімінің дәлелі

MINFO-6 жүйесінде енгізілген Merkle тізілімі операторлардан ақпаратты жариялауды талап етеді
TTL қорытындысымен бірге түбірлік және әрбір енгізу дәлелдері. Жүгіруден кейін бірден
дәлел көмекшісі, Merkle артефактілерін түсіріңіз:

```bash
iroha app sorafs gateway merkle snapshot \
  --denylist artifacts/ministry/denylist_registry/2026-05-14/denylist.json \
  --json-out artifacts/ministry/denylist_registry/2026-05-14/denylist_merkle_snapshot.json \
  --norito-out artifacts/ministry/denylist_registry/2026-05-14/denylist_merkle_snapshot.to

iroha app sorafs gateway merkle proof \
  --denylist artifacts/ministry/denylist_registry/2026-05-14/denylist.json \
  --index 12 \
  --json-out artifacts/ministry/denylist_registry/2026-05-14/denylist_merkle_proof_entry_12.json \
  --norito-out artifacts/ministry/denylist_registry/2026-05-14/denylist_merkle_proof_entry_12.to
```JSON суреті BLAKE3 Merkle түбірін, жапырақ санын және әрбірін жазады
дескриптор/хэш жұбы, сондықтан GAR дауыстары хэштелген нақты ағашқа сілтеме жасай алады.
`--norito-out` жеткізу `.to` артефактісін JSON жанында сақтайды.
шлюздер тізілім жазбаларын тікелей Norito арқылы қырып тастамай қабылдайды
stdout. `merkle proof` кез келген үшін бағыт биттерін және бауырлас хэштерді шығарады
нөлге негізделген жазба индексі, бұл әрқайсысы үшін қосу дәлелін тіркеуді жеңілдетеді
GAR жадынамасында келтірілген апаттық канон — қосымша Norito көшірмесі дәлелдемені сақтайды
кітапта таратуға дайын. Келесі JSON және Norito артефактілерін сақтаңыз
мөлдірлік шығарылымдары мен басқару үшін TTL жиыны мен айырмашылық бумасына
анкерлер бір түбірге сілтеме жасайды.