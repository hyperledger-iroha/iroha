---
lang: fr
direction: ltr
source: docs/portal/docs/nexus/nexus-refactor-plan.ar.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
identifiant : plan de refactorisation de lien
title: خطة اعادة هيكلة دفتر Sora Nexus
description: نسخة مطابقة لـ `docs/source/nexus_refactor_plan.md` توضح اعمال التنظيف المرحلية لقاعدة شفرة Iroha 3.
---

:::note المصدر القانوني
Il s'agit de la référence `docs/source/nexus_refactor_plan.md`. ابق النسختين متوافقتين حتى تصل النسخة متعددة اللغات الى البوابة.
:::

# خطة اعادة هيكلة دفتر Sora Nexus

Vous pouvez également utiliser le registre Sora Nexus ("Iroha 3"). تعكس مخطط المستودع الحالي والانتكاسات المرصودة في محاسبة genesis/WSV واجماع Sumeragi ومشغلات العقود Le pointeur-ABI est Norito. الهدف هو الوصول الى معمارية متماسكة قابلة للاختبار دون محاولة ادخال كل الاصلاحات في تصحيح واحد Oui.

## 0. مبادئ موجهة
- الحفاظ على سلوك حتمي عبر عتاد متنوع؛ Il y a des indicateurs de fonctionnalité et des solutions de secours.
- Norito pour votre appareil. Il y aura des matchs aller-retour pour les matchs aller-retour Norito.
- يمر التكوين عبر `iroha_config` (utilisateur -> réel -> valeurs par défaut). ازل bascule entre la fonction de fonction et la fonction de fonction.
- سياسة ABI تبقى V1 et قابلة للتفاوض. Vous pouvez également utiliser les types de pointeurs/appels système.
- `cargo test --workspace` et tests d'or (`ivm`, `norito`, `integration_tests`) sont également disponibles.## 1. لقطة من طوبولوجيا المستودع
- `crates/iroha_core` : ممثلو Sumeragi وWSV ومحمل genesis وخطوط الانابيب (requête, superposition, voies zk) et مضيف العقود الذكية.
- `crates/iroha_data_model` : المخطط المرجعي للبيانات والاستعلامات على السلسلة.
- `crates/iroha` : la connexion avec la CLI et le SDK.
- `crates/iroha_cli` : la CLI est utilisée pour les API et `iroha`.
- `crates/ivm` : Le lien entre le pointeur et l'ABI est Kotodama.
- `crates/norito` : codec pour JSON et AoS/NCB.
- `integration_tests` : Fonctions de démarrage et de genèse/bootstrap et Sumeragi, déclencheurs et pagination.
- Vous avez besoin de Sora Nexus Ledger (`nexus.md`, `new_pipeline.md`, `ivm.md`) pour vous aider ومتهالك جزئيا مقارنة بالشفرة.

## 2. ركائز اعادة الهيكلة والمراحل### المرحلة A - الاساسات والمراقبة
1. **Télémétrie WSV + instantanés**
   - L'API est utilisée pour `state` (trait `WorldStateSnapshot`) et Sumeragi et CLI.
   - استخدام `scripts/iroha_state_dump.sh` pour les instantanés حتمية عبر `iroha state dump --format norito`.
2. **حتمية Genesis/Bootstrap**
   - اعادة هيكلة ادخال genesis ليمر عبر مسار واحد يعتمد Norito (`iroha_core::genesis`).
   - اضافة تغطية تكامل/ارتداد تعيد تشغيل genesis مع اول كتلة وتؤكد تطابق جذور WSV by arm64/x86_64 (متابعة في `integration_tests/tests/genesis_replay_determinism.rs`).
3. **اختبارات ثبات عبر الحزم**
   - Il s'agit du `integration_tests/tests/genesis_json.rs` pour le pipeline WSV et l'ABI et le harnais.
   - L'échafaudage `cargo xtask check-shape` utilise la dérive de schéma (l'arriéré de DevEx est lié à la dérive de schéma `scripts/xtask/README.md`).### المرحلة B - WSV et الاستعلام
1. **معاملات تخزين الحالة**
   - دمج `state/storage_transactions.rs` في محول معاملات يفرض ترتيب commit وكشف التعارض.
   - Les éléments suivants sont associés aux actifs/monde/déclencheurs.
2. **اعادة هيكلة نموذج الاستعلام**
   - نقل منطق pagination/cursor الى مكونات قابلة لاعادة الاستخدام تحت `crates/iroha_core/src/query/`. Utilisez le code Norito pour `iroha_data_model`.
   - Les requêtes d'instantanés, les déclencheurs, les actifs et les rôles sont également disponibles (voir `crates/iroha_core/tests/snapshot_iterable.rs` pour les tâches).
3. **اتساق اللقطات**
   - La CLI `iroha ledger query` est associée à Sumeragi/fetchers.
   - La régression de la CLI est basée sur `tests/cli/state_snapshot.rs` (fonctionnalités de régression).### المرحلة C - خط انابيب Sumeragi
1. **الطوبولوجيا وادارة الحقبات**
   - `EpochRosterProvider` trait de caractère pour les instantanés de mise dans WSV.
   - `WsvEpochRosterAdapter::from_peer_iter` est destiné aux bancs/tests.
