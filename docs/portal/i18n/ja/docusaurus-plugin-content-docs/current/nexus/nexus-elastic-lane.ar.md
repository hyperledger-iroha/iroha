---
lang: ja
direction: ltr
source: docs/portal/docs/nexus/nexus-elastic-lane.ar.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
ID: nexus-elastic-lane
タイトル: تجهيز LANE المرن (NX-7)
サイドバーラベル: レーン レーン
説明: ブートストラップ マニフェスト Nexus レーン ロールアウト。
---

:::メモ
テストは `docs/source/nexus_elastic_lane.md` です。 حافظ على النسختين متطابقتين حتى يصل مسح الترجمة الى البوابة。
:::

# مجموعة ادوات تجهيز lan المرن (NX-7)

> **評価:** NX-7 - 評価レーン  
> **回答:** 回答 - マニフェストの回答 Norito 回答 煙の確認
> バンドル スロット + マニフェスト バンドル セキュリティ スロット + マニフェストああ
> دون سكربتات مخصصة。

هذا الدليل يوجه المشغلين عبر المساعد الجديد `scripts/nexus_lane_bootstrap.sh` الذي يقوم بأتمتة توليد マニフェスト لـ laneレーン/データスペースのロールアウト。レーン数 Nexus (レーン数) レーン数 (レーン数) 数。 عادة اشتقاق هندسة الكتالوج يدويا。

## 1. いいえ

1. エイリアス レーンとデータスペース ومجموعة المدققين وتحمّل الاعطال (`f`) وسياسة 決済。
2. قائمة نهائية بالمدققين (معرفات الحسابات) وقائمة 名前空間 المحمية。
3. وصول الى مستودع تهيئة العقد كي تتمكن من اضافة المقتطفات المولدة.
4. マニフェストはレーン (`nexus.registry.manifest_directory` و `cache_directory`) を表します。
5. レーンへのアクセス / レーンへのアクセス PagerDuty レーンへのアクセスそうです。

## 2. レーンのアーティファクト

回答:

```bash
scripts/nexus_lane_bootstrap.sh \
  --lane-alias "Payments Lane" \
  --lane-id 3 \
  --dataspace-alias payments \
  --governance-module parliament \
  --settlement-handle xor_global \
  --validator ih58... \
  --validator ih58... \
  --validator ih58... \
  --protected-namespace payments \
  --description "High-throughput interbank payments lane" \
  --dataspace-description "Payments dataspace" \
  --route-instruction finance::pacs008 \
  --encode-space-directory \
  --space-directory-out artifacts/nexus/payments_lane/payments.manifest.to \
  --telemetry-contact payments-ops@sora.org \
  --output-dir artifacts/nexus/payments_lane
```

回答:

- `--lane-id` يجب ان يطابق インデックス الادخال الجديد في `nexus.lane_catalog`。
- `--dataspace-alias` と `--dataspace-id/hash` يتحكمان في ادخال كتالوج データスペース (افتراضيا يستخدم id الخاص بالlane عند الحذف)。
- `--validator` は、`--validators-file` です。
- `--route-instruction` / `--route-account` を確認してください。
- `--metadata key=value` (`--telemetry-contact/channel/runbook`) は、ランブックを実行するために使用されます。
- `--allow-runtime-upgrades` + `--runtime-upgrade-*` フック ランタイム アップグレードとマニフェストとレーンの実行。
- `--encode-space-directory` يستدعي `cargo xtask space-directory encode` 評価。テスト `--space-directory-out` テスト `.to` テストを実行してください。

ينتج السكربت ثلاثة アーティファクト داخل `--output-dir` (الافتراضي هو المجلد الحالي) ، مع رابع اختياري عندエンコーディング:

1. `<slug>.manifest.json` - レーン クォーラムと名前空間のマニフェスト フック ランタイム アップグレード。
2. `<slug>.catalog.toml` - مقتطف TOML يحتوي `[[nexus.lane_catalog]]` و `[[nexus.dataspace_catalog]]` واي قواعد توجيه مطلوبة。 `fault_tolerance` レーン リレー データスペース (`3f+1`)。
3. `<slug>.summary.json` - ロールアウト ロールアウト ロールアウト ロールアウト ロールアウト`cargo xtask space-directory encode` (`space_directory_encode.command`)。 JSON のオンボーディング كدليل。
4. `<slug>.manifest.to` - يصدر عند تفعيل `--encode-space-directory`؛ `iroha app space-directory manifest publish` と Torii を確認してください。

`--dry-run` JSON/国際標準化 `--force` 国際アーティファクトああ。

## 3. いいえ

