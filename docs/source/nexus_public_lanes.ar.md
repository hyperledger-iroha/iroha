---
lang: ar
direction: rtl
source: docs/source/nexus_public_lanes.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: f9bb3a13cec7d80bfd1729709eb0744a5a062954002ada5d48608f62f8907668
source_last_modified: "2025-12-08T18:48:53.874766+00:00"
translation_last_reviewed: 2026-01-01
---

<div dir="rtl">

<!-- الترجمة العربية لـ docs/source/nexus_public_lanes.md -->

# رهن المسارات العامة في Nexus (NX-9)

الحالة: 🈺 قيد التنفيذ → **مواءمة runtime ووثائق المشغلين** (ابريل 2026)
المالكون: Economics WG / Governance WG / Core Runtime
مرجع خارطة الطريق: NX-9 – Public lane staking & reward module

تلتقط هذه المذكرة نموذج البيانات القياسي وسطح التعليمات وضوابط الحوكمة وخطافات التشغيل
لبرنامج رهن المسارات العامة في Nexus. الهدف هو تمكين المدققين دون تصريح من الانضمام
الى المسارات العامة وربط الحصص وخدمة الكتل وتلقي المكافآت، بينما تحافظ الحوكمة على
ادوات slashing/runbook الحتمية.

يقع هيكل الكود حاليا في:

- انواع نموذج البيانات: `crates/iroha_data_model/src/nexus/staking.rs`
- تعريفات ISI: `crates/iroha_data_model/src/isi/staking.rs`
- هيكل تنفيذي اساسي (يرجع خطأ حتميا حتى يصل منطق NX-9):
  `crates/iroha_core/src/smartcontracts/isi/staking.rs`

يمكن لـ Torii/SDKs البدء بتمرير حمولات Norito قبل اكتمال تنفيذ runtime؛ تعليمات الرهن
تقوم الان بقفل اصل الرهن عبر السحب من `stake_account`/`staker` الى حساب escrow
مرتبط (`nexus.staking.stake_escrow_account_id`). تقوم slashes بخصم escrow
وائتمان sink المهيأ (`nexus.staking.slash_sink_account_id`)، وتعيد unbonds الاموال
الى الحساب الاصلي عند انتهاء المؤقت.

## 1. حالة الدفتر وانواعه

### 1.1 سجلات المدققين

يتتبع `PublicLaneValidatorRecord` الحالة القياسية لكل مدقق:

| الحقل | الوصف |
|-------|-------|
| `lane_id: LaneId` | المسار الذي يخدمه المدقق. |
| `validator: AccountId` | الحساب الذي يوقع رسائل الاجماع. |
| `stake_account: AccountId` | الحساب الذي يوفر self-bond (قد يختلف عن هوية المدقق). |
| `total_stake: Numeric` | الحصة الذاتية + التفويضات المعتمدة. |
| `self_stake: Numeric` | الحصة التي يقدمها المدقق. |
| `metadata: Metadata` | نسبة العمولة، معرفات التلِمتري، اعلام الاختصاص، معلومات الاتصال. |
| `status: PublicLaneValidatorStatus` | دورة الحياة (pending/active/jailed/exiting/etc.). حمولة `PendingActivation` ترمز الى العصر المستهدف. |
| `activation_epoch: Option<u64>` | العصر الذي اصبح فيه المدقق نشطا (يثبت عند التفعيل). |
| `activation_height: Option<u64>` | ارتفاع الكتلة المسجل عند التفعيل. |
| `last_reward_epoch: Option<u64>` | العصر الذي صدر فيه اخر دفع. |

يسرد `PublicLaneValidatorStatus` مراحل دورة الحياة:

- `PendingActivation(epoch)` — انتظار عصر التفعيل المحدد من الحوكمة؛ حمولة الترتيب تخزن
  ابكر عصر تفعيل محسوب كـ `current_epoch + 1` (genesis bootstrap uses `current_epoch`)
  (تستخرج العصور من `epoch_length_blocks`).
- `Active` — يشارك في الاجماع ويمكنه تحصيل المكافآت.
- `Jailed { reason }` — تعليق مؤقت (تعطل، خرق تلِمتري، الخ).
- `Exiting { releases_at_ms }` — unbonding؛ تتوقف المكافآت عن التراكم.
- `Exited` — تمت الازالة من المجموعة.
- `Slashed { slash_id }` — حدث slashing مسجل للتدقيق.

بيانات التفعيل احادية الاتجاه: `activation_epoch`/`activation_height` تثبت عند اول تفعيل
لمدقق pending، واي محاولة لاعادة التفعيل بعصر/ارتفاع اقدم ترفض. تتم ترقية المدققين
pending تلقائيا عند بداية اول كتلة يصل فيها العصر الى الحد المجدول، ويسجل عداد
تفعيل المدققين (`nexus_public_lane_validator_activation_total`) الترقية مع تغيير الحالة.

