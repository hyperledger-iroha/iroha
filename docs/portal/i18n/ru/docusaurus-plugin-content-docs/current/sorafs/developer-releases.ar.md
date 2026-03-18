---
lang: ru
direction: ltr
source: docs/portal/docs/sorafs/developer-releases.ar.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
Название: عملية الإصدار
Краткое описание: Доступен интерфейс CLI/SDK, а также возможность изменения настроек. الإصدار المعتمدة.
---

# عملية الإصدار

Добавление SoraFS (`sorafs_cli`, `sorafs_fetch`, помощники) в SDK
(`sorafs_car`, `sorafs_manifest`, `sorafs_chunker`). يحافظ خط إصدار واحد على
Запустите CLI, выполните lint/test и выполните следующие действия:
اللاحقين. Он был убит в 1998 году в Лос-Анджелесе.

## 0. تأكيد اعتماد مراجعة الأمان

В фильме, опубликованном в журнале "Страна", говорится:

- نزّل أحدث مذكرة مراجعة SF-6 ([reports/sf6-security-review](./reports/sf6-security-review.md))
  Подключите SHA256 для подключения к сети.
- أرفق رابط تذكرة المعالجة (مثل `governance/tickets/SF6-SR-2026.md`)
  Создан в Рабочей группе по разработке инструментов для инженеров безопасности.
- تحقّق в إغلاق قائمة المعالجة في المذكرة؛ Сделайте это в ближайшее время.
- Дополнительный ремень безопасности (`cargo test -p sorafs_car -- --nocapture sorafs_cli::proof_stream::bounded_channels`)
  Он сказал это.
- تأكد أن أمر التوقيع الذي ستنفذه يتضمن `--identity-token-provider` و
  `--identity-token-audience=<aud>` был вызван Фульсио в 2007 году.

Он был убит в 2007 году в 1997 году.

## 1. تنفيذ بوابة الإصدار/الاختبارات

يشغّل المساعد `ci/check_sorafs_cli_release.sh` التنسيق и Clippy والاختبارات عبر
Используйте CLI и SDK для целевой версии файла (`.target`).
Он был создан в честь Дэна CI.

```bash
CARGO_TARGET_DIR=.target ci/check_sorafs_cli_release.sh
```

В сообщении говорится:

- `cargo fmt --all -- --check` (рабочая область)
- `cargo clippy --locked --all-targets` вместо `sorafs_car` (название `cli`),
  و`sorafs_manifest` و`sorafs_chunker`
- `cargo test --locked --all-targets` لتلك الحزم نفسها

Он был убит в 1980-х годах в Вашингтоне. Он и Сан-Франциско
الإصدار متصلة بـ main; Он выбрал вишню, которую он выбрал для себя.
Он был создан в 2008 году в Вашингтоне, штат Нью-Йорк, США (`--identity-token-issuer`,
`--identity-token-audience`) حيث يلزم؛ تؤدي الحجج المفقودة فشل التشغيل.

## 2. تطبيق سياسة الإصدارات

Загрузите CLI/SDK для SoraFS и выберите SemVer:

- `MAJOR`: Установлена версия 1.0. Версия 1.0 была создана на базе `0.y`.
  **Зарегистрируйтесь** в интерфейсе командной строки Norito.
- `MINOR`: Добавление кода Norito جديدة خلف سياسة
  اختيارية, إضافات تليمترية).
- `PATCH`: إصلاحات عيوب, وإصدارات وثائق فقط, وتحديثات تبعيات لا تغيّر. السلوك
  الملاحظ.

حافظ دائمًا على نسخ `sorafs_car` و`sorafs_manifest` و`sorafs_chunker` متطابقة كي
Используйте SDK для создания приложений. Ответ на вопрос:

1. Установите `version =` или `Cargo.toml`.
2. Установите флажок `Cargo.lock` и установите `cargo update -p <crate>@<new-version>` (отсутствует).
   مساحة العمل نسخًا صريحة).
3. Он сказал:

## 3. إعداد ملاحظات الإصدار

Доступ к интерфейсу командной строки и SDK для использования с Markdown, а также с помощью CLI и SDK.
حوكمة. استخدم القالب في `docs/examples/sorafs_release_notes.md` (انسخه إلى
Он был главой государства и главой государства Бранденбургом).

Сообщение от автора:- **Дополнительно**: добавлена ​​возможность использования CLI и SDK.
- **Уведомления**: Дэниел Кейнс, ترقيات السياسات, المتطلبات الدنيا للبوابة/العقدة.
- **Доступно для перевозки**: أوامر TL;DR لتحديث تبعيات charge وإعادة تشغيل приспособления الحتمية.
- **التحقق**: تجزئات مخرجات الأوامر أو الأظرف والإصدار الدقيق لـ
  `ci/check_sorafs_cli_release.sh` находится в центре внимания.

Создан на сайте DB (на сайте GitHub) и опубликован на сайте GitHub.
الآرتيفاكتات المولدة حتميًا.

## 4. تنفيذ خطافات الإصدار

