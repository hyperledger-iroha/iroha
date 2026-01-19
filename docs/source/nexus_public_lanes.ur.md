---
lang: ur
direction: rtl
source: docs/source/nexus_public_lanes.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: f9bb3a13cec7d80bfd1729709eb0744a5a062954002ada5d48608f62f8907668
source_last_modified: "2025-12-08T18:48:53.874766+00:00"
translation_last_reviewed: 2026-01-01
---

<div dir="rtl">

<!-- اردو ترجمہ برائے docs/source/nexus_public_lanes.md -->

# Nexus پبلک لین اسٹیکنگ (NX-9)

اسٹیٹس: 🈺 جاری → **runtime + آپریٹر ڈاکس ہم آہنگ** (اپریل 2026)
مالکان: Economics WG / Governance WG / Core Runtime
روڈمیپ حوالہ: NX-9 – Public lane staking & reward module

یہ نوٹ Nexus کے پبلک لین اسٹیکنگ پروگرام کے لئے کینونیکل ڈیٹا ماڈل، انسٹرکشن سطح، گورننس کنٹرولز،
اور آپریشنل ہکس کو سمیٹتا ہے۔ مقصد یہ ہے کہ permissionless validators پبلک لینز میں شامل ہوں،
stake bond کریں، بلاکس سروس کریں، اور ریوارڈز حاصل کریں جبکہ گورننس کے پاس deterministic slashing/runbook
لیورز برقرار رہیں۔

کوڈ کا اسکیفولڈ اب یہاں موجود ہے:

- ڈیٹا ماڈل ٹائپس: `crates/iroha_data_model/src/nexus/staking.rs`
- ISI تعریفیں: `crates/iroha_data_model/src/isi/staking.rs`
- core executor stub (NX-9 لاجک آنے تک ایک deterministic guard error واپس کرتا ہے):
  `crates/iroha_core/src/smartcontracts/isi/staking.rs`

Torii/SDKs مکمل runtime امپلیمنٹیشن سے پہلے Norito payloads کو وائر کر سکتے ہیں؛ اسٹیکنگ
انسٹرکشنز اب configured staking asset کو `stake_account`/`staker` سے bonded escrow account
(`nexus.staking.stake_escrow_account_id`) میں منتقل کر کے لاک کرتی ہیں۔ Slashes escrow کو debit
اور configured sink (`nexus.staking.slash_sink_account_id`) کو credit کرتی ہیں، اور unbonds ٹائمر
ختم ہونے پر فنڈز کو اصل اکاؤنٹ میں واپس لاتی ہیں۔

## 1. لیجر اسٹیٹ اور ٹائپس

### 1.1 ویلیڈیٹر ریکارڈز

`PublicLaneValidatorRecord` ہر ویلیڈیٹر کی کینونیکل اسٹیٹ کو ٹریک کرتا ہے:

| فیلڈ | وضاحت |
|------|-------|
| `lane_id: LaneId` | وہ lane جسے ویلیڈیٹر سروس کرتا ہے۔ |
| `validator: AccountId` | اکاؤنٹ جو consensus پیغامات پر دستخط کرتا ہے۔ |
| `stake_account: AccountId` | اکاؤنٹ جو self-bond فراہم کرتا ہے (ویلیڈیٹر شناخت سے مختلف ہو سکتا ہے)۔ |
| `total_stake: Numeric` | self stake + منظور شدہ delegations۔ |
| `self_stake: Numeric` | ویلیڈیٹر کی فراہم کردہ stake۔ |
| `metadata: Metadata` | کمیشن فیصد، ٹیلی میٹری IDs، jurisdiction flags، رابطہ معلومات۔ |
| `status: PublicLaneValidatorStatus` | لائف سائیکل (pending/active/jailed/exiting/etc.). `PendingActivation` payload ہدف epoch انکوڈ کرتا ہے۔ |
| `activation_epoch: Option<u64>` | وہ epoch جب ویلیڈیٹر فعال ہوا (ایکٹیویشن پر سیٹ ہوتا ہے)۔ |
| `activation_height: Option<u64>` | ایکٹیویشن پر ریکارڈ ہونے والی بلاک ہائٹ۔ |
| `last_reward_epoch: Option<u64>` | وہ epoch جس میں آخری ادائیگی ہوئی۔ |