تحافظ النشرات المصرح بها على roster الخاص بـ genesis peers فعالا حتى قبل وجود رهن
لمدققي المسارات العامة: طالما ان لديهم مفاتيح اجماع نشطة، يعود runtime الى genesis peers
كمجموعة مدققين. هذا يمنع تعطل bootstrap اثناء تعطيل او طرح رهن staking.

### 1.2 حصص الرهن و unbonding

يتم تمثيل المفوضين (وكذلك المدققين الذين يعززون bond الخاص بهم) عبر
`PublicLaneStakeShare`:

- `bonded: Numeric` — المبلغ المربوط الفعال.
- `pending_unbonds: BTreeMap<Hash, PublicLaneUnbonding>` — سحوبات معلقة مرتبطة بـ `request_id` من العميل.
- `metadata` تخزن اشارات UX/back-office (مثل ارقام مرجعية لمكتب الحفظ).

تحتوي `PublicLaneUnbonding` على جدول السحب الحتمي (`amount`, `release_at_ms`). ويعرض Torii
الحصص الفعالة والسحوبات المعلقة عبر `GET /v1/nexus/public_lanes/{lane}/stake` حتى تعرض
المحافظ المؤقتات دون RPCs مخصصة.

خطافات دورة الحياة (تطبيق runtime):

- تنتقل إدخالات `PendingActivation(epoch)` تلقائيا الى `Active` عندما يصل العصر الحالي الى `epoch`.
  يسجل التفعيل `activation_epoch` و`activation_height`، ويتم رفض التراجعات سواء في التفعيل التلقائي
  او في استدعاءات `ActivatePublicLaneValidator` الصريحة.
- تنتقل إدخالات `Exiting(releases_at_ms)` الى `Exited` عندما يتجاوز timestamp الكتلة `releases_at_ms`،
  مع تنظيف صفوف حصص الرهن لاستعادة السعة دون تنظيف يدوي.
- يرفض تسجيل المكافآت حصص المدققين ما لم يكن المدقق `Active`، لمنع تراكم المكافآت للمدققين
  pending/exiting/jailed.

### 1.3 سجلات المكافآت

تستخدم توزيعات المكافآت `PublicLaneRewardRecord` و `PublicLaneRewardShare`:

```norito
{
  "lane_id": 1,
  "epoch": 4242,
  "asset": "4cuvDVPuLBKJyN6dPbRQhmLh68sU",
  "total_reward": "250.0000",
  "shares": [
    { "account": "6cmzPVPX5jDQFNfiz6KgmVfm1fhoAqjPhoPFn4nx9mBWaFMyUCwq4cw", "role": "Validator", "amount": "150" },
    { "account": "34mSYnCXkCzHXm31UDHh7SJfGvC4QPEhwim8z7sys2iHqXpCwCQkjL8KHvkFLSs1vZdJcb37r", "role": "Nominator", "amount": "100" }
  ],
  "metadata": {
    "telemetry_epoch_root": "0x4afe…",
    "distribution_tx": "0xaabbccdd"
  }
}
```

توفر السجلات ادلة حتمية لكل عملية دفع للمدققين ولوحات المتابعة. تتدفق بنية المكافآت
الى ISI `RecordPublicLaneRewards`.

حمايات runtime:

- يجب تفعيل Nexus builds؛ ترفض offline/stub builds تسجيل المكافآت.
- تتقدم عصور المكافآت بشكل احادي لكل lane؛ ترفض العصور القديمة او المكررة.
- يجب ان تتطابق اصول المكافآت مع fee sink المهيأ (`nexus.fees.fee_sink_account_id` /
  `nexus.fees.fee_asset_id`) وان يغطي رصيد sink كامل `total_reward`.
- يجب ان تكون كل حصة موجبة وتلتزم بالمواصفة العددية للاصل؛ يجب ان يساوي مجموع الحصص `total_reward`.

## 2. كتالوج التعليمات

كل التعليمات تقع تحت `iroha_data_model::isi::staking`. وهي تشتق encoders/decoders من Norito
لتتمكن SDKs من ارسال الحمولات دون codecs مخصصة.

### 2.1 `RegisterPublicLaneValidator`

يسجل مدققا ويربط رهن ابتدائي:

```norito
{
  "lane_id": 1,
  "validator": "6cmzPVPX5jDQFNfiz6KgmVfm1fhoAqjPhoPFn4nx9mBWaFMyUCwq4cw",
  "stake_account": "6cmzPVPX5jDQFNfiz6KgmVfm1fhoAqjPhoPFn4nx9mBWaFMyUCwq4cw",
  "initial_stake": "150000",
  "metadata": {
    "commission_bps": 750,
    "jurisdiction": "JP",
    "telemetry_id": "val-01"
  }
}
```

