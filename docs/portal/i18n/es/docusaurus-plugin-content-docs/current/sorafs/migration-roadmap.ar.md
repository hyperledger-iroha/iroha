---
lang: es
direction: ltr
source: docs/portal/docs/sorafs/migration-roadmap.ar.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
título: "خارطة طريق ترحيل SoraFS"
---

> مقتبس من [`docs/source/sorafs/migration_roadmap.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sorafs/migration_roadmap.md).

# خارطة طريق ترحيل SoraFS (SF-1)

هذا المستند يُحوِّل إرشادات الترحيل الموثقة في
`docs/source/sorafs_architecture_rfc.md` إلى خطة تشغيلية. يوسِّع مخرجات SF-1 إلى
معالم جاهزة للتنفيذ ومعايير بوابة وقوائم تحقق للمالكين حتى تمكن فرق التخزين
والحوكمة وDevRel وSDK من تنسيق الانتقال من استضافة artefactos القديمة إلى نشر
Utilice SoraFS.

خارطة الطريق حتمية عمدا: كل معلم يسمي artefactos المطلوبة واستدعاءات الأوامر وخطوات
الاستيثاق حتى تنتج خطوط الأنابيب اللاحقة مخرجات متطابقة وتحافظ الحوكمة على أثر
قابل للتدقيق.

## نظرة عامة على المعالم

| المعلم | النافذة | الأهداف الأساسية | ما يجب تسليمه | المالكون |
|--------|---------|------------------|----------------|----------|
| **M1 - Aplicación determinista** | الأسابيع 7-12 | فرض accesorios موقعة وتجهيز إثباتات alias بينما تعتمد خطوط الأنابيب banderas de expectativa. | تحقق ليلي من accesorios, manifiestos موقعة من المجلس، إدخالات puesta en escena في سجل alias. | Almacenamiento, Gobernanza, SDK |

يتم تتبع حالة المعالم في `docs/source/sorafs/migration_ledger.md`. كل تغيير في هذه
الخارطة يجب أن يحدِّث السجل ليبقى الحوكمة وهندسة الإصدارات على نفس النسق.

## مسارات العمل

### 2. اعتماد fijando الحتمي| الخطوة | المعلم | الوصف | المالك(ون) | المخرجات |
|--------|--------|-------|------------|----------|
| accesorios de تدريبات | M0 | Ejecuciones en seco de un resumen del fragmento de `fixtures/sorafs_chunker`. نشر التقرير تحت `docs/source/sorafs/reports/`. | Proveedores de almacenamiento | `determinism-<date>.md` La prueba pasa/no pasa. |
| فرض التواقيع | M1 | `ci/check_sorafs_fixtures.sh` + `.github/workflows/sorafs-fixtures-nightly.yml` تفشل إذا انحرفت التواقيع أو manifiestos. anula la exención de التطوير تتطلب من الحوكمة مرفق بالـ PR. | Grupo de Trabajo sobre Herramientas | سجل CI، رابط تذكرة waiver (إن وجدت). |
| Banderas de expectativa | M1 | خطوط الأنابيب تستدعي `sorafs_manifest_stub` بتوقعات صريحة لتثبيت المخرجات: | Documentos CI | سكربتات محدثة تشير إلى indicadores de expectativa (انظر كتلة الأمر أدناه). |
| Fijación de registro primero | M2 | `sorafs pin propose` y `sorafs pin approve` manifiestan el manifiesto CLI الافتراضي يستخدم `--require-registry`. | Operaciones de gobernanza | Utilice el registro CLI para acceder a los archivos. |
| تكافؤ observabilidad | M3 | Los programas Prometheus/Grafana utilizan fragmentos de manifiestos del registro التنبيهات موصولة بمناوبة ops. | Observabilidad | رابط لوحة، IDs قواعد التنبيه، نتائج GameDay. |

#### أمر النشر القياسي

```bash
cargo run -p sorafs_manifest --bin sorafs_manifest_stub -- docs/book \
  --manifest-out artifacts/docs/book/2025-11-01/docs.manifest \
  --manifest-signatures-out artifacts/docs/book/2025-11-01/docs.manifest_signatures.json \
  --car-out artifacts/docs/book/2025-11-01/docs.car \
  --chunk-fetch-plan-out artifacts/docs/book/2025-11-01/docs.fetch_plan.json \
  --car-digest=13fa919c67e55a2e95a13ff8b0c6b40b2e51d6ef505568990f3bc7754e6cc482 \
  --car-size=429391872 \
  --root-cid=f40101... \
  --dag-codec=0x71
```

استبدل قيم digest والحجم و CID بالمراجع المتوقعة المسجلة في إدخال سجل الترحيل
للـ artefacto.

### 3. انتقال alias والاتصالات| الخطوة | المعلم | الوصف | المالك(ون) | المخرجات |
|--------|--------|-------|------------|----------|
| إثباتات alias في puesta en escena | M1 | تسجيل مطالبات alias في Pin Registry الخاص بـ staging and إثباتات Merkle مع manifests (`--alias`). | Gobernanza, Documentos | paquete إثباتات مخزن بجوار manifiesto + تعليق في السجل باسم alias. |
| فرض الإثباتات | M2 | Manifestaciones de puertas de enlace بدون رؤوس `Sora-Proof` حديثة؛ CI يضيف خطوة `sorafs alias verify` لجلب الإثباتات. | Redes | تصحيح إعدادات gateway + مخرجات CI توثق التحقق الناجح. |

### 4. الاتصالات والتدقيق

- **انضباط السجل:** كل تغيير حالة (desviación de accesorios, registro de registro, alias de تفعيل) يجب أن يضيف
  ملاحظة مؤرخة في `docs/source/sorafs/migration_ledger.md`.
- **محاضر الحوكمة:** جلسات المجلس التي تعتمد تغييرات Pin Registro أو سياسات alias يجب أن تشير
  إلى هذه الخارطة والسجل معا.
- **الاتصالات الخارجية:** DevRel ينشر تحديثات الحالة عند كل معلم (مدونة + مقتطف changelog)
  مع إبراز الضمانات الحتمية وجداول alias الزمنية.

## التبعيات والمخاطر| التبعية | الأثر | التخفيف |
|---------|-------|---------|
| توفر عقد Registro de PIN | Aquí despliegue el pin M2 primero. | تجهيز العقد قبل M2 مع اختبارات repetición Hay un respaldo de sobre y una regresión. |
| مفاتيح توقيع المجلس | مطلوبة لـ sobres de manifiesto y registro. | Adaptador de corriente `docs/source/sorafs/signing_ceremony.md`؛ تدوير المفاتيح بتداخل وتدوين ذلك في السجل. |
| إيقاع إصدارات SDK | يجب على العملاء احترام إثباتات alias قبل M3. | مواءمة نوافذ إصدار SDK مع بوابات المعالم؛ Listas de verificación de إضافة للترحيل في قوالب الإصدار. |

Nombre del producto: `docs/source/sorafs_architecture_rfc.md`
ويجب الرجوع إليها عند إجراء تعديلات.

## قائمة تحقق معايير الخروج

| المعلم | المعايير |
|--------|----------|
| M1 | - Accesorios para el hogar الليلية خضراء لسبعة أيام متتالية.  - تحقق إثباتات alias في puesta en escena de CI.  - الحوكمة تصادق على سياسة banderas de expectativa. |

## إدارة التغيير

1. اقترح التعديلات عبر PR يقوم بتحديث هذا الملف **و**
   `docs/source/sorafs/migration_ledger.md`.
2. اربط محاضر الحوكمة وأدلة CI في وصف الـ PR.
3. Utilice el almacenamiento de datos + DevRel para almacenar y almacenar archivos.

اتباع هذا الإجراء يضمن أن rollout SoraFS يبقى حتميا وقابلا للتدقيق وشفافا عبر
الفرق المشاركة في إطلاق Nexus.