`PublicLaneValidatorStatus` لائف سائیکل فیزز درج کرتا ہے:

- `PendingActivation(epoch)` — گورننس کے مقرر کردہ activation epoch کا انتظار؛ ٹپل payload `epoch_length_blocks`
  سے نکلا ہوا earliest activation epoch محفوظ کرتا ہے۔
- `Active` — consensus میں حصہ لیتا ہے اور ریوارڈز حاصل کر سکتا ہے۔
- `Jailed { reason }` — عارضی معطلی (downtime، ٹیلی میٹری breach وغیرہ)۔
- `Exiting { releases_at_ms }` — unbonding؛ ریوارڈز رک جاتے ہیں۔
- `Exited` — سیٹ سے ہٹا دیا گیا۔
- `Slashed { slash_id }` — آڈٹ کے لئے ریکارڈ کیا گیا slashing ایونٹ۔

Activation metadata monotonic ہے: `activation_epoch`/`activation_height` پہلی بار pending ویلیڈیٹر کے
active ہونے پر سیٹ ہوتے ہیں، اور پہلے epoch/height پر دوبارہ فعال کرنے کی کوشش رد کی جاتی ہے۔
Pending ویلیڈیٹرز خودکار طور پر اس پہلے بلاک کے آغاز پر promote ہوتے ہیں جس کا epoch طے شدہ حد کو پہنچے،
اور activation metrics کاؤنٹر (`nexus_public_lane_validator_activation_total`) اس promotion کو status تبدیلی
کے ساتھ ریکارڈ کرتا ہے۔

Permissioned deployments genesis peer roster کو فعال رکھتے ہیں حتی کہ پبلک لین ویلیڈیٹر stake موجود نہ ہو:
جب تک peers کے consensus keys زندہ ہوں، runtime validator set کے لئے genesis peers پر fallback کرتا ہے۔
یہ bootstrap deadlock سے بچاتا ہے جب staking admission غیر فعال ہو یا rollout میں ہو۔

### 1.2 Stake shares اور unbonding

Delegators (اور وہ ویلیڈیٹرز جو اپنا bond بڑھاتے ہیں) `PublicLaneStakeShare` کے ذریعے ماڈل ہوتے ہیں:

- `bonded: Numeric` — فعال bonded مقدار۔
- `pending_unbonds: BTreeMap<Hash, PublicLaneUnbonding>` — pending withdrawals جو client فراہم کردہ `request_id`
  سے keyed ہیں۔
- `metadata` UX/back-office اشارے رکھتا ہے (مثلاً custody desk کے حوالہ نمبر)۔

`PublicLaneUnbonding` deterministic withdrawal شیڈول (`amount`, `release_at_ms`) رکھتا ہے۔ Torii اب live shares
اور pending withdrawals کو `GET /v1/nexus/public_lanes/{lane}/stake` کے ذریعے ایکسپوز کرتا ہے تاکہ wallets
بغیر bespoke RPCs کے ٹائمر دکھا سکیں۔

Lifecycle hooks (runtime enforced):

- `PendingActivation(epoch)` entries خودکار طور پر `Active` میں بدلتی ہیں جب موجودہ epoch `epoch` تک پہنچے۔
  Activation `activation_epoch` اور `activation_height` ریکارڈ کرتا ہے، اور regressions کو auto-activation اور
  `ActivatePublicLaneValidator` کی explicit calls دونوں میں رد کیا جاتا ہے۔
- `Exiting(releases_at_ms)` entries `Exited` میں بدلتی ہیں جب بلاک timestamp `releases_at_ms` سے آگے نکل جائے،
  stake-share rows صاف ہو جاتی ہیں تاکہ validator capacity دستی cleanup کے بغیر واپس آ سکے۔
