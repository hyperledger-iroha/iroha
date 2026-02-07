---
lang: pt
direction: ltr
source: docs/portal/docs/sorafs/chunker-registry-charter.ar.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: chunker-registry-charter
título: ميثاق سجل chunker para SoraFS
sidebar_label: Nome do chunker
description: ميثاق الحوكمة لتقديم ملفات chunker واعتمادها.
---

:::note المصدر المعتمد
Verifique o valor `docs/source/sorafs/chunker_registry_charter.md`. احرص على إبقاء النسختين متزامنتين إلى أن يتم إيقاف مجموعة توثيق Sphinx القديمة.
:::

# ميثاق حوكمة سجل chunker em SoraFS

> **مُصادَق عليه:** 2025-10-29 por Sora Parliament Infrastructure Panel (انظر
> `docs/source/sorafs/council_minutes_2025-10-29.md`). Você pode fazer isso sem problemas
> ويجب على فرق التنفيذ اعتبار هذه الوثيقة معيارية حتى يعتمد ميثاق بديل.

Você pode usar o chunker em SoraFS.
وهو يكمل [دليل تأليف ملفات chunker](./chunker-profile-authoring.md) عبر وصف كيفية اقتراح الملفات الجديدة ومراجعتها
وتصديقها ثم إيقافها لاحقاً.

## النطاق

Você pode fazer isso em `sorafs_manifest::chunker_registry` e
Não há ferramentas disponíveis (CLI de manifesto e CLI de anúncio de provedor e SDKs). ويُطبّق ثوابت alias e identificador
Exemplo de `chunker_registry::ensure_charter_compliance()`:

- معرفات الملفات أعداد صحيحة موجبة تزيد بشكل رتيب.
- Você pode usar o `namespace.name@semver` para substituir o `profile_aliases`.
  تليه البدائل القديمة.
- سلاسل البدائل يتم قصها وتكون فريدة ولا تتعارض مع المقابض المعتمدة في إدخالات أخرى.

## الأدوار

- **Autor(es)** – يُعدّون المقترح, ويعيدون توليد fixtures, ويجمعون أدلة الحتمية.
- **Grupo de Trabalho de Ferramentas (TWG)** – يتحقق من المقترح باستخدام قوائم التحقق المنشورة ويضمن ثبات قواعد السجل.
- **Conselho de Governança (CG)** – يراجع تقرير TWG, ويوقع ظرف المقترح, ويوافق على جداول النشر/الإيقاف.
- **Equipe de armazenamento** – يدير تنفيذ السجل وينشر تحديثات التوثيق.

## سير العمل عبر دورة الحياة

1. **تقديم المقترح**
   - ينفذ المؤلف قائمة التحقق من دليل التأليف ويُنشئ JSON de `ChunkerProfileProposalV1`
     `docs/source/sorafs/proposals/`.
   - Clique em CLI aqui:
     ```bash
     cargo run -p sorafs_manifest --bin sorafs_manifest_chunk_store -- --list-profiles
     cargo run -p sorafs_manifest --bin sorafs_manifest_chunk_store -- \
       --promote-profile=<handle> --json-out=-
     cargo run -p sorafs_manifest --bin sorafs_manifest_stub -- \
       --chunker-profile=<handle> --json-out=-
     ```
   - قدّم PR يحتوي على fixtures والمقترح وتقرير الحتمية وتحديثات السجل.

2. **Ferramentas de trabalho (TWG)**
   - أعد تشغيل قائمة التحقق (fixtures, fuzz, خط manifest/PoR).
   - شغّل `cargo test -p sorafs_car --chunker-registry` وتأكد من نجاح
     `ensure_charter_compliance()` é um problema.
   - تحقق من أن سلوك CLI (`--list-profiles`, `--promote-profile`, streaming
     `--json-out=-`).
   - Limpe a máquina de lavar roupa e a máquina de lavar roupa.

3. **موافقة المجلس (GC)**
   - راجع تقرير TWG وبيانات المقترح.
   - وقّع digest المقترح (`blake3("sorafs-chunker-profile-v1" || bytes)`) وأضف التواقيع إلى
     ظرف المجلس المرفق مع luminárias.
   - سجّل نتيجة التصويت في محاضر الحوكمة.

4. **Não**
   - ادمج PR مع تحديث:
     -`sorafs_manifest::chunker_registry_data`.
     - Nome (`chunker_registry.md` وأدلة التأليف/المطابقة).
     - luminárias وتقارير الحتمية.
   - Instale o SDK e o SDK para obter mais informações.

5. **الإيقاف / الإزالة التدريجية**
   - يجب أن تتضمن المقترحات التي تستبدل ملفاً قائماً نافذة نشر مزدوجة (فترات سماح) وخطة Bem.

6. **تغييرات طارئة**
   - تتطلب الإزالة, أو الإصلاحات العاجلة تصويتاً بالأغلبية من المجلس.
   - يجب على TWG توثيق خطوات الحد من المخاطر وتحديث سجل الحوادث.

## Ferramentas de ferramentas- `sorafs_manifest_chunk_store` e `sorafs_manifest_stub` são:
  - `--list-profiles`.
  - `--promote-profile=<handle>` لتوليد كتلة البيانات المعتمدة المستخدمة عند ترقية ملف.
  - `--json-out=-` é uma opção para stdout, mas não permite que você execute o procedimento.
- يتم استدعاء `ensure_charter_compliance()` عند تشغيل الثنائيات ذات الصلة
  (`manifest_chunk_store`, `provider_advert_stub`). Isso é o que acontece com CI إذا كانت
  Você pode fazer isso.

## حفظ السجلات

- Verifique o valor do arquivo em `docs/source/sorafs/reports/`.
- محاضر المجلس التي تشير إلى قرارات chunker محفوظة ضمن
  `docs/source/sorafs/migration_ledger.md`.
- Os dados `roadmap.md` e `status.md` devem ser removidos do computador.

## المراجع

- دليل التأليف: [دليل تأليف ملفات chunker](./chunker-profile-authoring.md)
- Nome de usuário: `docs/source/sorafs/chunker_conformance.md`
- مرجع السجل: [سجل ملفات chunker](./chunker-registry.md)