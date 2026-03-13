---
lang: ja
direction: ltr
source: docs/portal/docs/nexus/nexus-default-lane-quickstart.ar.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
ID: nexus-default-lane-quickstart
タイトル: レーン レーン (NX-5)
サイドバーラベル: レーン レーン レーン
説明: レーンのフォールバック、Nexus、Torii、SDK のレーン ID、レーンの数。
---

:::メモ
テストは `docs/source/quickstart/default_lane.md` です。 حافظ على النسختين متطابقتين حتى يصل مسح التوطين إلى البوابة。
:::

# レーン レーン (NX-5)

> **情報:** NX-5 - レーンを表示します。フォールバック `nexus.routing_policy.default_lane` フォールバック REST/gRPC Torii SDK フォールバック `nexus.routing_policy.default_lane` `lane_id` は正規のレーンです。 يوجه هذا الدلين لإعداد الكتالوج، والتحقق من fallback في `/status`، واختبار سلوك العميلああ、それは。

## ああ、ああ

- ソラ/Nexus 、 `irohad` (`irohad --sora --config ...`)。
- صول إلى مستودع الإعدادات حتى تتمكن من تعديل أقسام `nexus.*`.
- `iroha_cli` مهيأ للتحدث مع عنقود الهدف.
- `curl`/`jq` (英語) `/status` في Torii。

## 1. レーンとデータスペース

レーンとデータスペースの両方が可能です。レーン数とデータスペースのエイリアス:

```toml
[nexus]
lane_count = 3

[[nexus.lane_catalog]]
index = 0
alias = "core"
description = "Primary execution lane"
dataspace = "universal"

[[nexus.lane_catalog]]
index = 1
alias = "governance"
description = "Governance & parliament traffic"
dataspace = "governance"

[[nexus.lane_catalog]]
index = 2
alias = "zk"
description = "Zero-knowledge attachments"
dataspace = "zk"

[[nexus.dataspace_catalog]]
alias = "universal"
id = 0
description = "Single-lane data space"
fault_tolerance = 1

[[nexus.dataspace_catalog]]
alias = "governance"
id = 1
description = "Governance proposals & manifests"
fault_tolerance = 1

[[nexus.dataspace_catalog]]
alias = "zk"
id = 2
description = "Zero-knowledge proofs and attachments"
fault_tolerance = 1
```

أن يكون كل `index` فريدا ومتتاليا。データスペース هي قيم 64-بت؛レーンを通過してください。

## 2. いいえ、いいえ、いいえ、いいえ。

قسم `nexus.routing_policy` يتحكم في LANE الاحتياطي ويسمح بتجاوز التوجيه لتعليمات محددة أو بادئاتああ。スケジューラーは `default_lane` と `default_dataspace` をサポートします。ルータは `crates/iroha_core/src/queue/router.rs` ويطبق السياسة بشكل شفاف على واجهات Torii REST/gRPC。

```toml
[nexus.routing_policy]
default_lane = 0                # use the "core" lane when no rules match
default_dataspace = "universal"    # reuse the public dataspace for the fallback

[[nexus.routing_policy.rules]]
lane = 1
dataspace = "governance"
[nexus.routing_policy.rules.matcher]
instruction = "governance"
description = "Route governance instructions to the governance lane"

[[nexus.routing_policy.rules]]
lane = 2
dataspace = "zk"
[nexus.routing_policy.rules.matcher]
instruction = "smartcontract::deploy"
description = "Route contract deployments to the zk lane for proof tracking"
```

レーン数は 1 つです。レーン レーン レーン レーン レーン レーン レーン レーン レーン レーン レーン レーン レーン レーン レーン レーン レーン レーン レーン レーン レーン レーン レーン レーン レーン レーン レーン レーン レーン レーン レーン レーン レーン レーン レーン レーン レーン レーン レーン レーン レーン レーン レーン レーン レーン レーン レーン レーン レーン レーン レーン レーン レーン 性 レーン レーン レーン レーン レーン レーン レーン レーン レーン レーン レーン レーン レーン レーン レーン レーン レーン レーン レーン レーン 性 レーン レーン レーン レーン レーン レーン レーン レーン レーン レーン レーン レーン レーン レーン レーン レーン レーン レーン レーン レーン レーン レーン レーン レーン レーン レーン レーン レーン レーン レーン レーン レーン レーン レーン レーン レーン レーン レーン レーン レーン レーン レーン レーン レーン レーン レーン レーン レーン レーン レーン レーン レーン レーン。そうです。

