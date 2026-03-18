---
lang: ar
direction: rtl
source: docs/portal/docs/devportal/incident-runbooks.pt.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# دفاتر الأحداث وتدريبات التراجع

## اقتراح

العنصر الخاص بخريطة الطريق **DOCS-9** يتطلب إنشاء كتب لعب أكثر من مجرد خطة للتعلم من أجلك
يقوم مشغلو البوابة باستعادة المراسلات دون إرسالها. Esta nota cobre tres
حوادث عالية الخطية - تنشر falhos، وتدهور النسخ المتماثلة، وعدد من التحليلات - e
توثيق التدريبات الثلاثة التي تثبت التراجع عن الاسم المستعار والتحقق من صحته
استمرار العمل من النهاية إلى النهاية.

### مادة ذات صلة

- [`devportal/deploy-guide`](./deploy-guide) - سير عمل التغليف والتوقيع والترويج للاسم المستعار.
- [`devportal/observability`](./observability) - علامات الإصدار والتحليلات والتحقيقات المرجعية.
-`docs/source/sorafs_node_client_protocol.md`
  ه [`sorafs/pin-registry-ops`](../sorafs/pin-registry-ops)
  - قياس عن بعد للسجل وحدود التصعيد.
- `docs/portal/scripts/sorafs-pin-release.sh` والمساعدين `npm run probe:*`
  قوائم مرجعية مرجعية.

### أدوات القياس والقياس عن بعد

| سينال / أداة | اقتراح |
| ------------- | ------- |
| `torii_sorafs_replication_sla_total` (تم اللقاء/الفائت/معلق) | اكتشاف حجب النسخ المتماثل وانتهاكات SLA. |
| `torii_sorafs_replication_backlog_total`، `torii_sorafs_replication_completion_latency_epochs` | مقدار عمق الأعمال المتراكمة وتأخر إكمالها للفرز. |
| `torii_sorafs_gateway_refusals_total`، `torii_sorafs_manifest_submit_total{status="error"}` | معظمهم من البوابة التي تتبعها بشكل متكرر لنشرها. |
| `npm run probe:portal` / `npm run probe:tryit-proxy` | المسابر الاصطناعية التي تقوم بالإصدارات والتراجع عن عمليات التحقق. |
| `npm run check:links` | بوابة الروابط quebrados. usado apos cada mitigacao. |
| `sorafs_cli manifest submit ... --alias-*` (يُستخدم بواسطة `scripts/sorafs-pin-release.sh`) | آلية الترويج/عكس الاسم المستعار. |
| لوحة `Docs Portal Publishing` Grafana (`dashboards/grafana/docs_portal.json`) | جمع الرفض عن بعد/الاسم المستعار/TLS/النسخ المتماثل. تنبيهات مرجع PagerDuty هذه مؤلمة كدليل. |

## Runbook - نشر falho ou artefato ruim

### أغطية التوزيع

- مجسات المعاينة/منتجة فالهام (`npm run probe:portal -- --expect-release=...`).
- التنبيهات Grafana em `torii_sorafs_gateway_refusals_total` ou
  `torii_sorafs_manifest_submit_total{status="error"}` للطرح.
- دليل ضمان الجودة، ملاحظة تدور حول quebradas أو falhas do proxy، جربه فورًا
  الاسم المستعار الترويجي.

### المحتوى الفوري

1. **تنشر Congelar:** ماركر o خط أنابيب CI com `DEPLOY_FREEZE=1` (إدخال سير العمل
   GitHub) أو أوقف وظيفة Jenkins حتى لا يتم إرسالها إليك.
2. **التقاط القطع الأثرية:** بايكسار `build/checksums.sha256`,
   `portal.manifest*.{json,to,bundle,sig}`، وهي عبارة عن تحقيقات مبنية على أساس falha para que
   o مرجع التراجع عن هضم exatos.
3. **إخطار أصحاب المصلحة:** تخزين SRE، والمستندات الرئيسية/DevRel، والمسؤول المناوب
   إدارة الوعي (خاصة عندما يكون `docs.sora` مؤثرًا).

### إجراء التراجع

1. تحديد أو بيان آخر سلعة معروفة (LKG). سير العمل في إنتاج المخزن
   `artifacts/devportal/<release>/sorafs/portal.manifest.to`.
2. إعادة تسمية الاسم المستعار إلى هذا البيان عبر مساعد الشحن:

