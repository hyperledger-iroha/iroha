---
lang: ar
direction: rtl
source: docs/portal/docs/sns/local-to-global-toolkit.ar.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# مجموعة ادوات العناوين المحلية -> العالمية

احترام هذه الصفحة `docs/source/sns/local_to_global_toolkit.md` من مستودع الاحادي. وهي ميجا ادوات CLI و runbooks مطلوبة لبند خارطة الطريق **ADDR-5c**.

## نظرة عامة

- `scripts/address_local_toolkit.sh` يلف CLI الخاص بـ `iroha` لانتاج:
  - `audit.json` - منظم الصرف من `iroha tools address audit --format json`.
  - `normalized.txt` -- literals IH58 (المفضل) / مضغوط (`sora`) (الخيار الثاني) محولة لكل محدد من النطاق Local.
- استخدم السكربت مع لوحة تناول الاناوين (`dashboards/grafana/address_ingest.json`)
  وقواعد Alertmanager (`dashboards/alerts/address_ingest_rules.yml`) لاثبات ان Cutover Local-8 /
  محلي-12 امن. راقب لوحات التصادم Local-8 و Local-12 والتنبيهات
  `AddressLocal8Resurgence` و`AddressLocal12Collision` و`AddressInvalidRatioSlo` قبل
  أحدث التطورات.
- ارجع الى [إرشادات عرض العنوان](address-display-guidelines.md) و
  [دليل تشغيل بيان العنوان](../../../source/runbooks/address_manifest_ops.md) لسياق UX والاستجابة.

##استخدام

```bash
scripts/address_local_toolkit.sh       --input fixtures/address/local_digest_examples.txt       --output-dir artifacts/address_migration       --network-prefix 753       --format ih58
```

الخيارات:

- `--format compressed (`sora`)` لخروج `sora...` بدلات من IH58.
- `--no-append-domain` لاصدار حرفي بدون نطاق.
- `--audit-only` للتخطي خطوة للتحويل.
- `--allow-errors` للاستمرار عند ظهور صفوف تالفة (وفقًا لسلوك CLI).

يطبع السكربت مسارات أثرية في نهاية التشغيل. ارفق الملف كلاين مع
تذكرة لقطة إدارة التغيير ومع Grafana التي تؤكد صفر
اكتشافات محلية-8 وصفر تصادمات محلية-12 لمدة >=30 يوما.

## تكامل CI1. شغل السكربت في الوظيفة المخصصة واستخراج المخرجات.
2. حظر المنتجات المدمجة للطلاب `audit.json` عن المحددات المحلية (`domain.kind = local12`).
   على القيمة الافتراضية `true` (قم بالتحويل الى `false` فقط على بيئات dev/test عند
   تشخيصات السماء) واضف
   `iroha tools address normalize --fail-on-warning --only-local` إلى CI حتى الفشل
   منذ اللحظة قبل الوصول الى الإنتاج.

راجع المستند إلى المصدر مشددًا على التفاصيل وقوائم المحامين ومقتطف مذكرة الإصدار
الذي قمت باعادة استخدامه عند اعلان القطع الشامل.