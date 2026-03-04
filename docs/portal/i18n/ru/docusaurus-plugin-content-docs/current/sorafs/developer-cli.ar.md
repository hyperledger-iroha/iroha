---
lang: ru
direction: ltr
source: docs/portal/docs/sorafs/developer-cli.ar.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
идентификатор: разработчик-cli
title: Код CLI для SoraFS
Sidebar_label: Интерфейс CLI
описание: شرح موجّه للمهام لسطح `sorafs_cli` الموحّد.
---

:::примечание
Был установлен `docs/source/sorafs/developer/cli.md`. Он был создан в честь Сфинкса.
:::

Добавлено `sorafs_cli` (находится в ящике `sorafs_car` в контейнере `cli`) Установите флажок SoraFS. استخدم هذا الكتيب للانتقال مباشرةً إلى مسارات العمل الشائعة؛ Он выступил в роли президента США в Вашингтоне تشغيلي.

## تغليف الحمولات

استخدم `car pack` لإنتاج أرشيفات CAR и куски . Он был создан Чанкером SF-1 в Лос-Анджелесе в Нью-Йорке.

```bash
sorafs_cli car pack \
  --input fixtures/video.mp4 \
  --car-out artifacts/video.car \
  --plan-out artifacts/video.plan.json \
  --summary-out artifacts/video.car.json
```

- Код чанка: `sorafs.sf1@1.0.0`.
- Проверьте контрольные суммы, чтобы проверить контрольные суммы.
- Обработка дайджестов JSON в виде дайджестов и фрагментов CID для обработки данных. والمُنسِّق.

## بناء المانيفستات

```bash
sorafs_cli manifest build \
  --summary artifacts/video.car.json \
  --pin-min-replicas 4 \
  --pin-storage-class hot \
  --pin-retention-epoch 96 \
  --manifest-out artifacts/video.manifest.to \
  --manifest-json-out artifacts/video.manifest.json
```

- Установите `--pin-*` на `PinPolicy` и `sorafs_manifest::ManifestBuilder`.
- `--chunk-plan` вызывается в CLI для обработки дайджеста SHA3 фрагмента фрагмента данных. Он подготовил дайджест журнала «The الملخص».
- Вы можете просмотреть JSON-файл Norito, чтобы просмотреть различия в настройках.

## Он сказал:

```bash
sorafs_cli manifest sign \
  --manifest artifacts/video.manifest.to \
  --bundle-out artifacts/video.manifest.bundle.json \
  --signature-out artifacts/video.manifest.sig \
  --identity-token-env SIGSTORE_ID_TOKEN
```

- Он выступил в роли Рэйчела в фильме "Пенсильвания", а также в фильме "Самоооооооо".
- Выполняется запуск (`token_source`, `token_hash_hex`, дайджест фрагмента) с помощью JWT, когда он находится в исходном состоянии. `--include-token=true`.
— Создан в CI: ادمجه مع OIDC в GitHub Actions عبر ضبط `--identity-token-provider=github-actions`.

## إرسال المانيفستات إلى Torii

```bash
sorafs_cli manifest submit \
  --manifest artifacts/video.manifest.to \
  --chunk-plan artifacts/video.plan.json \
  --torii-url https://gateway.example/v1 \
  --authority ih58... \
  --private-key ed25519:0123...beef \
  --alias-namespace sora \
  --alias-name video::launch \
  --alias-proof fixtures/alias_proof.bin \
  --summary-out artifacts/video.submit.json
```

- يجري فك ترميز Norito псевдоним ويتحقق в дайджесте المانيفست قبل POST. Я18NT00000006X.
- Выполняется дайджест SHA3 фрагмента для последующего просмотра.
- Откройте веб-сайт HTTP и нажмите кнопку «Скрыть».

## التحقق من محتوى CAR والأدلة

```bash
sorafs_cli proof verify \
  --manifest artifacts/video.manifest.to \
  --car artifacts/video.car \
  --summary-out artifacts/video.verify.json
```

- В журнале PoR «Нью-Йорк» дайджесты последних событий.
- Воспользуйтесь услугами, которые вы можете получить, чтобы получить информацию о том, как это сделать. حوكمة.

## بث تليمترية الأدلة

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

- Написано NDJSON Лили Уилсоном Брэнсоном (написано на сайте `--emit-events=false`).
- Он был выбран в качестве кандидата на пост президента США в 2017 году. Создает JSON-файл, который позволяет получить доступ к файлам в формате JSON.
- Он сказал, что Сэнсэй Трэйнс в Вашингтоне, и Сэнсэй Трэйс. Используйте его для PoR (`--por-root-hex`). Установите флажок `--max-failures` и `--max-verification-failures`.
- يدعم PoR حاليًا؛ PDP и PoTR используют SF-13/SF-14.
- يكتب `--governance-evidence-dir` Запустите программу CLI, URL-адрес URL-адреса (дайджест) Он сказал, что он хочет, чтобы он сделал это.

## مراجع إضافية- `docs/source/sorafs_cli.md` — توثيق شامل للأعلام.
- `docs/source/sorafs_proof_streaming.md` — установите флажок Grafana.
- `docs/source/sorafs/manifest_pipeline.md` — используется для разбиения на части автомобильного диска CAR.