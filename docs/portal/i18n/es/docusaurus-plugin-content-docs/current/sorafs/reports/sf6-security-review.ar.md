---
lang: es
direction: ltr
source: docs/portal/docs/sorafs/reports/sf6-security-review.ar.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
título: مراجعة أمان SF-6
resumen: النتائج وعناصر المتابعة من التقييم المستقل للتوقيع بدون مفاتيح، وبث الإثباتات، وخطوط إرسال الـ manifiestos.
---

# مراجعة أمان SF-6

**نافذة التقييم:** 2026-02-10 → 2026-02-18  
**قادة المراجعة:** Gremio de ingeniería de seguridad (`@sec-eng`), Grupo de trabajo de herramientas (`@tooling-wg`)  
**النطاق:** SoraFS CLI/SDK (`sorafs_cli`, `sorafs_car`, `sorafs_manifest`), y transmisión de prueba, manifiestos de Torii تكامل Sigstore/OIDC, y la versión de CI.  
**القطع الفنية:**  
- CLI y pantalla táctil (`crates/sorafs_car/src/bin/sorafs_cli.rs`)  
- Manifiesto/prueba de معالجات في Torii (`crates/iroha_torii/src/sorafs/api.rs`)  
- Versión de lanzamiento (`ci/check_sorafs_cli_release.sh`, `scripts/release_sorafs_cli.sh`)  
- Arnés تساوي حتمي (`crates/sorafs_car/tests/sorafs_cli.rs`, [تقرير تكافؤ GA لمشغل SoraFS](./orchestrator-ga-parity.md))

## المنهجية1. **ورش modelado de amenazas** قامت بمواءمة قدرات المهاجمين لمحطات عمل المطورين وأنظمة CI وعقد Torii.  
2. **مراجعة الشيفرة** ركزت على أسطح الاعتمادات (تبادل رموز OIDC, firma sin llave), تحقق manifiestos de Norito, y contrapresión في transmisión de prueba.  
3. **اختبارات ديناميكية** Muestra manifiestos de dispositivos y pruebas de detección (repetición de tokens, manipulación de manifiestos, pruebas de flujos a prueba) con arnés de paridad y unidades fuzz. مخصصة.  
4. **فحص الإعدادات** تحقق من defaults في `iroha_config`, معالجة أعلام CLI، وسكربتات release لضمان تشغيلات حتمية وقابلة للتدقيق.  
5. **مقابلة عملية** أكدت تدفق remediación ومسارات التصعيد وجمع evidencia التدقيق مع propietarios الإطلاق في Tooling WG.

## ملخص النتائج| identificación | الشدة | المجال | النتيجة | المعالجة |
|----|----------|------|---------|------------|
| SF6-SR-01 | عالي | Firma sin llave | La audiencia de كانت الافتراضية لرموز OIDC ضمنية في قوالب CI، ما يعرض لخطر بين المستأجرين. | تمت إضافة فرض صريح لـ `--identity-token-audience` في ganchos الإطلاق وقوالب CI ([proceso de liberación](../developer-releases.md), `docs/examples/sorafs_ci.md`). أصبح CI يفشل عند غياب audiencia. |
| SF6-SR-02 | متوسط ​​| Transmisión de prueba | مسارات amortiguadores de contrapresión غير محدودة للمشتركين، ما يتيح استنزاف الذاكرة. | يفرض `sorafs_cli proof stream` أحجام قنوات محدودة مع truncamiento حتمي، ويسجل Norito resúmenes y ويجهض التدفق؛ También incluye fragmentos de respuesta de espejo Torii (`crates/iroha_torii/src/sorafs/api.rs`). |
| SF6-SR-03 | متوسط ​​| إرسال manifiesta | Los manifiestos CLI se encuentran en los planes de fragmentos que se encuentran en el archivo `--plan`. | `sorafs_cli manifest submit` incluye resúmenes de CAR y `--expect-plan-digest`, discrepancias y corrección de errores. تغطي الاختبارات حالات النجاح/الفشل (`crates/sorafs_car/tests/sorafs_cli.rs`). |
| SF6-SR-04 | منخفض | Pista de auditoría | كانت قائمة التحقق للإطلاق تفتقر إلى سجل موافقة موقع لمراجعة الأمان. | تمت إضافة قسم في [proceso de liberación](../developer-releases.md) يطلب إرفاق hashes لمذكرة المراجعة ورابط تذكرة sign-off قبل GA. |