2. **تبسيط تدفق الاجماع**
   - اعادة تنظيم `crates/iroha_core/src/sumeragi/*` الى وحدات: `pacemaker`, `aggregation`, `availability`, `witness` et plus encore. C'est `consensus`.
   - استبدال تمرير الرسائل العشوائي بأغلفة Norito نوعية وادخال property tests لتغيير العرض (متابعة في backlog رسائل Sumeragi).
3. ** تكامل voie/preuve **
   - مواءمة voies preuves مع التزامات DA وضمان ان gating لـ RBC موحد.
   - اختبار تكامل de bout en bout `integration_tests/tests/extra_functional/seven_peer_consistency.rs` يتحقق الان من المسار مع تمكين RBC.### المرحلة D - العقود الذكية ومضيفات pointeur-ABI
1. **تدقيق حدود المضيف**
   - Utilisez le type de pointeur (`ivm::pointer_abi`) et le type de pointeur (`iroha_core::smartcontracts::ivm::host`).
   - Les manifestes sont disponibles pour `crates/iroha_core/tests/ivm_pointer_abi_tlv_types.rs` et `ivm_host_mapping.rs` pour le TLV doré.
2. **Déclencheurs Sandbox تنفيذ**
   - Les déclencheurs sont activés par `TriggerExecutor` et sont connectés à d'autres systèmes.
   - اضافة اختبارات régression لمشغلات call/time تغطي مسارات الفشل (متابعة عبر `crates/iroha_core/tests/trigger_failure.rs`).
3. **محاذاة CLI والعميل**
   - Utilisez les CLI (`audit`, `gov`, `sumeragi`, `ivm`) pour les rendre plus faciles à utiliser. Il s'agit de `iroha`.
   - Captures d'écran JSON en CLI et `tests/cli/json_snapshot.rs` ; Il est possible de créer des liens vers des fichiers JSON.### المرحلة E - Codec Norito
1. **سجل المخططات**
   - انشاء سجل مخطط Norito تحت `crates/norito/src/schema/` لتوفير ترميزات قانونية للانواع الاساسية.
   - Le document teste les charges utiles des tests (`norito::schema::SamplePayload`).
2. ** تحديث luminaires dorés **
   - Les luminaires dorés sont `crates/norito/tests/*` pour le WSV الجديد عند اكتمال اعادة الهيكلة.
   - `scripts/norito_regen.sh` يعيد توليد golden JSON لنوريتو بشكل حتمي عبر المساعد `norito_regen_goldens`.
3. **تكامل IVM/Norito**
   - Il s'agit du manifeste Kotodama avec le pointeur de métadonnées ABI Kotodama.
   - `crates/ivm/tests/manifest_roundtrip.rs` يحافظ على تساوي Norito encode/décode les éléments.## 3. قضايا مشتركة
- **استراتيجية الاختبارات** : pour les tests unitaires -> tests de caisse -> tests d'intégration. الاختبارات الفاشلة تلتقط الانتكاسات الحالية؛ الاختبارات الجديدة تمنع عودتها.
- **التوثيق** : بعد كل مرحلة، حدث `status.md` وادفع العناصر المفتوحة الى `roadmap.md` مع حذف المهام المكتملة.
- **معايير الاداء** : الحفاظ على bancs الحالية في `iroha_core` و`ivm` و`norito` ; Il s'agit d'une question de régressions.
- **Drapeaux de fonctionnalité** : permet de basculer entre les chaînes d'outils de la caisse et les chaînes d'outils (`cuda`, `zk-verify-batch`). La carte SIMD pour CPU est compatible avec les cartes SIMD Les solutions de secours sont également disponibles pour vous.## 4. Activités de plein air
- Échafaudage للمرحلة A (trait instantané + télémétrie ربط) - راجع المهام القابلة للتنفيذ في تحديثات feuille de route.
- تدقيق العيوب الاخير لـ `sumeragi` و`state` و`ivm` اظهر النقاط التالية :
  - `sumeragi` : allocations pour code mort pour le changement de vue et la relecture VRF et la télémétrie EMA. تبقى هذه الاجزاء gated حتى تتوفر تبسيطات تدفق الاجماع في المرحلة C et تكامل voie/preuve.
  - `state` : `Cell` pour la télémétrie et la télémétrie WSV pour A, pour les applications SoA/parallel-apply Le backlog du pipeline pour le projet C.
  - `ivm` : permet de basculer entre les enveloppes CUDA et Halo2/Metal pour les enveloppes D et D. تسريع GPU العرضي؛ Il s'agit des noyaux et du backlog GPU.
- La RFC est également disponible pour la signature de la demande de signature.

## 5. اسئلة مفتوحة
- هل يجب ان يبقى RBC اختياريا بعد P1, ام انه الزامي لخطوط دفتر Nexus؟ يتطلب قرار اصحاب المصلحة.
- La composabilité est également disponible pour DS dans P1 et les preuves de voie sont disponibles.
- ما الموقع القانوني لمعاملات ML-DSA-87؟ Objet : caisse `crates/fastpq_isi` (قيد الانشاء).

---

_اخر تحديث: 2025-09-12_