```bash
cd docs/portal
./scripts/sorafs-pin-release.sh \
  --build-dir build \
  --artifact-dir artifacts/revert-$(date +%Y%m%d%H%M) \
  --sorafs-dir artifacts/revert-$(date +%Y%m%d%H%M)/sorafs \
  --pin-min-replicas 5 \
  --alias "docs-prod-revert" \
  --alias-namespace "${PIN_ALIAS_NAMESPACE}" \
  --alias-name "${PIN_ALIAS_NAME}" \
  --alias-proof "${PIN_ALIAS_PROOF_PATH}" \
  --torii-url "${TORII_URL}" \
  --submitted-epoch "$(date +%Y%m%d)" \
  --authority "${AUTHORITY}" \
  --private-key "${PRIVATE_KEY}" \
  --skip-submit

# swap in the LKG artefacts before submission
cp /secure/archive/lkg/portal.manifest.to artifacts/.../sorafs/portal.manifest.to
cp /secure/archive/lkg/portal.manifest.bundle.json artifacts/.../sorafs/

cargo run -p sorafs_orchestrator --bin sorafs_cli -- \
  manifest submit \
  --manifest artifacts/.../sorafs/portal.manifest.to \
  --chunk-plan artifacts/.../sorafs/portal.plan.json \
  --torii-url "${TORII_URL}" \
  --authority "${AUTHORITY}" \
  --private-key "${PRIVATE_KEY}" \
  --alias-namespace "${PIN_ALIAS_NAMESPACE}" \
  --alias-name "${PIN_ALIAS_NAME}" \
  --alias-proof "${PIN_ALIAS_PROOF_PATH}" \
  --metadata rollback_from="${FAILED_RELEASE}" \
  --summary-out artifacts/.../sorafs/rollback.submit.json
```

3. سجل أو استكمل التراجع عن أي تذكرة يتم القيام بها جنبًا إلى جنب مع الملخصات
   بيان LKG ه القيام بيان كوم falha.

### فاليداكاو

1.`npm run probe:portal -- --expect-release=${LKG_TAG}`.
2.`npm run check:links`.
3. `sorafs_cli manifest verify-signature ...` و`sorafs_cli proof verify ...`
   (veja o guia deploy) لتأكيد استمرار البيان المكرر
   Batendo com o arquivado.
4.`npm run probe:tryit-proxy` لضمان تشغيل الوكيل لـ Try-It.

### حادث محتمل

1. خط أنابيب مباشر للنشر فقط بعد أن يؤدي إلى زيادة.
2. مقدمة كمدخلات "الدروس المستفادة" في [`devportal/deploy-guide`](./deploy-guide)
   com novos pontos، إذا كان الأمر كذلك.
3. عيوب العبر لمجموعة الخصيتين (مسبار، مدقق الارتباط، وما إلى ذلك).

## Runbook - Degradacao de Replicacao

### أغطية التوزيع

- تنبيه: `sum(torii_sorafs_replication_sla_total{outcome="met"}) /
  المشبك_مين(sum(torii_sorafs_replication_sla_total{outcome=~"met|missed"}), 1) <
  0.95` لمدة 10 دقائق.
- `torii_sorafs_replication_backlog_total > 10` لمدة 10 دقائق (تم التحقق منه
  `pin-registry-ops.md`).
- تقرير Governanca الاسم المستعار lento apos um Release.

### الفرز

1. فحص لوحات المعلومات [`sorafs/pin-registry-ops`](../sorafs/pin-registry-ops) للفقرة
   قم بتأكيد ما إذا كانت الأعمال المتراكمة مترجمة إلى فئة تخزين أو إلى أسطول من مقدمي الخدمة.
2. تقوم سجلات Cruze بعمل Torii من خلال التحذيرات `sorafs_registry::submit_manifest` لتحديدها
   كما التقديمات estao falhando.
3. Amostre a saude das النسخ المتماثلة عبر `sorafs_cli manifest status --manifest ...` (القائمة
   النتائج من قبل الموفر).

### ميتيجاكاو

1. قم باسترداد البيان باستخدام أكبر عدد من النسخ المتماثلة (`--pin-min-replicas 7`)
   `scripts/sorafs-pin-release.sh` لجدولة توزيع البضائع على مجموعة أكبر
   دي مقدمي الخدمات. قم بالتسجيل في الملخص الجديد دون تسجيل أي حادث.
2. إذا كانت الأعمال المتراكمة ستتوفر لموفر واحد، فسيتم تعطيلها مؤقتًا عبر o
   جدولة النسخ المتماثل (موثقة في `pin-registry-ops.md`) وأرسلها مرة أخرى
   بيان عن موفري الخدمات الآخرين لتحديث الاسم المستعار.
3. عندما تقوم بعمل جدارية باسم مستعار لمزيد من النقد لجدار النسخ، قم بإعادة vincule o
   الاسم المستعار للبيان الذي تم إعداده (`docs-preview`)، بعد نشر البيان
   مرافقة عندما أو SRE محو أو تراكم.

### استعادة البيانات واستردادها

1. الشاشة `torii_sorafs_replication_sla_total{outcome="missed"}` لضمان ذلك
   يستقر الكونتادور.
2. التقط صورة `sorafs_cli manifest status` كدليل على كل نسخة طبق الأصل
   امتثال.
3. قم بتشغيل أو تحديث النسخ المتماثلة بعد الوفاة مع الخطوات التالية
   (توسيع نطاق موفري الخدمة، وضبط القطع، وما إلى ذلك).

## Runbook - مجموعة من التحليلات أو القياس عن بعد