## 3. إقلاع عقدة مع تطبيق السياسة

```bash
IROHA_CONFIG=/path/to/nexus/config.toml
irohad --sora --config "${IROHA_CONFIG}"
```

あなたのことを忘れないでください。 تظهر أي أخطاء تحقق (فهارس مفقودة، aliases مكررة، معرفات dataspace غير صالحة) ゴシップ。

## 4. レーン

オンラインでのアクセスと、オンラインでのアクセスと、CLI でのレーンのアクセス (マニフェスト محمّل) وجاهزすごい。レーン:

```bash
iroha_cli app nexus lane-report --summary
```

出力例:

```
Lane  Alias            Module           Status  Quorum  Validators  Detail
   0  core             parliament       ready      05           07  manifest ok
   1  governance       parliament       ready      05           07  manifest ok
   2  zk               parliament       sealed     03           05  manifest required
```

レーンを実行してください。`sealed` ランブックを実行してください。 `--fail-on-sealed` CI です。

## 5. فحص حمولة حالة Torii

`/status` のスケジューラ レーン。 `curl`/`jq` を表示します。回答:

```bash
curl -s http://127.0.0.1:8080/status | jq '.nexus.routing_policy'
```

出力例:

```json
{
  "default_lane": 0,
  "default_dataspace": "universal",
  "rules": [
    {"lane": 1, "dataspace": "governance", "matcher": {"instruction": "governance"}},
    {"lane": 2, "dataspace": "zk", "matcher": {"instruction": "smartcontract::deploy"}}
  ]
}
```

レーン `0`: スケジューラ レーン `0`:

```bash
curl -s http://127.0.0.1:8080/status \
  | jq '.nexus_scheduler_lane_teu_status[] | select(.lane_id == 0)
        | {lane_id, alias, dataspace_alias, committed, manifest_ready, scheduler_utilization_pct}'
```

هذا يؤكد أن لقطة TEU وبيانات エイリアス ورايات マニフェスト تتطابق مع الإعداد。 Grafana レーン取り込み。

## 6. いいえ

- **Rust/CLI.** `iroha_cli` およびクレート Rust のバージョン `lane_id` および `--lane-id` / `LaneSelector`。キュー ルーター `default_lane`。 `--lane-id`/`--dataspace-id` レーン レーンを確認してください。
- **JS/Swift/Android.** SDK の `laneId`/`lane_id` كاختيارية وتعود إلى القيمة المعلنة `/status`。制作、演出、制作、制作、制作、制作、制作、制作、制作、制作、制作、制作、制作やあ。
- **パイプライン/SSE テスト。** テスト `tx_lane_id == <u32>` (`docs/source/pipeline.md`)。 شترك في `/v2/pipeline/events/transactions` بهذا الإثبات أن الكتابات المرسلة بدون レーン صريح تصل تحت معرف LANEああ。

## 7. いいえ

- `/status` ينشر ايضا `nexus_lane_governance_sealed_total` و `nexus_lane_governance_sealed_aliases` كي يتمكن Alertmanager のレーン マニフェスト。開発ネットを開発してください。
- スケジューラ レーン (`dashboards/grafana/nexus_lanes.json`) のエイリアス/スラッグ。 إذا اعدت تسمية alias، اعد تسمية دلائل Kura المقابلة كي يحافظ المدققون على مسارات حتمية (NX-1)。
- レーンをロールバックします。ハッシュ マニフェスト マネージャ ランブック ランブック マネージャ マニフェスト マニフェスト マニフェスト マニフェスト マネージド ランブックすごいです。