قواعد التحقق:

- `initial_stake` >= `min_self_stake` (معامل حوكمة).
- يجب ان تتضمن metadata خطافات الاتصال/التلِمتري قبل التفعيل.
- توافق الحوكمة او ترفض الادخال؛ حتى ذلك الحين تكون الحالة `PendingActivation` ويقوم runtime بترقية
  المدقق الى `Active` عند حد العصر التالي بعد بلوغ العصر المستهدف
  (`current_epoch + 1` (genesis bootstrap uses `current_epoch`) عند التسجيل).

### 2.2 `BondPublicLaneStake`

يربط رهن اضافيا (self-bond للمدقق او مساهمة مفوض).

الحقول الاساسية: `staker`, `amount`, و metadata اختيارية للبيانات. يجب على runtime تطبيق
حدود خاصة بكل lane (`max_delegators`, `min_bond`, `commission caps`).

### 2.3 `SchedulePublicLaneUnbond`

يبدأ مؤقت unbonding. يقدم المرسلون `request_id` حتميا (توصية: `blake2b(invoice)`)، و`amount`
و`release_at_ms`. يجب على runtime التحقق من `amount` <= الحصة المربوطة وضبط `release_at_ms`
على فترة unbonding المهيأة.

### 2.4 `FinalizePublicLaneUnbond`

بعد انتهاء المؤقت، يقوم هذا ISI بفك الرهن المعلق واعادته الى `staker`. يتحقق المنفذ
من request id، ويتأكد ان timestamp الفك في الماضي، ويصدر تحديث `PublicLaneStakeShare`
ويسجل التلِمتري.

### 2.5 `SlashPublicLaneValidator`

تستخدم الحوكمة هذه التعليمة لخصم الرهن وسجن/اخراج المدققين.

- `slash_id` يربط الحدث بالتلِمتري + مستندات الحوادث.
- `reason_code` سلسلة enum مستقرة (مثل `double_sign`, `downtime`, `safety_violation`).
- `metadata` تخزن hashes لحزم الادلة، مراجع runbook، او IDs للمنظمين.

تنتقل slashes الى المفوضين وفق سياسة الحوكمة (خسارة نسبية او validator-first).
ستصدر منطق runtime تعليقات `PublicLaneRewardRecord` عند توفر NX-9.

### 2.6 `RecordPublicLaneRewards`

يسجل الدفع لعصر محدد. الحقول:

- `reward_asset`: الاصل الموزع (افتراضي `xor#nexus`).
- `total_reward`: اجمالي minted/transferred.
- `shares`: قائمة `PublicLaneRewardShare`.
- `metadata`: مراجع معاملات الدفع او root hashes او dashboards.

هذه التعليمة idempotent لكل `(lane_id, epoch)` وتدعم المحاسبة الليلية.

## 3. العمليات ودورة الحياة والادوات

- **دورة الحياة + الاوضاع:** يتم تفعيل lanes stake-elected عبر
  `nexus.staking.public_validator_mode = stake_elected` بينما تبقى lanes المقيدة admin-managed
  (`nexus.staking.restricted_validator_mode = admin_managed`). تحافظ النشرات المصرح بها على
  genesis peers نشطين حتى يوجد stake؛ ولمسارات stake-elected ما زلنا نطلب وجود peer مسجل
  بمفتاح اجماع نشط في commit topology قبل نجاح `RegisterPublicLaneValidator`. تحدد
  genesis fingerprints و`use_stake_snapshot_roster` ما اذا كان runtime يستمد roster من
  لقطات stake او يعود الى genesis peers.
- **عمليات التفعيل/الخروج:** تدخل التسجيلات `PendingActivation` بهدف
  `current_epoch + 1` (genesis bootstrap uses `current_epoch`) وتترقى تلقائيا في اول كتلة
  يصل فيها العصر الى الحد (`epoch_length_blocks`). يمكن للمشغلين ايضا استدعاء
  `ActivatePublicLaneValidator` بعد الحد لفرض الترقية. تنقل المخارج المدققين الى
  `Exiting(release_at_ms)` ولا تتحرر السعة الا عندما يصل timestamp الكتلة الى
  `release_at_ms`؛ اعادة التسجيل بعد slash تتطلب الخروج لتمييز السجل `Exited`
  واستعادة السعة. تستخدم فحوصات السعة `nexus.staking.max_validators` وتعمل بعد
  finalizer الخروج، لذا تمنع المخارج المستقبلية التسجيلات الجديدة حتى انتهاء المؤقت.