### أغطية التوزيع

- `npm run probe:portal` passa، ولكن لوحات المعلومات هي المعلمة التي تقوم بإدخال الأحداث
  `AnalyticsTracker` لمدة > 15 دقيقة.
- مراجعة الخصوصية لزيادة الأحداث التي تم حذفها.
- `npm run probe:tryit-proxy` فالها إم المسارات `/probe/analytics`.

### الرد

1. التحقق من مدخلات البناء: `DOCS_ANALYTICS_ENDPOINT` ه
   `DOCS_ANALYTICS_SAMPLE_RATE` لم يتم إصدار artefato (`build/release.json`).
2. أعد تنفيذ `npm run probe:portal` مع `DOCS_ANALYTICS_ENDPOINT` للمتابعة
   جامع التدريج لتأكيد أن المتعقب سيصدر حمولات.
3. إذا توقف المجمعون عن العمل، حدد `DOCS_ANALYTICS_ENDPOINT=""` وأعد البناء
   لمنع حدوث ماس كهربائي في جهاز التعقب؛ قم بتسجيل الدخول إلى انقطاع التيار الكهربائي
   هل وتيرة الحادث.
4. Valide que `scripts/check-links.mjs` ainda faz بصمة الإصبع de `checksums.sha256`
   (يقوم هذا التحليل *nao* بحظر التحقق من صحة خريطة الموقع).
5. عند تجميع الجهد، ركب `npm run test:widgets` لممارسة اختبارات وحدة نظام التشغيل
   هل يساعد في التحليلات قبل إعادة النشر.

### حادث محتمل

1. تحديث [`devportal/observability`](./observability) مع أداة التجميع الجديدة
   أو متطلبات amostragem.
2. احصل على نصيحة إدارية بشأن بيانات التحليلات المتعلقة بالخسائر أو المحذوفات في المنتديات
   دا سياسة.

## تدريبات ثلاثية الأبعاد على المرونة

قم بتنفيذ التدريبات الأساسية خلال **البداية الثالثة من كل ثلاثة أشهر** (يناير/أبريل/يوليو/خارج)
أو على الفور مع أي بنية تحتية أكبر. أرمازين أرتيفاتوس إم
`artifacts/devportal/drills/<YYYYMMDD>/`.

| حفر | باسوس | ايفيدنسيا |
| ----- | ----- | -------- |
| Ensaio de rollback de alias | 1. كرر التراجع عن "Deploy falho" باستخدام بيان الإنتاج الأحدث.<br/>2. أعد إنتاج المنتج حتى يتمكن من تجاوزه.<br/>3. المسجل `portal.manifest.submit.summary.json` وسجلات المسبار في معكرونة الحفر. | `rollback.submit.json`، أداة التحقيق، علامة الإصدار الإلكتروني. |
| Audittoria de validacao sintetica | 1. قضيب `npm run probe:portal` و`npm run probe:tryit-proxy` ضد الإنتاج والتدريج.<br/>2. قضيب `npm run check:links` وفتح `build/link-report.json`.<br/>3. Anexar Screenshots/Exports de Paineis Grafana يؤكد نجاح المسبارين. | تظهر سجلات المسابير + `link-report.json` المرجعية أو بصمة الإصبع. |

قم بتصعيد التدريبات على الخسارة لمدير Docs/DevRel ومراجعة إدارة SRE،
ما هي خريطة الطريق التي تتطلب أدلة ثلاثية تحدد ما إذا كان التراجع عن الاسم المستعار ونظام التشغيل
تحقيقات تفعل بوابة مستمرة saudaveis.

## Coordenacao PagerDuty e عند الطلب

- خدمة PagerDuty **Docs Portal Publishing** وإرسال التنبيهات الجيدة من
  `dashboards/grafana/docs_portal.json`. كما أعيد `DocsPortal/GatewayRefusals`،
  `DocsPortal/AliasCache`، و`DocsPortal/TLSExpiry` الصفحة الرئيسية لـ Docs/DevRel
  com Storage SRE como secundario.
- عند الضغط على الصفحة، بما في ذلك `DOCS_RELEASE_TAG`، قم بإرفاق لقطات الشاشة مع Grafana
  الصور والربط بين مسبار التحقيق/التحقق من الارتباط في ملاحظاتك قبل البدء
  mitigacao.
- بعد التخفيف (التراجع أو إعادة النشر)، أعد تنفيذ `npm run probe:portal`،
  `npm run check:links`، التقاط اللقطات Grafana يقوم بتحديث أحدث اللقطات كمقاييس
  عتبات دي فولتا آوس. قم بإرفاق جميع الأدلة المتعلقة بحادثة PagerDuty
  قبل الحل.
- إذا تم إلغاء التنبيهات في نفس الوقت (على سبيل المثال، تنتهي صلاحية TLS أكثر تراكمًا)،
  عمليات الرفض الأولية (مثل النشر)، أو تنفيذ إجراء التراجع، ه
  بعد حل TLS/backlog com Storage SRE na ponte.