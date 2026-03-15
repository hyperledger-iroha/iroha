---
lang: kk
direction: ltr
source: docs/examples/finance/repo_governance_packet_template.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: cd018a94197722adfbb9d54bf02f1c486147078174ba4c81f32e9d93b8c3f6d5
source_last_modified: "2026-01-22T16:26:46.473419+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

<!--
  SPDX-License-Identifier: Apache-2.0
-->

# Репо басқару пакетінің үлгісі (F1 жол картасы)

Жол картасы элементі талап ететін артефакт бумасын дайындау кезінде осы үлгіні пайдаланыңыз
F1 (репо өмірлік циклінің құжаттамасы және құралдары). Мақсат – рецензенттерді тапсыру a
Әрбір кірісті, хэшті және дәлелдемелерді тізімдейтін жалғыз Markdown файлы
басқару кеңесі ұсыныста сілтеме жасалған байттарды қайталай алады.

> Үлгіні өзіңіздің дәлелдер каталогыңызға көшіріңіз (мысалы
> `artifacts/finance/repo/2026-03-15/packet.md`), толтырғыштарды ауыстырыңыз және
> оны төменде сілтеме жасалған хэштелген артефактілердің жанында орындаңыз/жүктеп салыңыз.

## 1. Метадеректер

| Өріс | Мән |
|-------|-------|
| Келісім/идентификаторды өзгерту | `<repo-yyMMdd-XX>` |
| Дайындалған / күні | `<desk lead> – 2026-03-15T10:00Z` |
| Қараған | `<dual-control reviewer(s)>` |
| Түрін өзгерту | `Initiation / Haircut update / Substitution matrix change / Margin policy` |
| Кастодиан(дар) | `<custodian id(s)>` |
| Байланысты ұсыныс/референдум | `<governance ticket id or GAR link>` |
| Дәлелдер анықтамалығы | ``artifacts/finance/repo/<slug>/`` |

## 2. Нұсқаулардың пайдалы жүктемелері

Жұмыс үстелдері арқылы қол қойылған кезеңдік Norito нұсқауларын жазып алыңыз.
`iroha app repo ... --output`. Әрбір жазба шығарылған хэшті қамтуы керек
файл және дауыс беруден кейін жіберілетін әрекеттің қысқаша сипаттамасы
өтеді.

| Әрекет | Файл | SHA-256 | Ескертпелер |
|--------|------|---------|-------|
| Бастау | `instructions/initiate.json` | `<sha256>` | Құрамында касса + контрагент бекіткен қолма-қол ақша/кепіл бөліктері бар. |
| Маржа қоңырауы | `instructions/margin_call.json` | `<sha256>` | Қоңырауды тудырған каденс + қатысушы идентификаторын түсіреді. |
| Демалу | `instructions/unwind.json` | `<sha256>` | Шарттар орындалғаннан кейін кері аяқтың дәлелі. |

```bash
# Example hash helper (repeat per instruction file)
sha256sum artifacts/finance/repo/<slug>/instructions/initiate.json \
  | tee artifacts/finance/repo/<slug>/hashes/initiate.sha256
```

## 2.1 Кастодианның алғысы (тек үш жақты)

Репо `--custodian` пайдаланған сайын осы бөлімді аяқтаңыз. Басқару пакеті
әрбір кастодианның қол қойылған растауын және хэшті қамтуы керек
`docs/source/finance/repo_ops.md` құжатының §2.8 тармағында сілтеме жасалған файл.

| Кастодиан | Файл | SHA-256 | Ескертпелер |
|----------|------|---------|-------|
| `<i105...>` | `custodian_ack_<custodian>.md` | `<sha256>` | Қамқорлық терезесін, бағыттау тіркелгісін және бұрғылау контактісін қамтитын қол қойылған SLA. |

> Басқа дәлелдердің жанында растауды сақтаңыз (`artifacts/finance/repo/<slug>/`)
> сондықтан `scripts/repo_evidence_manifest.py` файлды сол ағашқа жазады
> кезеңдік нұсқаулар және конфигурация үзінділері. Қараңыз
> `docs/examples/finance/repo_custodian_ack_template.md` толтыруға дайын
> басқару дәлелдері келісім-шартына сәйкес келетін үлгі.