- Reward recording validator shares کو رد کرتا ہے جب تک validator `Active` نہ ہو، تاکہ pending/exiting/jailed
  validators payouts accrue نہ کریں۔

### 1.3 Reward records

Reward distributions کے لئے `PublicLaneRewardRecord` اور `PublicLaneRewardShare` استعمال ہوتے ہیں:

```norito
{
  "lane_id": 1,
  "epoch": 4242,
  "asset": "xor#wonderland",
  "total_reward": "250.0000",
  "shares": [
    { "account": "validator@lane", "role": "Validator", "amount": "150" },
    { "account": "delegator@lane", "role": "Nominator", "amount": "100" }
  ],
  "metadata": {
    "telemetry_epoch_root": "0x4afe…",
    "distribution_tx": "0xaabbccdd"
  }
}
```

Records آڈیٹرز اور dashboards کو ہر payout کے لئے deterministic evidence فراہم کرتے ہیں۔
Reward struct `RecordPublicLaneRewards` ISI میں جاتی ہے۔

Runtime guards:

- Nexus builds فعال ہونے چاہئیں؛ offline/stub builds reward recording کو reject کرتے ہیں۔
- Reward epochs ہر lane میں monotonically آگے بڑھتے ہیں؛ stale یا duplicate epochs reject ہوتے ہیں۔
- Reward assets configured fee sink (`nexus.fees.fee_sink_account_id` /
  `nexus.fees.fee_asset_id`) سے match ہونے چاہئیں، اور sink balance کو `total_reward` مکمل cover کرنا چاہیے۔
- ہر share مثبت ہو اور reward asset کی numeric spec کو respect کرے؛ share totals کو `total_reward` کے برابر ہونا چاہیے۔

## 2. Instruction catalog

تمام instructions `iroha_data_model::isi::staking` کے تحت ہیں۔ ان کے لئے Norito encoders/decoders derive ہوتے ہیں
تاکہ SDKs بغیر bespoke codecs کے payloads submit کر سکیں۔

### 2.1 `RegisterPublicLaneValidator`

ایک validator رجسٹر کرتا ہے اور ابتدائی stake bond کرتا ہے:

```norito
{
  "lane_id": 1,
  "validator": "validator@lane",
  "stake_account": "validator@lane",
  "initial_stake": "150000",
  "metadata": {
    "commission_bps": 750,
    "jurisdiction": "JP",
    "telemetry_id": "val-01"
  }
}
```

Validation rules:

- `initial_stake` >= `min_self_stake` (گورننس پیرامیٹر)۔
- Metadata میں activation سے پہلے contact/telemetry hooks شامل ہونا لازمی ہے۔
- گورننس entry کو approve/deny کرتی ہے؛ تب تک status `PendingActivation` رہتا ہے اور runtime validator کو
  `Active` میں promote کرتا ہے جب target activation epoch پہنچ جائے اور اگلی epoch boundary آئے۔

### 2.2 `BondPublicLaneStake`

اضافی stake bond کرتا ہے (validator self-bond یا delegator contribution).

Key fields: `staker`, `amount`, اور اختیاری metadata۔ Runtime کو lane-specific limits نافذ کرنے ہیں
(`max_delegators`, `min_bond`, `commission caps`).

### 2.3 `SchedulePublicLaneUnbond`

Unbonding timer شروع کرتا ہے۔ submitters ایک deterministic `request_id`
(سفارش: `blake2b(invoice)`), `amount`, اور `release_at_ms` فراہم کرتے ہیں۔ Runtime کو یہ verify کرنا چاہیے کہ
`amount` <= bonded stake ہو اور `release_at_ms` کو configured unbonding period کے مطابق clamp کرے۔

### 2.4 `FinalizePublicLaneUnbond`

