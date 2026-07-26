---
lang: ja
direction: ltr
source: docs/portal/docs/soranet/privacy-metrics-pipeline.ar.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
ID: プライバシー-メトリクス-パイプライン
タイトル: مسار مقاييس الخصوصية في SoraNet (SNNet-8)
サイドバーラベル: 重要な情報
説明: SoraNet とオーケストレーターのテレメトリ。
---

:::メモ
`docs/source/soranet/privacy_metrics_pipeline.md`。 حافظ على النسختين متطابقتين حتى يتم تقاعد مجموعة الوثائق القديمة。
:::

# مسار مقاييس الخصوصية في SoraNet

SNNet-8 テレメトリ リレー。リレー、ハンドシェイク、サーキット、バケット、Prometheus、サーキット、バケットمحافظا على عدم ربط الدوائر الفردية بينما يمنح المشغلين رؤية قابلة للعمل。

## ナオミ

- テスト `tools/soranet-relay/src/privacy.rs` テスト `PrivacyAggregator`。
- バケット بدقيقة وقت الجدار (`bucket_secs`، الافتراضي 60 ثانية) وتخزينها في حلقة محدودة (`max_completed_buckets`، الافتراضي 120)。株式数、コレクター数、コレクター数 (`max_share_lag_buckets`، الافتراضي 12) 保有数、プリオ数コレクターは、バケツを抑制しました。
- `RelayConfig::privacy` يطابق مباشرة `PrivacyConfig` ويعرض مفاتيح الضبط (`bucket_secs`、`min_handshakes`、`flush_delay_buckets`、 `force_flush_buckets`、`max_completed_buckets`、`max_share_lag_buckets`、`expected_shares`)。 SNNet-8a は、SNNet-8a をサポートしています。
- ランタイム ヘルパーの説明: `record_circuit_accepted`、`record_circuit_rejected`、`record_throttle`、`record_throttle_cooldown`、`record_capacity_reject`、 `record_active_sample`、`record_verified_bytes`、`record_gar_category`。

## 管理者リレー

管理者はリレーを管理し、`GET /privacy/events` を監視します。 JSON محدد الاسطر (`application/x-ndjson`) يحتوي على حمولة `SoranetPrivacyEventV1` منعكسة من `PrivacyEventBuffer`ああ。 يحتفظ المخزن بأحدث الاحداث حتى `privacy.event_buffer_capacity` إدخالا (الافتراضي 4096) ويتم تفريغه عند危険なスクレーパーを危険にさらします。ハンドシェイク、スロットル、検証済みの帯域幅、アクティブ回路、GAR の評価、Prometheus の収集、コレクターの評価パンくずリストは、パンくずリストを表示します。

## リレー

テレメトリ、リレー、`privacy`:

```json
{
  "mode": "Entry",
  "listen": "0.0.0.0:443",
  "privacy": {
    "bucket_secs": 60,
    "min_handshakes": 12,
    "flush_delay_buckets": 1,
    "force_flush_buckets": 6,
    "max_completed_buckets": 120,
    "max_share_lag_buckets": 12,
    "expected_shares": 2
  }
}
```

SNNet-8 へのアクセス:

|ああ |ああ | और देखें
|----------|----------|----------|
| `bucket_secs` | عرض كل نافذة تجميع (ثوان)。 | `60` |
| `min_handshakes` |大きなバケツを持っています。 | `12` |
| `flush_delay_buckets` |バケットは、バケツを持っています。 | `1` |
| `force_flush_buckets` |バケットが抑制されました。 | `6` |
| `max_completed_buckets` | متأخرات バケット المحتفظ بها (تمنع ذاكرة غير محدودة)。 | `120` |
| `max_share_lag_buckets` |コレクター株式の抑制。 | `12` |
| `expected_shares` |プリオコレクターは株式を保有しています。 | `2` |
| `event_buffer_capacity` | NDJSON 管理者です。 | `4096` |

ضبط `force_flush_buckets` اقل من `flush_delay_buckets`، او تصفير العتبات، او تعطيل حارس الاحتفاظ يفشل التحققテレメトリリレーを監視します。

حد `event_buffer_capacity` يقيد ايضا `/admin/privacy/events`، مما يضمن عدم تمكن scrapers من التاخر بلا نهاية.

## Prio コレクター株

SNNet-8a コレクター、バケット、プリオのコレクション。オーケストレーターは `/privacy/events` NDJSON を所有し、`SoranetPrivacyEventV1` は `SoranetPrivacyPrioShareV1` を共有し、`SoranetSecureAggregator::ingest_prio_share` を所有しています。バケット リレー `PrivacyBucketConfig::expected_shares` مساهمات، بما يعكس سلوك リレー。株式数、バケット、ヒストグラム、`SoranetPrivacyBucketMetricsV1`。ハンドシェイク المدمج عن `min_contributors`، يتم تصدير バケット كـ `suppressed` بما يعكس سلوك المجمعリレー。抑制された `suppression_reason` および `insufficient_contributors`、`collector_suppressed`、 `collector_window_elapsed`、`forced_flush_window_elapsed` テレメトリ。ピオ株 `collector_window_elapsed` ピオ株 `max_share_lag_buckets` コレクターズ コイン最高です。

