---
lang: ar
direction: rtl
source: docs/source/kagami_profiles.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 061304711d940567ec3c15a75c388085e65aafc6962abc2da6e943fa9a9903fa
source_last_modified: "2026-01-27T18:39:03.379028+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# ملفات تعريف Kagami Iroha3

يقوم Kagami بشحن الإعدادات المسبقة لشبكات Iroha 3 حتى يتمكن المشغلون من ختم الحتمية
يتجلى التكوين دون شعوذة المقابض لكل شبكة.

- ملفات التعريف: `iroha3-dev` (السلسلة `iroha3-dev.local`، المجمعات k=1 r=1، بذور VRF المشتقة من معرف السلسلة عند تحديد NPoS)، `iroha3-taira` (السلسلة `iroha3-taira`، المجمعات k=3 r=3، يتطلب `--vrf-seed-hex` عندما يكون NPoS محدد)، `iroha3-nexus` (السلسلة `iroha3-nexus`، المجمعات k=5 r=3، تتطلب `--vrf-seed-hex` عند تحديد NPoS).
- الإجماع: تتطلب شبكات ملفات تعريف Sora (Nexus + مساحات البيانات) NPoS ولا تسمح بعمليات القطع المرحلية؛ يجب تشغيل عمليات نشر Iroha3 المصرح بها بدون ملف تعريف Sora.
- الجيل: `cargo run -p iroha_kagami -- genesis generate --profile <profile> --ivm-dir . --genesis-public-key <pk> --consensus-mode <npos|permissioned> [--vrf-seed-hex <hex>]`. استخدم `--consensus-mode npos` لـ Nexus؛ `--vrf-seed-hex` صالح فقط لـ NPoS (مطلوب للخصية/الرابطة). يقوم Kagami بتثبيت DA/RBC على خط Iroha3 ويصدر ملخصًا (سلسلة، مجمعات، DA/RBC، بذرة VRF، بصمة الإصبع).
- التحقق: يعيد `cargo run -p iroha_kagami -- verify --profile <profile> --genesis <path> [--vrf-seed-hex <hex>]` تشغيل توقعات الملف الشخصي (معرف السلسلة، DA/RBC، المجمعات، تغطية PoP، بصمة الإجماع). قم بتوفير `--vrf-seed-hex` فقط عند التحقق من بيان NPoS للخصية/الرابطة.
- نماذج الحزم: الحزم التي تم إنشاؤها مسبقًا موجودة ضمن `defaults/kagami/iroha3-{dev,taira,nexus}/` (genesis.json، config.toml، docker-compose.yml، Vere.txt، README). التجديد باستخدام `cargo xtask kagami-profiles [--profile <name>|all] [--out <dir>] [--kagami <bin>]`.
- Mochi: `mochi`/`mochi-genesis` اقبل `--genesis-profile <profile>` و`--vrf-seed-hex <hex>` (NPoS فقط)، وأرسلهما إلى Kagami، واطبع نفس ملخص Kagami إلى stdout/stderr عند استخدام ملف التعريف.

تقوم الحزم بتضمين BLS PoPs جنبًا إلى جنب مع إدخالات الهيكل حتى ينجح `kagami verify`
خارج الصندوق؛ اضبط الأقران/المنافذ الموثوقة في التكوينات حسب الحاجة لـ local
يركض الدخان.