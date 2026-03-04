---
lang: pt
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
`docs/source/sorafs_architecture_rfc.md` é um problema. يوسِّع مخرجات SF-1
معالم جاهزة للتنفيذ ومعايير بوابة وقوائم تحقق للمالكين حتى تتمكن فرق التخزين
والحوكمة وDevRel وSDK من تنسيق الانتقال من استضافة artefatos قديمة إلى نشر
O problema é SoraFS.

خارطة الطريق حتمية عمدا: كل معلم يسمي artefatos المطلوبة واستدعاءات الأوامر وخطوات
Você pode fazer o download do seu telefone e fazer o download do seu cartão de crédito
قابل للتدقيق.

## نظرة عامة على المعالم

| المعلم | النافذة | الأهداف الأساسية | ما يجب تسليمه | المالكون |
|--------|---------|------------------|----------------|----------|
| **M1 - Aplicação Determinística** | Dia 7-12 | فرض fixtures موقعة وتجهيز إثباتات alias بينما تعتمد خطوط الأنابيب sinalizadores de expectativa. | تحقق ليلي من fixtures, manifests موقعة من المجلس, إدخالات staging في سجل alias. | Armazenamento, governança, SDKs |

Você pode fazer isso em `docs/source/sorafs/migration_ledger.md`. كل تغيير في هذه
الخارطة يجب أن يحدِّث السجل ليبقى الحوكمة وهندسة الإصدارات على نفس النسق.

## مسارات العمل

### 2. اعتماد fixando الحتمي

| الخطوة | المعلم | الوصف | المالك(ون) | Informações |
|--------|--------|-------|------------|----------|
| luminárias para jogos | M0 | Dry-runs أسبوعية تقارن digests المحلية للـ chunk مع `fixtures/sorafs_chunker`. Verifique o valor do `docs/source/sorafs/reports/`. | Provedores de armazenamento | `determinism-<date>.md` significa aprovação/reprovação. |
| فرض التواقيع | M1 | `ci/check_sorafs_fixtures.sh` + `.github/workflows/sorafs-fixtures-nightly.yml` é um arquivo de arquivo e manifesto. substitui a renúncia da renúncia do PR. | GT Ferramentaria | سجل CI, رابط تذكرة renúncia (إن وجدت). |
| Sinalizadores de expectativa | M1 | A chave `sorafs_manifest_stub` é a chave para a configuração: | Documentos CI | Você pode usar sinalizadores de expectativa (انظر كتلة الأمر أدناه). |
| Fixação primeiro do registro | M2 | `sorafs pin propose` e `sorafs pin approve` são arquivos do manifesto; CLI é compatível com `--require-registry`. | Operações de Governança | Para remover o registro CLI do sistema, você pode usá-lo. |
| Observabilidade | M3 | Os itens Prometheus/Grafana devem ser armazenados em pedaços de manifestos no registro; As operações de operações são importantes. | Observabilidade | رابط لوحة, IDs قواعد التنبيه, نتائج GameDay. |

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

Faça um resumo, um resumo e um CID para obter mais informações sobre o seu negócio.
Este artefato.

### 3. انتقال alias والاتصالات

| الخطوة | المعلم | الوصف | المالك(ون) | Informações |
|--------|--------|-------|------------|----------|
| إثباتات pseudônimo de encenação | M1 | تسجيل مطالبات alias في Pin Registry الخاص بـ staging وإرفاق إثباتات Manifestos Merkle مع (`--alias`). | Governança, Documentos | bundle إثباتات مخزن بجوار manifest + تعليق في السجل باسم alias. |
| فرض الإثباتات | M2 | Gateways ترفض manifestos بدون رؤوس `Sora-Proof` حديثة؛ CI não é compatível com `sorafs alias verify`. | Rede | Use o gateway + مخرجات CI توثق التحقق الناجح. |

### 4. الاتصالات والتدقيق- **انضباط السجل:** كل تغيير حالة (drift للـ fixtures, تقديم registro, تفعيل alias) يجب أن يضيف
  A solução está em `docs/source/sorafs/migration_ledger.md`.
- **محاضر الحوكمة:** جلسات المجلس التي تعتمد تغييرات Pin Registry أو سياسات alias يجب أن تشير
  Isso é algo que você pode fazer.
- **الاتصالات الخارجية:** DevRel ينشر تحديثات الحالة عند كل معلم (مدونة + مقتطف changelog)
  مع إبراز الضمانات الحتمية وجداول alias الزمنية.

## التبعيات والمخاطر

| التبعية | الأثر | التخفيف |
|--------|-------|---------|
| توفر عقد Registro de Pin | Em seguida, implemente o pino M2 primeiro. | تجهيز العقد قبل M2 مع اختبارات replay; O fallback do envelope não está associado às regressões. |
| مفاتيح توقيع المجلس | Você pode usar envelopes de manifesto e registro. | O nome do produto é `docs/source/sorafs/signing_ceremony.md`; Verifique se o produto está funcionando corretamente. |
| SDK do SDK | يجب على العملاء احترام إثباتات alias قبل M3. | مواءمة نوافذ إصدار SDK مع بوابات المعالم؛ Listas de verificação estão disponíveis no site da empresa. |

O código de barras e o cartão de crédito são `docs/source/sorafs_architecture_rfc.md`
ويجب الرجوع إليها عند إجراء تعديلات.

## قائمة تحقق معايير الخروج

| المعلم | المعايير |
|--------|----------|
| M1 | - مهمة fixtures الليلية خضراء لسبعة أيام متتالية.  - تحقق إثباتات alias no staging داخل CI.  - الحوكمة تصادق على سياسة sinalizadores de expectativa. |

## إدارة التغيير

1. اقترح التعديلات عبر PR يقوم بتحديث هذا الملف **و**
   `docs/source/sorafs/migration_ledger.md`.
2. اربط محاضر الحوكمة وأدلة CI e PR.
3. بعد الدمج, أخطر قائمة بريد storage + DevRel بملخص وإجراءات متوقعة للمشغلين.

A implementação do rollout SoraFS pode ser realizada e executada
O código é Nexus.