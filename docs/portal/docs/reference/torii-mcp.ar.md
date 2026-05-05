<!-- Auto-generated stub for Arabic (ar) translation. Replace this content with the full translation. -->

---
lang: ar
direction: rtl
source: docs/portal/docs/reference/torii-mcp.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 316a408473f53a9763a18f40d49cfd766b5b93a3611e277e5a761e366e85c082
source_last_modified: "2026-03-15T11:38:44.302824+00:00"
translation_last_reviewed: 2026-04-02
translator: machine-google-reviewed
---

---
id: torii-mcp
title: Torii MCP API
description: الدليل المرجعي لاستخدام جسر بروتوكول سياق النموذج الأصلي لـ Torii.
---

يعرض Torii جسرًا أصليًا لبروتوكول سياق النموذج (MCP) في `/v1/mcp`.
تتيح نقطة النهاية هذه للوكلاء اكتشاف الأدوات واستدعاء مسارات Torii/Connect من خلال JSON-RPC.

## شكل نقطة النهاية

- يقوم `GET /v1/mcp` بإرجاع البيانات الوصفية للإمكانيات (وليست ملفوفة بـ JSON-RPC).
- يقبل `POST /v1/mcp` طلبات JSON-RPC 2.0.
- إذا كان `torii.mcp.enabled = false`، فلن يتم عرض أي من المسارين.
- إذا تم تمكين `torii.require_api_token`، فسيتم رفض الرمز المميز المفقود/غير الصالح قبل إرسال JSON-RPC.

## التكوين

تمكين MCP ضمن `torii.mcp`:

```json
{
  "torii": {
    "mcp": {
      "enabled": true,
      "max_request_bytes": 1048576,
      "max_tools_per_list": 500,
      "profile": "read_only",
      "expose_operator_routes": false,
      "allow_tool_prefixes": [],
      "deny_tool_prefixes": [],
      "rate_per_minute": 240,
      "burst": 120,
      "async_job_ttl_secs": 300,
      "async_job_max_entries": 2000
    }
  }
}
```

السلوك الرئيسي:

- يتحكم `profile` في رؤية الأداة (`read_only`، `writer`، `operator`).
- `allow_tool_prefixes`/`deny_tool_prefixes` يطبق سياسة إضافية تعتمد على الاسم.
- `rate_per_minute`/`burst` تطبيق تحديد مجموعة الرمز المميز لطلبات MCP.
- يتم الاحتفاظ بحالة المهمة غير المتزامنة من `tools/call_async` في الذاكرة باستخدام `async_job_ttl_secs` و`async_job_max_entries`.

## تدفق العميل الموصى به

1. اتصل على `initialize`.
2. اتصل بـ `tools/list` وذاكرة التخزين المؤقت `toolsetVersion`.
3. استخدم `tools/call` للعمليات العادية.
4. استخدم `tools/call_async` + `tools/jobs/get` لعمليات أطول.
5. أعد تشغيل `tools/list` عندما يكون `listChanged` هو `true`.

لا تقم بترميز كتالوج الأدوات الكامل. اكتشاف في وقت التشغيل.

## الأساليب والدلالات

طرق JSON-RPC المدعومة:

-`initialize`
-`tools/list`
-`tools/call`
-`tools/call_batch`
-`tools/call_async`
-`tools/jobs/get`

ملاحظات:- يقبل `tools/list` كلاً من `toolset_version` و`toolsetVersion`.
- يقبل `tools/jobs/get` كلاً من `job_id` و`jobId`.
- `tools/list.cursor` عبارة عن إزاحة سلسلة رقمية؛ ترجع القيم غير الصالحة إلى `0`.
- `tools/call_batch` هو أفضل جهد لكل عنصر (مكالمة واحدة فاشلة لا تؤدي إلى فشل مكالمات الأخوة).
- `tools/call_async` يتحقق من صحة شكل المغلف فقط على الفور؛ تظهر أخطاء التنفيذ لاحقًا في حالة المهمة.
- يجب أن يكون `jsonrpc` هو `"2.0"`؛ تم قبول `jsonrpc` المحذوف من أجل التوافق.

## المصادقة وإعادة التوجيه

لا يتجاوز إرسال MCP ترخيص Torii. تنفذ المكالمات معالجات المسار العادية وعمليات التحقق من المصادقة.

يقوم Torii بإعادة توجيه الرؤوس الواردة المتعلقة بالمصادقة لإرسال الأداة:

-`Authorization`
-`x-api-token`
-`x-iroha-account`
-`x-iroha-signature`
-`x-iroha-api-version`

يمكن للعملاء أيضًا توفير رؤوس إضافية لكل مكالمة عبر `arguments.headers`.
يتم تجاهل `content-length` و`host` و`connection` من `arguments.headers`.

## نموذج الخطأ

طبقة HTTP:

- `400` JSON غير صالح
- تم رفض الرمز المميز `403` API قبل معالجة JSON-RPC
- الحمولة `413` تتجاوز `max_request_bytes`
- معدل `429` محدود
- `200` لاستجابات JSON-RPC (بما في ذلك أخطاء JSON-RPC)

طبقة JSON-RPC:- المستوى الأعلى `error.data.error_code` مستقر (على سبيل المثال `invalid_request`، `invalid_params`، `tool_not_found`، `tool_not_allowed`، `job_not_found`، `rate_limited`).
- تظهر حالات فشل الأداة حيث تنتج أداة MCP مع `isError = true` والتفاصيل المنظمة.
- تقوم حالات فشل أداة إرسال المسار بتعيين حالة HTTP إلى `structuredContent.error_code` (على سبيل المثال `forbidden`، `not_found`، `server_error`).

## تسمية الأداة

تستخدم الأدوات المشتقة من OpenAPI أسماء مستقرة تعتمد على المسار:

-`torii.<method>_<path...>`
- مثال: `torii.get_v1_accounts`

يتم أيضًا عرض الأسماء المستعارة المنسقة ضمن `iroha.*` و`connect.*`.

## المواصفات الأساسية

يتم الاحتفاظ بالعقد الكامل على مستوى السلك في:

-`crates/iroha_torii/docs/mcp_api.md`

عندما يتغير السلوك في `crates/iroha_torii/src/mcp.rs` أو `crates/iroha_torii/src/lib.rs`،
قم بتحديث هذه المواصفات بنفس التغيير ثم اعكس إرشادات استخدام المفتاح هنا.