## 3. Конфигурация үзіндісі

Кластерге түсетін `[settlement.repo]` TOML блогын қойыңыз (соның ішінде
`collateral_substitution_matrix`). Хэшті үзіндінің жанында сақтаңыз
аудиторлар репо брондау кезінде белсенді болған орындау уақыты саясатын растай алады
бекітілді.

```toml
[settlement.repo]
eligible_collateral = ["bond#wonderland", "note#wonderland"]
default_margin_percent = "0.025"

[settlement.repo.collateral_substitution_matrix]
"bond#wonderland" = ["bill#wonderland"]
```

`SHA-256 (config snippet): <sha256>`

### 3.1 Бекітуден кейінгі конфигурация суреттері

Референдум немесе басқару туралы дауыс беру аяқталғаннан кейін және `[settlement.repo]`
өзгерту іске қосылды, әр теңдесінен `/v2/configuration` суретін түсіріңіз.
аудиторлар бекітілген саясаттың кластер бойынша әрекет ететінін дәлелдей алады (қараңыз
`docs/source/finance/repo_ops.md` §2.9 дәлелдер жұмыс процесі үшін).

```bash
mkdir -p artifacts/finance/repo/<slug>/config/peers
curl -fsSL https://peer01.example/v2/configuration \
  | jq '.' \
  > artifacts/finance/repo/<slug>/config/peers/peer01.json
```

| Құрдас / дереккөз | Файл | SHA-256 | Блок биіктігі | Ескертпелер |
|-------------|------|---------|--------------|-------|
| `peer01` | `config/peers/peer01.json` | `<sha256>` | `<block-height>` | Сурет конфигурацияны шығарғаннан кейін бірден түсірілді. |
| `peer02` | `config/peers/peer02.json` | `<sha256>` | `<block-height>` | `[settlement.repo]` кезеңдік TOML сәйкестігін растайды. |

`hashes.txt` (немесе баламасы) ішіндегі тең идентификаторлармен бірге дайджесттерді жазыңыз
шолушылар өзгерісті қай түйіндер қабылдағанын бақылай алады. Суреттер
TOML үзіндісінің жанындағы `config/peers/` астында тұрады және алынады
`scripts/repo_evidence_manifest.py` арқылы автоматты түрде.

## 4. Детерминистік сынақ артефактілері

Келесіден соңғы нәтижелерді тіркеңіз:

- `cargo test -p iroha_core -- repo_deterministic_lifecycle_proof_matches_fixture`
- `cargo test --package integration_tests --test repo`

Жазба файл жолдары + журнал жинақтары үшін хэштер немесе CI арқылы жасалған JUnit XML
жүйесі.

| Артефакт | Файл | SHA-256 | Ескертпелер |
|----------|------|---------|-------|
| Өмірлік циклді дәлелдеу журналы | `tests/repo_lifecycle.log` | `<sha256>` | `--nocapture` шығысымен түсірілген. |
| Интеграциялық сынақ журналы | `tests/repo_integration.log` | `<sha256>` | Ауыстыру + маржа каденциясын қамтуды қамтиды. |

## 5. Өмірлік циклді дәлелдейтін сурет

Әрбір пакет экспортталған өмірлік циклдің детерминирленген суретін қамтуы керек
`repo_deterministic_lifecycle_proof_matches_fixture`. белгішесін көмегімен іске қосыңыз
Экспорттау тұтқалары қосылды, сондықтан шолушылар JSON жақтауын ажыратып, оған қарсы дайджест жасай алады
арматура `crates/iroha_core/tests/fixtures/` ішінде бақыланады (қараңыз
`docs/source/finance/repo_ops.md` §2.7).