ٹائمر ختم ہونے کے بعد، یہ ISI pending stake کو unlock کر کے `staker` کو واپس کرتا ہے۔ Executor request id
validate کرتا ہے، unlock timestamp کو ماضی میں ہونے کی تصدیق کرتا ہے، `PublicLaneStakeShare` update emit کرتا ہے،
اور telemetry record کرتا ہے۔

### 2.5 `SlashPublicLaneValidator`

گورننس اس instruction کو stake debit اور validators کو jail/eject کرنے کے لئے استعمال کرتی ہے۔

- `slash_id` ایونٹ کو telemetry + incident docs سے جوڑتا ہے۔
- `reason_code` ایک مستحکم enum string ہے (مثلاً `double_sign`, `downtime`, `safety_violation`).
- `metadata` evidence bundles کے hashes، runbook pointers، یا regulator IDs محفوظ کرتا ہے۔

Slashes governance policy کے مطابق delegators تک پہنچتے ہیں (proportional یا validator-first loss).
NX-9 کے بعد runtime logic `PublicLaneRewardRecord` annotations emit کرے گی۔

### 2.6 `RecordPublicLaneRewards`

کسی epoch کی payout ریکارڈ کرتا ہے۔ Fields:

- `reward_asset`: تقسیم ہونے والا asset (default `xor#nexus`).
- `total_reward`: minted/transferred total۔
- `shares`: `PublicLaneRewardShare` entries کا vector۔
- `metadata`: payout transactions، root hashes، یا dashboards کے حوالہ جات۔

یہ ISI `(lane_id, epoch)` کے لئے idempotent ہے اور nightly accounting کی بنیاد ہے۔

## 3. Operations، lifecycle، اور tooling

- **Lifecycle + modes:** stake-elected lanes کو `nexus.staking.public_validator_mode = stake_elected` کے ذریعے
  فعال کیا جاتا ہے جبکہ restricted lanes admin-managed رہتی ہیں (`nexus.staking.restricted_validator_mode = admin_managed`).
  Permissioned deployments stake آنے تک genesis peers کو فعال رکھتے ہیں؛ stake-elected lanes کے لئے ہم اب بھی
  commit topology میں ایک registered peer کے live consensus key کی موجودگی مانگتے ہیں تاکہ `RegisterPublicLaneValidator`
  succeed ہو۔ Genesis fingerprints اور `use_stake_snapshot_roster` یہ طے کرتے ہیں کہ runtime roster کو stake snapshots
  سے derive کرے یا genesis peers پر fallback کرے۔
- **Activation/exit operations:** registrations `PendingActivation` میں آتی ہیں اور `epoch_length_blocks` حد پوری
  ہونے والے پہلے بلاک پر auto-promote ہوتی ہیں۔ Operators حد کے بعد `ActivatePublicLaneValidator` call کر کے
  promotion فورس کر سکتے ہیں۔ Exits validators کو `Exiting(release_at_ms)` میں لے جاتی ہیں اور capacity صرف
  اسی وقت آزاد ہوتی ہے جب بلاک timestamp `release_at_ms` تک پہنچ جائے؛ slash کے بعد re-registration کے لئے
  exit ضروری رہتا ہے تاکہ ریکارڈ `Exited` ہو اور capacity reclaimed ہو۔ Capacity checks `nexus.staking.max_validators`
  استعمال کرتی ہیں اور exit finalizer کے بعد چلتی ہیں، اس لئے future-dated exits نئے registrations کو ٹائمر ختم
  ہونے تک بلاک کرتے ہیں۔
- **Config knobs:** `nexus.staking.min_validator_stake`, `nexus.staking.stake_asset_id`,
  `nexus.staking.stake_escrow_account_id`, `nexus.staking.slash_sink_account_id`,
  `nexus.staking.unbonding_delay`, `nexus.staking.withdraw_grace`, `nexus.staking.max_validators`,
  `nexus.staking.max_slash_bps`, `nexus.staking.reward_dust_threshold`, اور اوپر والے validator-mode switches۔
  انہیں `iroha_config::parameters::actual::Nexus` کے ذریعے تھریڈ کریں اور GA اقدار منظور ہونے کے بعد
  `status.md` میں ظاہر کریں۔