- **مفاتيح الاعداد:** `nexus.staking.min_validator_stake`, `nexus.staking.stake_asset_id`,
  `nexus.staking.stake_escrow_account_id`, `nexus.staking.slash_sink_account_id`,
  `nexus.staking.unbonding_delay`, `nexus.staking.withdraw_grace`,
  `nexus.staking.max_validators`,
  `nexus.staking.max_slash_bps`, `nexus.staking.reward_dust_threshold`، ومفاتيح وضع المدققين اعلاه.
  مررها عبر `iroha_config::parameters::actual::Nexus` واعرضها في `status.md` بعد اعتماد قيم GA.
- **Torii/CLI quickstart:**
  - `iroha app nexus lane-report --summary` يعرض مدخلات كتالوج lanes، readiness للـ manifest،
    واوضاع المدققين (stake-elected مقابل admin-managed) حتى يتأكد المشغلون من تفعيل قبول الرهن.
  - `iroha_cli app nexus public-lane validators --lane <id> [--summary]`
    يعرض مؤشرات دورة الحياة/التفعيل (العصر المستهدف المعلق، `activation_epoch` / `activation_height`,
    release للخروج، و slash id) مع رهن bonded/self.
    `iroha_cli app nexus public-lane stake --lane <id> [--validator i105...] [--summary]`
    يعكس endpoint `/stake` مع hints pending-unbond لكل زوج `(validator, staker)`.
  - لقطات Torii للوحات المتابعة و SDKs:
    - `GET /v1/nexus/public_lanes/{lane}/validators` – metadata, status
      (`PendingActivation`/`Active`/`Exiting`/`Exited`/`Slashed`), activation epoch/height,
      release timers, bonded stake, اخر reward epoch.
      `canonical i105 literal rendering` يتحكم بعرض literals (I105 هو المفضل؛ I105 هو الخيار الثاني لسورا فقط).
    - `GET /v1/nexus/public_lanes/{lane}/stake` – stake shares (`validator`,
      `staker`, bonded amount) مع pending unbond timers. `?validator=i105...`
      يرشح الاستجابة للوحة تركز على مدقق واحد؛ `canonical i105 rendering` يطبق على جميع literals.
  - تستخدم ISIs دورة الحياة المسار القياسي للمعاملات (Torii `/v1/transactions`
    او pipeline تعليمات CLI). امثلة حمولة Norito JSON:

    ```jsonc
    [
      { "ActivatePublicLaneValidator": { "lane_id": 1, "validator": "6cmzPVPX5jDQFNfiz6KgmVfm1fhoAqjPhoPFn4nx9mBWaFMyUCwq4cw" } },
      {
        "ExitPublicLaneValidator": {
          "lane_id": 1,
          "validator": "6cmzPVPX5jDQFNfiz6KgmVfm1fhoAqjPhoPFn4nx9mBWaFMyUCwq4cw",
          "release_at_ms": 1730000000000
        }
      }
    ]
    ```
- **التلِمتري و runbooks:** تكشف المقاييس عن عدد المدققين، stake المربوط والمعلق، اجمالي المكافآت،
  وعدادات slash ضمن عائلة `nexus_public_lane_*`. اوصل لوحات المتابعة بنفس مجموعة البيانات التي تستخدم
  في اختبارات قبول NX-9 حتى تبقى فروقات المدققين وادلة reward/slash قابلة للتدقيق. تعليمات slashing
  تبقى محصورة بالحوكمة؛ يجب ان يثبت تسجيل المكافآت اجمالي payout (hash دفعة الدفع).

## 4. مواءمة خارطة الطريق

- ✅ تنفذ Runtime و WSV storages دورة حياة مدققي NX-9؛ تغطي الاختبارات توقيت التفعيل، متطلبات peers،
  الخروج المؤجل، واعادة التسجيل بعد slashes.
- ✅ يعرض Torii `/v1/nexus/public_lanes/{lane}/{validators,stake,rewards/pending}` بترميز Norito JSON
  حتى تتمكن SDKs ولوحات المتابعة من مراقبة حالة lane دون RPCs مخصصة.
- ✅ تم توثيق مفاتيح الاعداد والتلِمتري؛ تحافظ النشرات المختلطة على عزل lanes stake-elected و
  admin-managed لضمان حتمية rosters المدققين.

</div>

### 2.7 `CancelConsensusEvidencePenalty`

Cancels consensus slashing before the delayed penalty applies.

- `evidence`: the Norito-encoded `Evidence` payload that was recorded in `consensus_evidence`.
- The record is marked `penalty_cancelled` and `penalty_cancelled_at_height`, preventing slashing when `slashing_delay_blocks` elapses.
