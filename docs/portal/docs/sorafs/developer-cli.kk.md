---
lang: kk
direction: ltr
source: docs/portal/docs/sorafs/developer-cli.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 3c9acf8a8d9c298ad2fe95e5480942441aa790346f90c5b5e1a8c1ff638c5e73
source_last_modified: "2026-01-22T16:26:46.522695+00:00"
translation_last_reviewed: 2026-02-07
id: developer-cli
title: SoraFS CLI Cookbook
sidebar_label: CLI Cookbook
description: Task-focused walkthrough of the consolidated `sorafs_cli` surface.
translator: machine-google-reviewed
---

:::ескерту Канондық дереккөз
:::

Біріктірілген `sorafs_cli` беті (`sorafs_car` қорапшасымен қамтамасыз етілген.
`cli` мүмкіндігі қосылған) SoraFS дайындау үшін қажетті әрбір қадамды көрсетеді
артефактілер. Жалпы жұмыс процестеріне тікелей өту үшін осы аспаздық кітапты пайдаланыңыз; онымен жұптаңыз
операциялық контекстке арналған манифест конвейері және оркестрдің жұмыс кітаптары.

## Пакеттің пайдалы жүктемелері

Детерминирленген CAR мұрағаттары мен бөлік жоспарларын жасау үшін `car pack` пайдаланыңыз. The
пәрмен, егер дескриптор қамтамасыз етілмесе, SF-1 кесіндісін автоматты түрде таңдайды.

```bash
sorafs_cli car pack \
  --input fixtures/video.mp4 \
  --car-out artifacts/video.car \
  --plan-out artifacts/video.plan.json \
  --summary-out artifacts/video.car.json
```

- Әдепкі кескіш дескриптор: `sorafs.sf1@1.0.0`.
- Анықтамалық кірістер лексикографиялық ретпен орындалады, сондықтан бақылау сомасы тұрақты болып қалады
  платформалар арқылы.
- JSON қорытындысы пайдалы жүктеме дайджесттерін, әрбір бөлік метадеректерін және түбірді қамтиды
  CID тізілім және оркестрмен танылған.

## Манифесттерді құру

```bash
sorafs_cli manifest build \
  --summary artifacts/video.car.json \
  --pin-min-replicas 4 \
  --pin-storage-class hot \
  --pin-retention-epoch 96 \
  --manifest-out artifacts/video.manifest.to \
  --manifest-json-out artifacts/video.manifest.json
```

- `--pin-*` опциялары тікелей `PinPolicy` өрістеріне салыстыру
  `sorafs_manifest::ManifestBuilder`.
- CLI SHA3 бөлігін қайта есептегенін қаласаңыз, `--chunk-plan` қамтамасыз етіңіз.
  тапсыру алдында дайджест; әйтпесе ол ішіне енгізілген дайджестті қайта пайдаланады
  түйіндеме.
- JSON шығысы тікелей айырмашылықтар үшін Norito пайдалы жүктемесін көрсетеді.
  шолулар.

## Белгі ұзақ өмір сүретін кілттерсіз көрінеді

```bash
sorafs_cli manifest sign \
  --manifest artifacts/video.manifest.to \
  --bundle-out artifacts/video.manifest.bundle.json \
  --signature-out artifacts/video.manifest.sig \
  --identity-token-env SIGSTORE_ID_TOKEN
```

- Кірістірілген таңбалауыштарды, орта айнымалы мәндерін немесе файлға негізделген көздерді қабылдайды.
- Шығу метадеректерін қосады (`token_source`, `token_hash_hex`, кесінді дайджест)
  `--include-token=true` қоспағанда, шикі JWT сақталмай.
- CI-де жақсы жұмыс істейді: орнату арқылы GitHub Actions OIDC біріктіріңіз
  `--identity-token-provider=github-actions`.

## Манифесттерді Torii жіберіңіз

```bash
sorafs_cli manifest submit \
  --manifest artifacts/video.manifest.to \
  --chunk-plan artifacts/video.plan.json \
  --torii-url https://gateway.example/v1 \
  --authority i105... \
  --private-key ed25519:0123...beef \
  --alias-namespace sora \
  --alias-name video::launch \
  --alias-proof fixtures/alias_proof.bin \
  --summary-out artifacts/video.submit.json
```

- Бүркеншік аттың дәлелдері үшін Norito декодтауын орындайды және олардың сәйкестігін тексереді
  манифест дайджесті Torii ішіне POST жібермес бұрын.
- Сәйкессіздік шабуылдарының алдын алу үшін SHA3 дайджестін жоспардан қайта есептейді.
- Жауап қорытындылары HTTP күйін, тақырыптарды және тізілімнің пайдалы жүктемелерін алады
  кейінірек тексеру.

## CAR мазмұны мен дәлелдерін тексеріңіз

```bash
sorafs_cli proof verify \
  --manifest artifacts/video.manifest.to \
  --car artifacts/video.car \
  --summary-out artifacts/video.verify.json
```

- PoR ағашын қалпына келтіреді және пайдалы жүктеме дайджесттерін манифест жиынымен салыстырады.
- Көшіру дәлелдерін жіберу кезінде қажет сандар мен идентификаторларды түсіреді
  басқаруға.

## Ағынды тексеру телеметриясы

```bash
sorafs_cli proof stream \
  --manifest artifacts/video.manifest.to \
  --gateway-url https://gateway.example/v1/sorafs/proof/stream \
  --provider-id provider::alpha \
  --samples 32 \
  --stream-token "$(cat stream.token)" \
  --summary-out artifacts/video.proof_stream.json \
  --governance-evidence-dir artifacts/video.proof_stream_evidence
```

- Әрбір ағынды дәлелдеу үшін NDJSON элементтерін шығарады (қайта ойнатуды өшіру
  `--emit-events=false`).
- Сәтті/сәтсіздік сандарын, кешігу гистограммаларын және іріктелген сәтсіздіктерді біріктіреді
  бақылау тақталары журналдарды сызып тастамай нәтижелерді жоспарлауы үшін жиынтық JSON.
- Шлюз қателер немесе жергілікті PoR тексеруі туралы хабарлағанда нөлден тыс шығады
  (`--por-root-hex` арқылы) дәлелдемелерді қабылдамайды. Шектерді көмегімен реттеңіз
  Жаттығулар үшін `--max-failures` және `--max-verification-failures`.
- Бүгінгі күні PoR қолдайды; PDP және PoTR бір конвертті SF-13/SF-14 бір рет қайта пайдаланады
  жер.
- `--governance-evidence-dir` көрсетілген қорытындыны, метадеректерді (уақыт белгісі,
  CLI нұсқасы, шлюз URL мекенжайы, манифест дайджесті) және манифесттің көшірмесі
  басқару пакеттері дәлелдеу ағынын мұрағаттауы үшін берілген каталог
  жүгіруді қайталамай дәлелдер.

## Қосымша сілтемелер

- `docs/source/sorafs_cli.md` — толық жалау құжаттамасы.
- `docs/source/sorafs_proof_streaming.md` — дәлелді телеметриялық схема және Grafana
  бақылау тақтасының үлгісі.
- `docs/source/sorafs/manifest_pipeline.md` — бөлшектеуге терең сүңгу, манифест
  құрамы және CAR өңдеу.