- **Torii/CLI quickstart:**
  - `iroha nexus lane-report --summary` lane catalog entries، manifest readiness، اور validator modes
    (stake-elected vs admin-managed) دکھاتا ہے تاکہ آپریٹرز confirm کر سکیں کہ کسی lane کے لئے staking admission فعال ہے۔
  - `iroha_cli nexus public-lane validators --lane <id> [--summary] [--address-format {ih58,compressed}]`
    lifecycle/activation markers (pending target epoch, `activation_epoch` / `activation_height`, exit release, slash id)
    کو bonded/self stake کے ساتھ دکھاتا ہے۔
    `iroha_cli nexus public-lane stake --lane <id> [--validator account@domain] [--summary]`
    `/stake` endpoint کو `(validator, staker)` جوڑی کے pending-unbond hints کے ساتھ mirror کرتا ہے۔
  - Torii snapshots for dashboards and SDKs:
    - `GET /v1/nexus/public_lanes/{lane}/validators` – metadata, status
      (`PendingActivation`/`Active`/`Exiting`/`Exited`/`Slashed`), activation epoch/height,
      release timers, bonded stake, last reward epoch.
      `address_format=ih58|compressed` literal rendering کو کنٹرول کرتا ہے۔
    - `GET /v1/nexus/public_lanes/{lane}/stake` – stake shares (`validator`,
      `staker`, bonded amount) کے ساتھ pending unbond timers۔ `?validator=account@domain` response کو
      ایک validator فوکس والے dashboards کے لئے filter کرتا ہے؛ `address_format` سب literals پر لاگو ہوتا ہے۔
  - Lifecycle ISIs standard transaction path استعمال کرتے ہیں (Torii `/v1/transactions`
    یا CLI instruction pipeline)۔ مثال Norito JSON payloads:

    ```jsonc
    [
      { "ActivatePublicLaneValidator": { "lane_id": 1, "validator": "validator@nexus" } },
      {
        "ExitPublicLaneValidator": {
          "lane_id": 1,
          "validator": "validator@nexus",
          "release_at_ms": 1730000000000
        }
      }
    ]
    ```
- **Telemetry + runbooks:** metrics validator counts، bonded/pending stake، reward totals، اور slash counters کو
  `nexus_public_lane_*` فیملی میں ظاہر کرتی ہیں۔ Dashboards کو اسی data set سے وائر کریں جو NX-9 acceptance tests
  میں استعمال ہوتا ہے تاکہ validator deltas اور reward/slash evidence auditable رہے۔ Slashing instructions
  governance-only رہتی ہیں؛ reward recording کو payout totals ثابت کرنے ہوں گے (payout batch hash).

## 4. Roadmap alignment

- ✅ Runtime اور WSV storages NX-9 validator lifecycle نافذ کرتے ہیں؛ regressions activation timing، peer prerequisites،
  delayed exits، اور slashes کے بعد re-registration کو کور کرتے ہیں۔
- ✅ Torii `/v1/nexus/public_lanes/{lane}/{validators,stake,rewards/pending}` Norito JSON کے ساتھ ایکسپوز کرتا ہے تاکہ
  SDKs اور dashboards custom RPCs کے بغیر lane state مانیٹر کر سکیں۔
- ✅ Config اور telemetry knobs دستاویزی ہیں؛ mixed deployments stake-elected اور admin-managed lanes کو الگ رکھتے ہیں
  تاکہ validator rosters deterministic رہیں۔

</div>

### 2.7 `CancelConsensusEvidencePenalty`

Cancels consensus slashing before the delayed penalty applies.

- `evidence`: the Norito-encoded `Evidence` payload that was recorded in `consensus_evidence`.
- The record is marked `penalty_cancelled` and `penalty_cancelled_at_height`, preventing slashing when `slashing_delay_blocks` elapses.