## 番号 Torii

Torii HTTP テレメトリ、リレー、コレクターの監視意味:- `POST /v1/soranet/privacy/event` يقبل حمولة `RecordSoranetPrivacyEventDto`。 `SoranetPrivacyEventV1` は `source` です。 يتحقق Torii من الطلب مقابل ملف telemetry النشط، يسجل الحدث، ويرد بـ HTTP `202 Accepted` مع مغلف Norito JSON يحتوي على نافذة الحساب (`bucket_start_unix`、`bucket_duration_secs`) リレー。
- `POST /v1/soranet/privacy/share` يقبل حمولة `RecordSoranetPrivacyShareDto`。コレクターの皆様。 HTTP `202 Accepted` Norito JSON コレクター、バケット、抑制遠隔測定 `Conversion` の遠隔測定コレクター。オーケストレーター、株式、リレー、オーケストレーター、株式、リレー、その他の組織。 Torii バケットリレー。

テレメトリの情報: `503 Service Unavailable` の情報。 Norito (`application/x.norito`) Norito JSON (`application/x.norito+json`) Torii を確認してください。

## مقاييس Prometheus

`mode` (`entry`、`middle`、`exit`) および `bucket_start`。セキュリティ:

|メトリック |説明 |
|----------|---------------|
| `soranet_privacy_circuit_events_total{kind}` |握手は `kind={accepted,pow_rejected,downgrade,timeout,other_failure,capacity_reject}` です。 |
| `soranet_privacy_throttles_total{scope}` |スロットル `scope={congestion,cooldown,emergency,remote_quota,descriptor_quota,descriptor_replay}`。 |
| `soranet_privacy_throttle_cooldown_millis_{sum,count}` |クールダウン、ハンドシェイクの制限。 |
| `soranet_privacy_verified_bytes_total` |帯域幅は非常に重要です。 |
| `soranet_privacy_active_circuits_{avg,max}` |バケット。 |
| `soranet_privacy_rtt_millis{percentile}` | RTT (`p50`、`p90`、`p99`) を参照してください。 |
| `soranet_privacy_gar_reports_total{category_hash}` |ガバナンス行動報告書、ダイジェスト版。 |
| `soranet_privacy_bucket_suppressed` |バケットは、非常に重要です。 |
| `soranet_privacy_pending_collectors{mode}` |コレクターはリレーを共有します。 |
| `soranet_privacy_suppression_total{reason}` |バケットは抑制されました、`reason={insufficient_contributors,collector_suppressed,collector_window_elapsed,forced_flush_window_elapsed}` は抑制されました。 |
| `soranet_privacy_snapshot_suppression_ratio` |勝利は抑制された/ドレイン (0-1) でした。 |
| `soranet_privacy_last_poll_unixtime` | UNIX は、コレクターアイドルです。 |
| `soranet_privacy_collector_enabled` |ゲージ يتحول الى `0` عندما يتعطل コレクター الخصوصية او يفشل بالبدء (يغذي تنبيه コレクター無効)。 |
| `soranet_privacy_poll_errors_total{provider}` | اخفاقات الاستطلاع مجمعة حسب エイリアスリレー (تزداد عند اخطاء فك الترميز، او اخفاقات HTTP، او اكواد حالة غير متوقعة）。 |

バケツを壊す必要はありません。

## いいえ

1. ** - 世界 - 世界 - `mode` と `window_start`。コレクターリレー。 `soranet_privacy_suppression_total{reason}` は、抑圧、コレクター、およびコレクターの活動を支援します。 يشحن اصل Grafana الآن لوحة **「抑制理由 (5 分)」** مخصصة تغذيها تلك العدادات، بالاضافة الى stat **「抑制されたバケット %」** الذي يحسب `sum(soranet_privacy_bucket_suppressed) / count(...)` لكل اختيار حتى يتمكن المشغلون من رصد تجاوزات الميزانيةやあ。 **「コレクター シェア バックログ」** (`soranet_privacy_pending_collectors`) 統計 **「スナップショット抑制率」** コレクターの統計情報ああ。
2. **التنبيه** - بقيادة الانذارات من عدادات آمنة للخصوصية: ارتفاعات رفض PoW، تواتر Cooldown، انجراف RTT です。お金を節約するために、お金を節約する必要があります。
3. **استجابة الحوادث** - اعتمد على البيانات المجمعة اولا。リレー、バケット、バケット、リレー、リレー、バケット、ネットワークすごいです。
4. **الاحتفاظ** - السحب البيانات بما يكفي لتجنب تجاوز `max_completed_buckets`。輸出業者は Prometheus をバケツに保管します。

## 抑圧 والتشغيل الآلي

SNNet-8 の監視 コレクターの監視 監視の抑圧 監視の監視(≤10% のバケット リレー数 30 個)。 دوات اللازمة لتلبية هذه البوابة تشحن الآن مع الشجرة؛ على المشغلين ربطها بطقوسهم الاسبوعية。抑制を抑制 Grafana を PromQL を使用して、 يمنح فرق المناوبة رؤية مباشرةありがとうございます。

### PromQL の抑制

يجب على المشغلين ابقاء مساعدات PromQL التالية قريبة؛警告 Grafana 警告 (`dashboards/grafana/soranet_privacy_metrics.json`) 警告マネージャー:

```promql
/* Suppression ratio per relay mode (30 minute window) */
(
  increase(soranet_privacy_suppression_total{reason=~"insufficient_contributors|collector_suppressed|collector_window_elapsed|forced_flush_window_elapsed"}[30m])
) /
clamp_min(
  increase(soranet_privacy_circuit_events_total{kind="accepted"}[30m]) +
  increase(soranet_privacy_suppression_total[30m]),
1
)
```

```promql
/* Detect new suppression spikes above the permitted minute budget */
increase(soranet_privacy_suppression_total{reason=~"insufficient_contributors|collector_window_elapsed|collector_suppressed"}[5m])
/
clamp_min(
  sum(increase(soranet_privacy_circuit_events_total{kind="accepted"}[5m])),
1
)
```ステータス **「抑制されたバケット %」** يبقى تحت ميزانية السياسة؛アラートマネージャー アラートマネージャー アラートマネージャー アラートマネージャー アラートマネージャー アラートマネージャー アラートマネージャー アラートマネージャー アラートマネージャー アラートマネージャーそうです。

### バケット数

ワークスペース `cargo xtask soranet-privacy-report` NDJSON が表示されます。輸出管理者リレー:

```bash
cargo xtask soranet-privacy-report \
  --input artifacts/sorafs_privacy/relay-a.ndjson \
  --input artifacts/sorafs_privacy/relay-b.ndjson \
  --json-out artifacts/sorafs_privacy/relay_summary.json
```

`SoranetSecureAggregator` の抑制 stdout の抑制 ويكتب اختياريا تقرير JSON の制限`--json-out <path|->`。コレクター المباشر (`--bucket-secs`, `--min-contributors`, `--expected-shares`, الخ)، ما يسمح للمشغلين باعادة最高のパフォーマンスを見せてください。 JSON を使用して Grafana を使用し、SNNet-8 を使用して抑制します。

### قائمة تدقيق التشغيل الآلي الاول

抑圧を阻止する。バケットは抑制されました。 (10%) バケットの数。回答:

1. NDJSON 管理者、リレー、`/v1/soranet/privacy/event|share`、オーケストレーター、`artifacts/sorafs_privacy/<relay>.ndjson`。
2. メッセージ:

   ```bash
   cargo xtask soranet-privacy-report \
     --input artifacts/sorafs_privacy/relay-a.ndjson \
     --input artifacts/sorafs_privacy/relay-b.ndjson \
     --json-out artifacts/sorafs_privacy/relay_summary.json \
     --max-suppression-ratio 0.10
   ```

   バケットのバケツ遠隔測定を行うことができます。 يجب ان تُظهر المقاييس المباشرة ان `soranet_privacy_pending_collectors` تتجه للصفر وان `soranet_privacy_snapshot_suppression_ratio` يبقى تحت نفس और देखें
3. JSON と CLI のセキュリティ SNNet-8 のセキュリティ セキュリティ セキュリティ セキュリティ セキュリティありがとうございます。

## 通信 (SNNet-8a)

- プリオ コレクター、ランタイム、リレー、コレクター、`SoranetPrivacyBucketMetricsV1` を共有します。 *(تم — راجع `ingest_privacy_payload` في `crates/sorafs_orchestrator/src/lib.rs` والاختبارات المصاحبة.)*
- 国際的な Prometheus の攻撃、抑圧、コレクターの攻撃。ああ。 *(تم — راجع `dashboards/grafana/soranet_privacy_metrics.json`, `dashboards/alerts/soranet_privacy_rules.yml`, `dashboards/alerts/soranet_policy_rules.yml` وملفات التحقق.)*
- انتاج مواد معايرة الخصوصية التفاضلية الموضحة في `privacy_metrics_dp.md` بما في ذلك دفاتر قابلةすごいです。 *(تم — تم توليد الدفتر والمواد بواسطة `scripts/telemetry/run_privacy_dp.py`; يقوم غلاف CI `scripts/telemetry/run_privacy_dp_notebook.sh` بتشغيل الدفتر عبر مسار `.github/workflows/release-pipeline.yml`; テスト حفظ ملخص الحوكمة في `docs/source/status/soranet_privacy_dp_digest.md`.)*

SNNet-8: テレメトリ、スクレーパー、ダッシュボード Prometheusああ。 مواد معايرة الخصوصية التفاضلية في مكانها، ومسار عمل release يحافظ على مخرجات الدفتر حديثة،抑圧。