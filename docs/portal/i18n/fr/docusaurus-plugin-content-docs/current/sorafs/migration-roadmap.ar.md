---
lang: fr
direction: ltr
source: docs/portal/docs/sorafs/migration-roadmap.ar.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
titre : "خارطة طريق ترحيل SoraFS"
---

> مقتبس من [`docs/source/sorafs/migration_roadmap.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sorafs/migration_roadmap.md).

# خارطة طريق ترحيل SoraFS (SF-1)

هذا المستند يُحوِّل إرشادات الترحيل الموثقة في
`docs/source/sorafs_architecture_rfc.md` est en cours de réalisation. يوسِّع مخرجات SF-1 إلى
معالم جاهزة للتنفيذ ومعايير بوابة وقوائم تحقق للمالكين حتى تتمكن فرق التخزين
والحوكمة وDevRel وSDK من تنسيق الانتقال من استضافة artefacts إلى نشر
مدعوم par SoraFS.

خارطة الطريق حتمية عمدا: كل معلم يسمي artefacts المطلوبة واستدعاءات الأوامر وخطوات
الاستيثاق حتى تنتج خطوط الأنابيب اللاحقة مخرجات متطابقة وتحافظ الحوكمة على أثر
قابل للتدقيق.

## نظرة عامة على المعالم

| المعلم | النافذة | الأهداف الأساسية | ما يجب تسليمه | المالكون |
|--------|---------|--------|----------------|--------------|
| **M1 - Application déterministe** | الأسابيع 7-12 | Les luminaires et les alias sont également des drapeaux d'attente. | Il y a des luminaires, des manifestes et des alias de mise en scène. | Stockage, gouvernance, SDK |

Vous êtes actuellement en contact avec `docs/source/sorafs/migration_ledger.md`. كل تغيير في هذه
الخارطة يجب أن يحدِّث السجل ليبقى الحوكمة وهندسة الإصدارات على نفس النسق.

## مسارات العمل

### 2. اعتماد épinglant الحتمي| الخطوة | المعلم | الوصف | المالك(ون) | المخرجات |
|--------|--------|-------|------------|----------|
| calendriers de تدريبات | M0 | Les essais à sec digèrent le morceau du morceau `fixtures/sorafs_chunker`. Il s'agit de `docs/source/sorafs/reports/`. | Fournisseurs de stockage | `determinism-<date>.md` est une réussite/échec. |
| فرض التواقيع | M1 | `ci/check_sorafs_fixtures.sh` + `.github/workflows/sorafs-fixtures-nightly.yml` sont des manifestes. remplace la renonciation de التطوير تتطلب من الحوكمة مرفق بالـ PR. | GT Outillage | Il y a une renonciation au CI, رابط تذكرة (إن وجدت). |
| Indicateurs d'attente | M1 | خطوط الأنابيب تستدعي `sorafs_manifest_stub` بتوقعات صريحة لتثبيت المخرجات: | Documents CI | Il s'agit de drapeaux d'attente (انظر كتلة الأمر أدناه). |
| Épinglage dans le registre en premier | M2 | `sorafs pin propose` و`sorafs pin approve` يغلِّفان تقديمات manifeste؛ CLI est disponible sur `--require-registry`. | Opérations de gouvernance | سجل تدقيق CLI للـ Registry, تليمترية فشل المقترحات. |
| Voir observabilité | M3 | Prometheus/Grafana vous permet de récupérer des morceaux dans les manifestes du registre. التنبيهات موصولة بمناوبة ops. | Observabilité | Utilisez les ID pour le GameDay. |

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

استبدل قيم digest والحجم وCID بالمراجع المتوقعة المسجلة في إدخال سجل الترحيل
للـ artefact.

### 3. انتقال alias والاتصالات| الخطوة | المعلم | الوصف | المالك(ون) | المخرجات |
|--------|--------|-------|------------|----------|
| إثباتات alias pour la mise en scène | M1 | Il existe un alias dans le registre Pin pour la mise en scène et les manifestes Merkle (`--alias`). | Gouvernance, Docs | bundle إثباتات مخزن بجوار manifest + تعليق في السجل باسم alias. |
| فرض الإثباتات | M2 | Gateways ترفض manifestes بدون رؤوس `Sora-Proof` حديثة؛ CI يضيف خطوة `sorafs alias verify` لجلب الإثباتات. | Réseautage | تصحيح إعدادات gateway + مخرجات CI توثق التحقق الناجح. |

### 4. الاتصالات والتدقيق

- **انضباط السجل:** كل تغيير حالة (drift للـ luminaires, تقديمregistration, تفعيل alias) يجب أن يضيف
  ملاحظة مؤرخة في `docs/source/sorafs/migration_ledger.md`.
- **محاضر الحوكمة:** جلسات المجلس التي تعتمد تغييرات Pin Registry et alias يجب أن تشير
  إلى هذه الخارطة والسجل معا.
- **الاتصالات الخارجية:** DevRel ينشر تحديثات الحالة عند كل معلم (مدونة + مقتطف changelog)
  مع إبراز الضمانات الحتمية وجداول alias الزمنية.

## التبعيات والمخاطر| التبعية | الأثر | التخفيف |
|---------|-------|---------|
| توفر عقد Registre des broches | Je déploie le M2 en premier. | تجهيز العقد قبل M2 مع اختبارات replay؛ Il s'agit d'une enveloppe de secours ou d'une enveloppe de régression. |
| مفاتيح توقيع المجلس | مطلوبة لـ enveloppes manifestes et registre. | مراسم التوقيع موثقة في `docs/source/sorafs/signing_ceremony.md`؛ تدوير المفاتيح بتداخل وتدوين ذلك في السجل. |
| Voir SDK | Il s'agit d'un alias M3. | Utiliser le SDK avec des fonctionnalités supplémentaires إضافة checklists للترحيل في قوالب الإصدار. |

المخاطر المتبقية ووسائل التخفيف مذكورة أيضا في `docs/source/sorafs_architecture_rfc.md`
ويجب الرجوع إليها عند إجراء تعديلات.

## قائمة تحقق معايير الخروج

| المعلم | المعايير |
|--------|----------|
| M1 | - مهمة luminaires الليلية خضراء لسبعة أيام متتالية.  - تحقق إثباتات alias في staging داخل CI.  - Les drapeaux d'attente sont également disponibles. |

## إدارة التغيير

1. اقترح التعديلات عبر PR يقوم بتحديث هذا الملف **و**
   `docs/source/sorafs/migration_ledger.md`.
2. اربط محاضر الحوكمة وأدلة CI في وصف الـ PR.
3. Utilisez le stockage + DevRel pour ajouter des éléments de stockage.

اتباع هذا الإجراء يضمن أن rollout SoraFS يبقى حتميا وقابلا للتدقيق وشفافا عبر
Il s'agit d'un Nexus.