تم إصلاح جميع نتائج alto/medio خلال نافذة المراجعة والتحقق منها عبر paridad arnés الحالي. لا توجد مشكلات حرجة كامنة.

## التحقق من الضوابط- **نطاق الاعتمادات:** تفرض قوالب CI الآن audiencia y emisor صريحين؛ La CLI y el asistente de liberación están conectados a `--identity-token-audience` y `--identity-token-provider`.  
- **إعادة التشغيل الحتمية:** تغطي الاختبارات المحدثة تدفقات إرسال manifests الإيجابية والسلبية، وتضمن أن resúmenes no coincidentes تبقى أخطاء غير حتمية وتُكتشف قبل لمس الشبكة.  
- **Transmisión a prueba de contrapresión:** Torii ببث عناصر PoR/PoTR عبر قنوات محدودة، ويحتفظ CLI فقط بعينات latencia مقطوعة + خمسة أمثلة فشل، مانعا نمو المشتركين غير المحدود مع الحفاظ على resúmenes حتمية.  
- **الرصد:** تلتقط عدادات streaming de prueba (`torii_sorafs_proof_stream_*`) y resúmenes CLI أسباب الإجهاض، مما يوفر migas de pan تدقيق للمشغلين.  
- **التوثيق:** تشير أدلة المطورين ([índice de desarrollador](../developer-index.md), [referencia CLI](../developer-cli.md)) إلى الأعلام الحساسة للأمان ومسارات التصعيد.

## إضافات إلى قائمة التحقق للإطلاق

**يجب** على مديري الإطلاق إرفاق الأدلة التالية عند ترقية مرشح GA:

1. Hash لآخر مذكرة مراجعة أمان (هذا المستند).  
2. Remediación de رابط لتذكرة المتابعة (مثل `governance/tickets/SF6-SR-2026.md`).  
3. مخرجات `scripts/release_sorafs_cli.sh --manifest ... --bundle-out ... --signature-out ...` التي تظهر معاملات audiencia/emisor صريحة.  
4. Cable de paridad del arnés (`cargo test -p sorafs_car -- --nocapture sorafs_cli::proof_stream::bounded_channels`).  
5. Utilice los contadores Torii para configurar los contadores de datos.

عدم جمع artefactos أعلاه يمنع aprobación لـ GA.

**Hashes مرجعية للقطع الفنية (cierre de sesión بتاريخ 2026-02-20):**

- `sf6_security_review.md` — `66001d0b53d8e7ed5951a07453121c075dea931ca44c11f1fcd1571ed827342a`

## متابعات معلقة- **تحديث نموذج التهديد:** إعادة هذه المراجعة كل ربع أو قبل إضافات كبيرة لأعلام CLI.  
- **تغطية fuzzing:** يتم fuzzing لتشفيرات نقل prueba de streaming عبر `fuzz/proof_stream_transport`, بما يشمل cargas útiles: identidad, gzip, deflate, zstd.  
- **تمرين الحوادث:** جدولة تمرين للمشغلين لمحاكاة اختراق token y rollback للـ manifest, مع ضمان أن التوثيق يعكس الإجراءات المطبقة.

## الاعتماد

- ممثل Gremio de Ingeniería de Seguridad: @sec-eng (2026-02-20)  
- Grupo de trabajo sobre herramientas: @tooling-wg (2026-02-20)

احفظ الموافقات الموقعة بجانب paquete القطع الفنية للإطلاق.