```bash
REPO_PROOF_SNAPSHOT_OUT=artifacts/finance/repo/<slug>/repo_proof_snapshot.json \
REPO_PROOF_DIGEST_OUT=artifacts/finance/repo/<slug>/repo_proof_digest.txt \
cargo test -p iroha_core \
  -- --exact smartcontracts::isi::repo::tests::repo_deterministic_lifecycle_proof_matches_fixture
```

Немесе арматураларды қалпына келтіру және оларды өзіңізге көшіру үшін бекітілген көмекшіні пайдаланыңыз
Дәлелдер жинағы бір қадамда:

```bash
scripts/regen_repo_proof_fixture.sh --toolchain <toolchain> \
  --bundle-dir artifacts/finance/repo/<slug>
```

| Артефакт | Файл | SHA-256 | Ескертпелер |
|----------|------|---------|-------|
| Snapshot JSON | `repo_proof_snapshot.json` | `<sha256>` | Дәлелдеу белгішесі шығаратын канондық өмірлік цикл кадры. |
| Дайджест файлы | `repo_proof_digest.txt` | `<sha256>` | `crates/iroha_core/tests/fixtures/repo_lifecycle_proof.digest` ішінен шағылыстырылған бас әріп алтылық дайджест; өзгеріссіз болса да бекітіңіз. |

## 6. Дәлелдер манифесті

Аудиторлар тексере алуы үшін бүкіл дәлелдер каталогы үшін манифест жасаңыз
мұрағатты ашпастан хэштер. Көмекші сипатталған жұмыс процесін көрсетеді
`docs/source/finance/repo_ops.md` §3.2.

```bash
python3 scripts/repo_evidence_manifest.py \
  --root artifacts/finance/repo/<slug> \
  --agreement-id <repo-identifier> \
  --output artifacts/finance/repo/<slug>/manifest.json
```

| Артефакт | Файл | SHA-256 | Ескертпелер |
|----------|------|---------|-------|
| Дәлелдер манифесті | `manifest.json` | `<sha256>` | Бақылау сомасын басқару билетіне/референдум ескертулеріне қосыңыз. |

## 7. Телеметрия және оқиғаның суреті

Сәйкес `AccountEvent::Repo(*)` жазбаларын және кез келген бақылау тақталарын немесе CSV экспорттау
экспортқа сілтеме `docs/source/finance/repo_ops.md`. Файлдарды жазыңыз +
мұнда хэштер бар, осылайша рецензенттер тікелей дәлелдерге ауыса алады.

| Экспорт | Файл | SHA-256 | Ескертпелер |
|--------|------|---------|-------|
| Репо оқиғалары JSON | `evidence/repo_events.ndjson` | `<sha256>` | Шикі Torii оқиға ағыны үстел тіркелгілеріне сүзілген. |
| CSV телеметриясы | `evidence/repo_margin_dashboard.csv` | `<sha256>` | Repo Margin тақтасы арқылы Grafana ішінен экспортталды. |

## 8. Бекітулер мен қолдар

- **Қос бақылауды қол қоюшылар:** `<names + timestamps>`
- **GAR / минут дайджест:** Қол қойылған GAR PDF файлының `<sha256>` немесе жүктеп салу минуттары.
- **Сақтау орны:** `governance://finance/repo/<slug>/packet/`

## 9. Бақылау парағы

Әрбір элементті аяқталғаннан кейін белгілеңіз.

- [ ] Нұсқаулардың пайдалы жүктемелері кезеңдік, хэштелген және тіркелген.
- [ ] Конфигурация үзіндісі хэш жазылды.
- [ ] Детерминистік сынақ журналдары түсірілді + хэштелген.
- [ ] Өмір циклінің суреті + дайджест экспортталды.
- [ ] Айғақ манифесті жасалды және хэш жазылды.
- [ ] Оқиға/телеметрия экспорттары түсірілді + хэштелген.
- [ ] Қосарлы басқару растаулары мұрағатталды.
- [ ] GAR/минут жүктеп салынған; жоғарыда жазылған дайджест.

Осы үлгіні әрбір пакетпен бірге сақтау DAG басқаруды сақтайды
детерминирленген және аудиторларға репо өмірлік циклі үшін портативті манифест береді
шешімдер.