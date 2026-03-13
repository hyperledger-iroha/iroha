---
lang: pt
direction: ltr
source: docs/portal/docs/sorafs/chunker-registry-rollout-checklist.ar.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: lista de verificação de implementação de registro de chunker
título: قائمة تحقق لإطلاق سجل chunker لسوراFS
sidebar_label: قائمة تحقق لإطلاق chunker
description: خطة إطلاق خطوة بخطوة لتحديثات سجل chunker.
---

:::note المصدر المعتمد
Modelo `docs/source/sorafs/chunker_registry_rollout_checklist.md`. احرص على إبقاء النسختين متزامنتين إلى أن يتم إيقاف مجموعة توثيق Sphinx القديمة.
:::

# قائمة تحقق لإطلاق سجل SoraFS

تجمع هذه القائمة الخطوات المطلوبة لترقية ملف chunker جديد, أو حزمة قبول مزوّد
Isso significa que você pode fazer isso sem problemas.

> **النطاق:** ينطبق على كل الإصدارات التي تعدل
> `sorafs_manifest::chunker_registry` أو أظرف قبول المزوّدين أو حزم الـ fixtures
> Nome (`fixtures/sorafs_chunker/*`).

## 1. تحقق ما قبل الإطلاق

1. أعد توليد الـ fixtures وتحقق من الحتمية:
   ```bash
   cargo run --locked -p sorafs_chunker --bin export_vectors
   cargo test -p sorafs_chunker --offline vectors
   ci/check_sorafs_fixtures.sh
   ```
2. تأكد من أن بصمات الحتمية في
   `docs/source/sorafs/reports/sf1_determinism.md` (ou seja, um código de barras)
   تتطابق مع الآثار المُعاد توليدها.
3. Use o `sorafs_manifest::chunker_registry` para usar
   `ensure_charter_compliance()` é:
   ```bash
   cargo test -p sorafs_manifest --lib chunker_registry::tests::ensure_charter_compliance
   ```
4. حدّث ملف اقتراح الحزمة:
   -`docs/source/sorafs/proposals/<profile>.json`
   - Você pode usar o recurso `docs/source/sorafs/council_minutes_*.md`
   - تقرير الحتمية

## 2. اعتماد الحوكمة

1. Grupo de Trabalho de Ferramentas e Digest الاقتراح إلى Painel de Infraestrutura do Parlamento Sora.
2. Faça o download do aplicativo aqui
   `docs/source/sorafs/council_minutes_YYYY-MM-DD.md`.
3. انشر الظرف الموقّع من البرلمان بجوار الـ jogos:
   `fixtures/sorafs_chunker/manifest_signatures.json`.
4. تحقق من إمكانية الوصول إلى الظرف عبر مساعد جلب الحوكمة:
   ```bash
   cargo xtask sorafs-fetch-fixture \
     --signatures <url-or-path-to-manifest_signatures.json> \
     --out fixtures/sorafs_chunker
   ```

## 3. Encenação

ارجع إلى [دليل مانيفست الـstaging](./staging-manifest-playbook) للحصول على شرح مفصل.

1. Use Torii para descobrir o código de descoberta ou `torii.sorafs` e aplicar a aplicação
   (`enforce_admission = true`).
2. ادفع أظرف قبول المزوّدين المعتمدة إلى دليل سجل staging المشار إليه في
   `torii.sorafs.discovery.admission.envelopes_dir`.
3. تحقق من انتشار إعلانات المزوّد عبر واجهة descoberta:
   ```bash
   curl -sS http://<torii-host>/v2/sorafs/providers | jq .
   ```
4. اختبر نقاط manifesto/plano مع رؤوس الحوكمة:
   ```bash
   sorafs-fetch --plan fixtures/chunk_fetch_specs.json \
     --gateway-provider "...staging config..." \
     --gateway-manifest-id <manifest-hex> \
     --gateway-chunker-handle sorafs.sf1@1.0.0
   ```
5. Verifique o valor do cartão (`torii_sorafs_*`) e verifique o valor do arquivo.
   Sim, sim.

## 4. إطلاق الإنتاج

1. Faça o staging no Torii.
2. أعلن نافذة التفعيل (التاريخ/الوقت, فترة السماح, خطة التراجع) لقنوات المشغلين e SDK.
3. ادمج PR الإصدار الذي يتضمن:
   - Luminárias وظرفًا محدثين
   - تغييرات الوثائق (مراجع الميثاق, تقرير الحتمية)
   - Roteiro / status do تحديث
4. ضع وسم الإصدار وأرشف القطع الموقعة لأغراض proveniência.

## 5. تدقيق ما بعد الإطلاق

1. التقط المقاييس النهائية (descoberta, معدل نجاح fetch, هيستوغرامات
   الأخطاء) 24 horas por dia.
2. Insira `status.md` para remover o excesso de água.
3. Verifique se o dispositivo está conectado (seja um computador portátil) em `roadmap.md`.