شغّل `scripts/release_sorafs_cli.sh` для дальнейшего использования.
Он был в Уилсоне. Воспользуйтесь интерфейсом командной строки, чтобы настроить его.
`sorafs_cli manifest sign`, а также `manifest verify-signature`.
لتظهر الإخفاقات قبل وضع الوسم. Название:

```bash
scripts/release_sorafs_cli.sh \
  --manifest artifacts/site.manifest.to \
  --chunk-plan artifacts/site.chunk_plan.json \
  --chunk-summary artifacts/site.car.json \
  --bundle-out artifacts/release/manifest.bundle.json \
  --signature-out artifacts/release/manifest.sig \
  --identity-token-provider=github-actions \
  --identity-token-audience=sorafs-release \
  --expect-token-hash "$(cat .release/token.hash)"
```

Ответ:

- تتبع مدخلات الإصدار (الحمولة، الخطط، الملخصات, تجزئة الرمز المتوقعة) داخل
  Он был отправлен в Лос-Анджелес в 1997 году. تُظهر حزمة
  светильники `fixtures/sorafs_manifest/ci_sample/`.
- Доступ к CI `.github/workflows/sorafs-cli-release.yml`; в фильме «Тренер Сон»
  Создайте рабочий процесс для рабочего процесса.
  Дэниел Уилсон Нэнси (переговоры → Убийства → Убийства) в CI أخرى
  Он сказал, что хочет, чтобы это произошло.
- احتفظ بالملفات `manifest.bundle.json` و`manifest.sig` و`manifest.sign.summary.json`
  و`manifest.verify.summary.json` создано — в 2017 году в 2017 году.
- Запланированные светильники для просмотра и просмотра фрагментов
  والملخصات إلى `fixtures/sorafs_manifest/ci_sample/` (وحدّث
  `docs/examples/sorafs_ci_sample/manifest.template.json`) قبل وضع الوسم.
  Накануне в Лас-Вегасе были зафиксированы матчи, проведенные в Манчестере.
- التقط سجل تشغيل تحقق قنوات `sorafs_cli proof stream` ذات الحدود وأرفقه بحزمة
  الإصدار لإثبات أن ضمانات доказательство потоковой передачи لا تزال نشطة.
- Установите флажок `--identity-token-audience` для получения дополнительной информации о том, как это сделать.
  إصدار؛ Он был убит в Сан-Франциско в Сан-Франциско Фульчо в Сан-Франциско.

استخدم `scripts/sorafs_gateway_self_cert.sh` عندما يحمل الإصدار أيضًا إطلاقًا
للبوابة. Он был выбран в качестве посредника в работе над проектом. Сообщение:

```bash
scripts/sorafs_gateway_self_cert.sh --config docs/examples/sorafs_gateway_self_cert.conf \
  --manifest artifacts/site.manifest.to \
  --manifest-bundle artifacts/release/manifest.bundle.json
```

## 5. وضع الوسم والنشر

В статье говорится:1. Установите `sorafs_cli --version` и `sorafs_fetch --version` для проверки работоспособности.
   الإصدار الجديد.
2. Установите флажок для `sorafs_release.toml`, чтобы установить флажок (مفضّل) أو
   Он был убит в 2007 году. تجنّب الاعتماد على متغيرات بيئة عشوائية; مرّر
   Доступ к CLI в `--config` (необходимо для проверки) وقابلة
   لإعادة الإنتاج.
3. أنشئ وسمًا موقّعًا (مفضّل) и وسمًا مُعلّقًا:
   ```bash
   git tag -s sorafs-vX.Y.Z -m "SoraFS CLI & SDK vX.Y.Z"
   git push origin sorafs-vX.Y.Z
   ```
4. Автомобиль для автомобиля (CAR, автомобильный салон, автомобильный салон). مخرجات
   Например) Установите флажок, чтобы получить доступ к [دليل النشر](./developer-deployment.md).
   ДАТА САЛАРИА - расписание матчей ДАИА Йулла Месси - расписание матчей ИГИЛ и МАФА.
   В 2007 году он был убит в 1980-х годах в Нью-Йорке.
5. أخطر قناة الحوكمة بروابط الوسم الموقّع, وملاحظات الإصدار, وتجزئات حزمة المانيفست/
   Установите флажок `manifest.sign/verify` и установите его. أضف رابط وظيفة CI
   (أو أرشيف السجلات) التي شغّلت `ci/check_sorafs_cli_release.sh` و
   `scripts/release_sorafs_cli.sh`. Он был создан в Лос-Анджелесе в 2007 году.
   الموافقات إلى الآرتيفاكتات؛ وعندما يرسل job `.github/workflows/sorafs-cli-release.yml`
   Он сказал, что хочет, чтобы это произошло.

## 6. ما بعد الإصدار

- Он был показан в фильме "Триумф" в Колумбийском университете (США)
  Он был убит в 1980-х годах.
- أضف عناصر إلى خارطة الطريق إذا لزم لاحق (مثل أعلام الترحيل أو إيقاف)
  المانيفستات القديمة).
- أرشف سجلات مخرجات بوابة الإصدار للمدققين — Нэнси Бэнкэн Сити الموقعة.

Доступ к интерфейсу CLI и SDK, а также к удаленному использованию данных. إصدار.