1. マニフェスト JSON `nexus.registry.manifest_directory` キャッシュ ディレクトリ (キャッシュ ディレクトリ レジストリ リモート)。重要な問題は、重要な問題を明らかにします。
2. `config/config.toml` (`config.d/*.toml`)。 `nexus.lane_count` يساوي على الاقل `lane_id + 1` وحدّث اي `nexus.routing_policy.rules` يجب ان تشير الى lan الجديد。
3. 宇宙ディレクトリのマニフェスト (`space_directory_encode.command`)。 هذا ينتج ペイロード `.manifest.to` الذي يتوقعه Torii ويسجل الادلة للمراجعين؛ `iroha app space-directory manifest publish`。
4. 「`irohad --sora --config path/to/config.toml --trace-config`」ロールアウトをトレースします。 هذا يثبت ان الهندسة الجديدة تطابق slug/قطاعات Kura المولدة.
5. マニフェスト/マニフェストを表示します。 JSON の概要を説明します。

## 4. バンドルの内容

マニフェストとオーバーレイの管理、レーンの管理、設定の管理。バンドラー ينسخ マニフェスト マニフェスト オーバーレイ オーバーレイ `nexus.registry.cache_directory`، ويمكنهオフラインでの tarball の使用:

```bash
scripts/nexus_lane_registry_bundle.sh \
  --manifest artifacts/nexus/payments_lane/payments.manifest.json \
  --output-dir artifacts/nexus/payments_lane/registry_bundle \
  --default-module parliament \
  --module name=parliament,module_type=parliament,param.quorum=2 \
  --bundle-out artifacts/nexus/payments_lane/registry_bundle.tar.gz
```

意味:1. `manifests/<slug>.manifest.json` - 回答 `nexus.registry.manifest_directory` 回答。
2. `cache/governance_catalog.json` - ضعها في `nexus.registry.cache_directory`。 كل ادخال `--module` يصبح تعريفا لوحدة قابلة للتبديل، ما يتيح تبديل وحدات الحوكمة (NX-2)オーバーレイ `config.toml` を確認してください。
3. `summary.json` - ハッシュのオーバーレイ。
4. `registry_bundle.tar.*` - SCP および S3 のアーティファクト。

マニフェスト + オーバーレイ マニフェスト + オーバーレイ ホスト マニフェスト + オーバーレイTorii を参照してください。

## 5. 煙が出る

بعد اعادة تشغيل Torii، شغّل مساعد sm الجديد للتحقق من ان lan يبلّغ `manifest_ready=true`، وانレーンは封印されています。レーンのマニフェスト `manifest_path` のレーンNX-7 のマニフェストを確認してください:

```bash
scripts/nexus_lane_smoke.py \
  --status-url https://torii.example.com/v1/sumeragi/status \
  --metrics-url https://torii.example.com/metrics \
  --lane-alias payments \
  --expected-lane-count 3 \
  --min-da-quorum 0.95 \
  --max-oracle-staleness 75 \
  --expected-oracle-twap 60 \
  --oracle-twap-tolerance 5 \
  --max-oracle-haircut-bps 75 \
  --min-settlement-buffer 0.25 \
  --min-block-height 1000 \
  --max-finality-lag 4 \
  --max-settlement-backlog 0.5 \
  --max-headroom-events 0 \
  --max-slot-p95 1000 \
  --max-slot-p99 1100 \
  --min-slot-samples 10
```

`--insecure` 自己署名。 خرج السكربت برمز غير صفري اذا كانت レーン مفقودة او 封印された او اذا انحرفت المقاييس/القياس عن القيمああ。 ستخدم `--min-block-height` و `--max-finality-lag` و `--max-settlement-backlog` و `--max-headroom-events` للحفاظ على القياس لكل レーン (ارتفاع)応答/バックログ/ヘッドルーム) 応答 `--max-slot-p95` / `--max-slot-p99` (応答 `--min-slot-samples`)スロット NX-18 のスロットが表示されます。

エアギャップ (CI) セキュリティ Torii セキュリティ エンドポイント:

```bash
scripts/nexus_lane_smoke.py \
  --status-file fixtures/nexus/lanes/status_ready.json \
  --metrics-file fixtures/nexus/lanes/metrics_ready.prom \
  --lane-alias core \
  --lane-alias payments \
  --expected-lane-count 3 \
  --min-da-quorum 0.95 \
  --max-oracle-staleness 75 \
  --expected-oracle-twap 60 \
  --oracle-twap-tolerance 5 \
  --max-oracle-haircut-bps 75 \
  --min-settlement-buffer 0.25 \
  --min-block-height 1000 \
  --max-finality-lag 4 \
  --max-settlement-backlog 0.5 \
  --max-headroom-events 0 \
  --max-slot-p95 1000 \
  --max-slot-p99 1100 \
  --min-slot-samples 10
```

フィクスチャ `fixtures/nexus/lanes/` アーティファクト ブートストラップ lint マニフェストありがとうございます。 `ci/check_nexus_lane_smoke.sh` و `ci/check_nexus_lane_registry_bundle.sh` (別名: `make check-nexus-lanes`) は、煙を出し、NX-7 を実行します。ペイロードとダイジェスト/オーバーレイのバンドルのダウンロード。