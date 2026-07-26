---
lang: ja
direction: ltr
source: docs/portal/docs/sorafs/orchestrator-config.ar.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
ID: オーケストレーター構成
タイトル: اعداد مُنسِّق SoraFS
サイドバーラベル: عداد المُنسِّق
説明: ضبط مُنسِّق الجلب متعدد المصادر، وفسّر الاخفاقات، وتتبع مخرجات التليمترية。
---

:::note ノート
テストは `docs/source/sorafs/developer/orchestrator.md` です。 حرص على إبقاء النسختين متزامنتين إلى أن يتم سحب مجموعة التوثيق القديمة。
:::

# دليل مُنسِّق الجلب متعدد المصادر

يقود مُنسِّق الجلب متعدد المصادر في SoraFS تنزيلات حتمية ومتوازية من مجموعة
広告が表示されます。 شرح هذا الدليل كيفية ضبط
المُنسِّق، وما هي إشارات الفشل المتوقعة أثناء عمليات الإطلاق، وأي تدفقات
重要な問題は、次のとおりです。

## 1. 名前を付けてください。

セキュリティ:

|翻訳 |ああ | और देखें
|----------|----------|----------|
| `OrchestratorConfig.scoreboard` | يطبّع أوزان المزوّدين، ويتحقق من حداثة التليمترية، ويحفظ لوحة النتائج JSON المستخدمةすごい。 | `crates/sorafs_car::scoreboard::ScoreboardConfig`。 |
| `OrchestratorConfig.fetch` | يطبّق حدود وقت التشغيل (ميزانيات إعادة المحاولة، حدود التوازي، مفاتيح التحقق)。 | ُطابق `FetchOptions` في `crates/sorafs_car::multi_fetch`。 |
| CLI / SDK の説明 |拒否/ブーストを実行します。 | `sorafs_cli fetch` يعرّض هذه الأعلام مباشرةً؛ `OrchestratorConfig` は SDK です。 |

JSON 文字 `crates/sorafs_orchestrator::bindings` 文字列
Norito JSON は、SDK をサポートしています。

### 1.1 JSON の説明

```json
{
  "scoreboard": {
    "latency_cap_ms": 6000,
    "weight_scale": 12000,
    "telemetry_grace_secs": 900,
    "persist_path": "/var/lib/sorafs/scoreboards/latest.json"
  },
  "fetch": {
    "verify_lengths": true,
    "verify_digests": true,
    "retry_budget": 4,
    "provider_failure_threshold": 3,
    "global_parallel_limit": 8
  },
  "telemetry_region": "iad-prod",
  "max_providers": 6,
  "transport_policy": "soranet_first"
}
```

`iroha_config` المعتادة (`defaults/`、ユーザー、実際)
最高のパフォーマンスを見せてください。フォールバック機能
SNNet-5a のロールアウトの確認
`docs/examples/sorafs_direct_mode_policy.json` ログイン
`docs/source/sorafs/direct_mode_pack.md`。

### 1.2 の説明

SNNet-9 を参照してください。 كائن `compliance` جديد
直接のみ:

```json
"compliance": {
  "operator_jurisdictions": ["US", "JP"],
  "jurisdiction_opt_outs": ["US"],
  "blinded_cid_opt_outs": [
    "C6B434E5F23ABD318F01FEDB834B34BD16B46E0CC44CD70536233A632DFA3828"
  ],
  "audit_contacts": ["mailto:compliance@example.org"]
}
```

- `operator_jurisdictions` يعلن رموز ISO‑3166 alpha‑2 التي تعمل فيها هذه
  और देखें最高のパフォーマンスを見せてください。
- `jurisdiction_opt_outs` يعكس سجل الحوكمة。 عندما تظهر أي ولاية تشغيلية ضمن
  القائمة، يفرض المُنسِّق `transport_policy=direct-only` ويُصدر سبب التراجع
  `compliance_jurisdiction_opt_out`。
- `blinded_cid_opt_outs` يسرد ダイジェスト المانيفست (ブラインド CID 16 進数)
  بأحرف كبيرة）。直接のみの対応です。
  `compliance_blinded_cid_opt_out` في التليمترية。
- `audit_contacts` يسجل عناوين URI التي تتوقع الحوكمة أن ينشرها المشغلون في
  プレイブックは GAR です。
- `attestations` يلتقط حزم الامتثال الموقّعة الداعمة للسياسة. كل إدخال يعرّف
  `jurisdiction` 認証 (ISO-3166 alpha-2) 、`document_uri` 、`digest_hex`
  64 件のコメント `issued_at_ms`، و`expires_at_ms`
  ああ。 تدفق هذه الآثار إلى قائمة تدقيق المُنسِّق كي تربط أدوات الحوكمة
  最高です。

مرّر كتلة الامتثال عبر طبقات المعداد المعتادة كي يحصل المشغلون على تجاوزات
やあ。書き込みモード: SDK の書き込みモード:
`upload-pq-only` 直接のみ
سريعاً عندما لا يوجد مزوّدون متوافقون。オプトアウト オプション
`governance/compliance/soranet_opt_outs.json` وينشر مجلس الحوكمة التحديثات عبر
ありがとうございます。 يتوفر مثال كامل للاعداد (بما في ذلك 認証) في
`docs/examples/sorafs_compliance_policy.json`، كما يوثق المسار التشغيلي في
[プレイブック امتثال GAR](../../../source/soranet/gar_compliance_playbook.md)。

### 1.3 CLI と SDK

| और देखें認証済み |
|--------------|------|
| `--max-peers` / `OrchestratorConfig::with_max_providers` | حد عدد المزوّدين الذين يمرون من فلتر لوحة النتائج。 `None` を確認してください。 |
| `--retry-budget` / `FetchOptions::per_chunk_retry_limit` | حد إعادة المحاولة لكل شريحة。 `MultiSourceError::ExhaustedRetries` です。 |
| `--telemetry-json` |レイテンシ/障害が発生する可能性があります。 `telemetry_grace_secs` を確認してください。 |
| `--scoreboard-out` | يحفظ لوحة النتائج المحسوبة (مزوّدون مؤهلون + غير مؤهلين) للفحص بعد التشغيل。 |
| `--scoreboard-now` |フィクスチャを使用して、Unix (UNIX) のフィクスチャを作成します。 |
| `--deny-provider` / フック スコア |広告を表示します。重要です。 |
| `--boost-provider=name:delta` |ラウンドロビン戦、決勝トーナメント、ラウンドロビン戦。 |
| `--telemetry-region` / `OrchestratorConfig::with_telemetry_region` | يوسم المقاييس والسجلات المهيكلة لتمكين لوحات المتابعة من التجميع حسب الجغرافيا أو موجةああ。 |
| `--transport-policy` / `OrchestratorConfig::with_transport_policy` | `soranet-first` を確認してください。 `direct-only` テスト `direct-only` テスト `soranet-strict` PQ のみ最高です。 |

SoraNet ファースト هو الافتراضي الحالي للشحن، ويجب أن تذكر التراجعات المانع SNNet
ああ。 SNNet-4/5/5a/5b/6a/7/8/12/13 を参照してください。
(حو `soranet-strict`) ، وحتى ذلك الحين يجب أن تقتصر تجاوزات `direct-only` على
あなたのことを忘れないでください。

كل الأعلام أعلاه تقبل صيغة `--` في كل من `sorafs_cli fetch` والثنائي
`sorafs_fetch`。 SDK ビルダーの開発。

### 1.4 キャッシュキャッシュ

CLI 接続、SoraNet 接続、リレー接続、SoraNet 接続、リレー接続
SNNet-5 のロールアウトを確認します。 ثلاثة أعلام جديدة تتحكم
意味:

|ああ |ああ |
|------|------|
| `--guard-directory <PATH>` | يشير إلى ملف JSON يصف أحدث توافق للـ リレー (جزء منه أدناه)。キャッシュを保存してください。 |
| `--guard-cache <PATH>` | يحفظ `GuardSet` المشفر بنوريتو。キャッシュをキャッシュします。 |
| `--guard-target <COUNT>` / `--guard-retention-days <DAYS>` |は、 العدد حراس الدخول المثبتين (الافتراضي 3) وفترة الاحتفاظ (الافتراضي 30 يوماً)。 |
| `--guard-cache-key <HEX>` | 32 個のキャッシュ MAC Blake3 個のキャッシュ 個の個数 個のキャッシュ 個の個数 個の個数 個の個数ああ。 |

回答:

`--guard-directory` は `GuardDirectorySnapshotV2` を評価します。
回答:- `version` — パスワード (`2`)。
- `directory_hash`、`published_at_unix`、`valid_after_unix`、`valid_until_unix` —
  最高のパフォーマンスを見せてください。
- `validation_phase` — ニュース ニュース (`1` = ニュース ニュース Ed25519 واحد،
  `2` = تفضيل توقيعات مزدوجة، `3` = اشتراط التوقيعات المزدوجة)。
- `issuers` - `fingerprint`、`ed25519_public`、
  `mldsa65_public`。回答:
  `BLAKE3("soranet.src.v2.issuer" || ed25519 || u32(len(ml-dsa)) || ml-dsa)`。
- `relays` — SRCv2 (`RelayCertificateBundleV2::to_cbor()`)。説明
  リレー リレー ML-KEM Ed25519/ML-DSA-65
  ああ。

CLI のセキュリティ キャッシュ
ああ。 JSON を使用する必要があります。 SRCv2 です。

CLI インターフェイス `--guard-directory` インターフェイス インターフェイス、キャッシュ インターフェイス。
يحافظ المحدد على الحراس المثبتين ضمن نافذة الاحتفاظ والمؤهلين في الدليل؛
は、中継を中継します。キャッシュを保存する
`--guard-cache` を参照してください。
SDK の開発と開発
`GuardSelector::select(&RelayDirectory, existing_guard_set, now_unix_secs)`
`GuardSet` は `SorafsGatewayFetchOptions` です。

`ml_kem_public_hex` يمكّن المحدد من تفضيل الحراس القادرين على PQ أثناء rollout
SNNet-5。 (`anon-guard-pq`, `anon-majority-pq`,
`anon-strict-pq`) リレー الكلاسيكية تلقائياً: عندما يتوفر حارس
PQ は、世界の主要都市の 1 つです。
CLI/SDK と `anonymity_status`/`anonymity_reason`、
`anonymity_effective_policy`、`anonymity_pq_selected`、`anonymity_classical_selected`、
`anonymity_pq_ratio`、`anonymity_classical_ratio` وحقول المرشحين/العجز/فارق
ブラウンアウトとフォールバックを回避します。

SRCv2 は `certificate_base64` です。うーん
حزمة ويعيد التحقق من تواقيع Ed25519/ML-DSA ويحتفظ بالشهادة
キャッシュを保存します。 عند وجود شهادة تصبح المصدر المعتمد لمفاتيح PQ
فضيلات المصافحة والترجيح؛ وتُهمل الشهادات المنتهية ويعود المحدد إلى حقول
そうです。ログイン して翻訳を追加する
`telemetry::sorafs.guard` و`telemetry::sorafs.circuit` التي تسجل نافذة الصلاحية
ログインしてください。

CLI でのアクセス:

```bash
sorafs_cli guard-directory fetch \
  --url https://directory.soranet.dev/mainnet_snapshot.norito \
  --output ./state/guard_directory.norito \
  --expected-directory-hash <directory-hash-hex>

sorafs_cli guard-directory verify \
  --path ./state/guard_directory.norito \
  --expected-directory-hash <directory-hash-hex>
```

ينزّل `fetch` اللقطة SRCv2 ويتحقق منها قبل الكتابة على القرص، بينما يعيد
`verify` تشغيل خط التحقق لآثار مصدرها فرق أخرى، ويصدر ملخص JSON يعكس مخرجات
CLI/SDK を使用します。

### 1.5 の意味

リレー キャッシュ セキュリティ セキュリティ セキュリティ セキュリティ セキュリティ
ソラネットは、ソラネットをサポートしています。 يقع الاعداد ضمن
`OrchestratorConfig` (`crates/sorafs_orchestrator/src/lib.rs:305`) セキュリティ:

- `relay_directory`: SNNet-3 中間/出口ホップ数
  やあ。
- `circuit_manager`: اعداد اختياري (ممكّن افتراضياً) يتحكم في TTL للدوائر。

Norito JSON 番号 `circuit_manager`:

```json
"circuit_manager": {
  "enabled": true,
  "circuit_ttl_secs": 900
}
```

SDK の概要
`SorafsGatewayFetchOptions::relay_directory`
(`crates/iroha/src/client.rs:320`) CLI を使用してください。
`--guard-directory` (`crates/iroha_cli/src/commands/sorafs.rs:365`)。يجدد المدير الدوائر كلما تغيرت بيانات الحارس (エンドポイント أو مفتاح PQ أو الطابع
(英語) TTL。 يقوم المساعد `refresh_circuits` المستدعى
قبل كل جلب (`crates/sorafs_orchestrator/src/lib.rs:1346`) بإصدار سجلات
`CircuitEvent` は、今日のことを意味します。浸す
`circuit_manager_latency_soak_remains_stable_across_rotations`
(`crates/sorafs_orchestrator/src/soranet.rs:1479`) يثبت استقرار الكمون عبر ثلاث
ログイン して翻訳を追加するऔर देखें
`docs/source/soranet/reports/circuit_stability.md:1`。

### 1.6 クイックスピード

يمكن للمُنسِّق اختيارياً تشغيل وكيل QUIC محلي بحيث لا تضطر إضافات المتصفح
SDK のキャッシュ。ログインしてください
ループバック هينهي اتصالات QUIC، ويعيد マニフェスト من Norito يصف الشهادة ومفتاح
キャッシュを保存してください。 تُعد أحداث النقل التي يصدرها الوكيل ضمن
`sorafs_orchestrator_transport_events_total`。

`local_proxy` 認証 JSON 認証:

```json
"local_proxy": {
  "bind_addr": "127.0.0.1:9443",
  "telemetry_label": "dev-proxy",
  "guard_cache_key_hex": "0123456789ABCDEF0123456789ABCDEF0123456789ABCDEF0123456789ABCDEF",
  "emit_browser_manifest": true,
  "proxy_mode": "bridge",
  "prewarm_circuits": true,
  "max_streams_per_circuit": 64,
  "circuit_ttl_hint_secs": 300,
  "norito_bridge": {
    "spool_dir": "./storage/streaming/soranet_routes",
    "extension": "norito"
  },
  "kaigi_bridge": {
    "spool_dir": "./storage/streaming/soranet_routes",
    "extension": "norito",
    "room_policy": "public"
  }
}
```

- `bind_addr` يتحكم بمكان استماع الوكيل (استخدم المنفذ `0` لطلب منفذ مؤقت)。
- `telemetry_label` は、今日のニュースです。
- `guard_cache_key_hex` (اختياري) يسمح للوكيل بعرض キャッシュ الحراس بالمفتاح ذاته
  CLI/SDK を確認するには、次の操作を行ってください。
- `emit_browser_manifest` يبدّل ما إذا كان المصافحة تعيد マニフェスト يمكن
  すごいですね。
- `proxy_mode` يحدد ما إذا كان الوكيل يجسر الحركة محلياً (`bridge`) أو يكتفي
  (`metadata-only`) SDK と SoraNet の開発。
  `bridge`; `metadata-only` はマニフェストです。
- `prewarm_circuits`、`max_streams_per_circuit`、`circuit_ttl_hint_secs`
  重要な情報を確認してください。
- `car_bridge` (اختياري) يشير إلى キャッシュ محلي لأرشيفات CAR。 `extension`
  في اللاحقة المضافة عند غياب `*.car`؛ `allow_zst = true` ログイン
  `*.car.zst` です。
- `kaigi_bridge` (اختياري) يعرّض مسارات Kaigi للوكيل。 يعلن `room_policy` إذا
  كان الجسر يعمل بوضع `public` أو `authenticated` حتى يختار المتصفح ملصقات GAR
  そうです。
- `sorafs_cli fetch` يوفّر `--local-proxy-mode=bridge|metadata-only` و
  `--local-proxy-norito-spool=PATH` スプールのスプール
  JSON を使用します。
- `downgrade_remediation` (اختياري) يضبط خطاف خفض المستوى التلقائي。認証済み
  評価を下げるリレー 評価をダウングレードする
  `threshold` 認証 `window_secs` 認証 `target_mode`
  (الافتراضي `metadata-only`)。 وبعد توقف ダウングレード يعود الوكيل إلى `resume_mode`
  `cooldown_secs`。 ستخدم مصفوفة `modes` لقصر التفعيل على أدوار リレー معينة
  (افتراضياً リレー الدخول)。

橋の架け橋:

- **`norito`** — يتم حل هدف البث الخاص بالعميل نسبةً إلى
  `norito_bridge.spool_dir`。 تتم تنقية الأهداف (لا traversal ولا مسارات مطلقة)،
  ログインしてください。
- **`car`** — 評価 `car_bridge.cache_dir` 評価
  `allow_zst` を参照してください。ああ
  الناجحة ترد بـ `STREAM_ACK_OK` قبل نقل الأرشيف كي يتمكن العملاء من تنفيذ
  ありがとう。HMAC キャッシュ タグ (キャッシュ タグ)
المصافحة) ويسجل رموز سبب التليمترية `norito_*` / `car_*` حتى تميز لحات
ログインしてください。

`Orchestrator::local_proxy().await` يعرّض المقبض التشغيلي حتى يتمكن المستدعون
PEM マニフェスト マニフェスト マニフェスト マニフェスト マニフェスト マニフェスト マニフェスト マニフェスト マニフェスト マニフェスト マニフェスト マニフェスト マニフェスト マニフェスト マニフェスト マニフェスト マニフェスト マニフェスト マニフェスト マニフェスト マニフェスト
ああ。

**マニフェスト v2**。 إضافة إلى الشهادة ومفتاح
キャッシュ バージョン v2 バージョン:

- `alpn` (`"sorafs-proxy/1"`) は、`capabilities` を表示します。
- `session_id` 親和性 `cache_tagging` 親和性 親和性
  HMAC は、セキュリティを強化します。
- テスト (`circuit`、`guard_selection`、`route_hints`)
  واجهة أغنى قبل فتح البث。
- `telemetry_v2` ログインしてください。
- `STREAM_ACK_OK` と `cache_tag_hex`。 عكس العميل القيمة في ترويسة
  `x-sorafs-cache-tag` HTTP メッセージ TCP メッセージ
  重要です。

هذه الحقول متوافقة للخلف — يمكن للعملاء الأقدم تجاهل المفاتيح الجديدة
والاعتماد على مجموعة v1。

## 2. いいえ

حققاً صارماً من القدرات والميزانيات قبل نقل أي بايت.ああ
回答:

1. **إخفاقات الأهلية (قبل التنفيذ).** المزوّدون الذين يفتقرون لقدرة النطاق، أو
   広告 منتهية، أو تليمترية قديمة يتم تسجيلهم في لوحة النتائج ويُستبعدون من
   ああ。 CLI مصفوفة `ineligible_providers` بالأسباب كي يتمكن
   重要な問題は、次のとおりです。
2. ** الاستنزاف أثناء التشغيل.** يتتبع كل مزوّد الإخفاقات المتتالية.認証済み
   `provider_failure_threshold` يتم وسم المزوّد `disabled` لبقية الجلسة。やあ
   انتقل جميع المزوّدين إلى `disabled` يعيد المُنسِّق
   `MultiSourceError::NoHealthyProviders { last_error, chunk_index }`。
3. ** إجهاضات حتمية.** 評価:
   - `MultiSourceError::NoCompatibleProviders` — يتطلب المانيفست مدى شرائح أو
     حاذاة لا يمكن للمزوّدين الباقين تلبيتها.
   - `MultiSourceError::ExhaustedRetries` — تم استهلاك ميزانية إعادة المحاولة لكل
     やあ。
   - `MultiSourceError::ObserverFailed` — رفض المراقبون ダウンストリーム (خطاطيف البث)
     ありがとうございます。

最高のパフォーマンスを見せてください。評価
مع هذه الأخطاء كمعوّقات إصدار — إعادة المحاولة بذات المدخلات ستعيد الفشل حتى
広告を掲載しています。

### 2.1 を実行します。

عند ضبط `persist_path` يكتب المُنسِّق لوحة النتائج النهائية بعد كل تشغيل。ヤシ
JSON の意味:

- `eligibility` (`eligible` × `ineligible::<reason>`)。
- `weight` (الوزن المطبع المعين لهذا التشغيل)。
- بيانات `provider` الوصفية (エンドポイント ميزانية التوازي)。

ロールアウトを行うには、ロールアウトを実行する必要があります。
قابلة للتدقيق。

## 3. いいえ。

### 3.1 セキュリティ Prometheus

`iroha_telemetry`:| और देखेंラベル |ああ |
|----------|----------|----------|
| `sorafs_orchestrator_active_fetches` | `manifest_id`、`region` |重要です。 |
| `sorafs_orchestrator_fetch_duration_ms` | `manifest_id`、`region` | هيستوغرام يسجل زمن الجلب من طرف إلى طرف. |
| `sorafs_orchestrator_fetch_failures_total` | `manifest_id`、`region`、`reason` | عدّاد للإخفاقات النهائية (استنزاف إعادة المحاولة، عدم وجود مزوّدين، فشل المراقب)。 |
| `sorafs_orchestrator_retries_total` | `manifest_id`、`provider`、`reason` | عدّاد لمحاولات إعادة المحاولة لكل مزوّد。 |
| `sorafs_orchestrator_provider_failures_total` | `manifest_id`、`provider`、`reason` | عدّاد لإخفاقات المزوّد على مستوى الجلسة المؤدية للتعطيل。 |
| `sorafs_orchestrator_policy_events_total` | `region`、`stage`、`outcome`、`reason` |ロールアウト (ロールアウト) を実行してください。 |
| `sorafs_orchestrator_pq_ratio` | `region`、`stage` | PQ リレー、SoraNet 中継。 |
| `sorafs_orchestrator_pq_candidate_ratio` | `region`、`stage` | PQ スコアボードをリレーします。 |
| `sorafs_orchestrator_pq_deficit_ratio` | `region`、`stage` | هيستوغرام لعجز السياسة (الفجوة بين الهدف والحصة الفعلية)。 |
| `sorafs_orchestrator_classical_ratio` | `region`、`stage` | هيستوغرام لحصة リレー في كل جلسة。 |
| `sorafs_orchestrator_classical_selected` | `region`、`stage` | هيستوغرام لعدد リレー كلاسيكية المختارة في كل جلسة。 |

ステージングは​​最高です。 और देखें
SF-6 の可観測性:

1. ** الجلب النشط** — تنبيه إذا ارتفع القياس دون اكتمالات مقابلة.
2. ** سسبة إعادة المحاولة** — تحذير عند تجاوز عدادات `retry` للخطوط الاساسية.
3. **إخفاقات المزوّد** — تشغيل ポケベル عند تجاوز أي مزوّد `session_failure > 0`
   15 月です。

### 3.2 説明

ينشر المُنسِّق أحداثاً مهيكلة إلى اهداف حتمية:

- `telemetry::sorafs.fetch.lifecycle` — علامات `start` و`complete` مع عدد الشرائح
  ログインしてください。
- `telemetry::sorafs.fetch.retry` — 回答 (`provider`、`reason`、
  `attempts`) トリアージ。
- `telemetry::sorafs.fetch.provider_failure` — مزوّدون تم تعطيلهم بسبب تكرار
  ああ。
- `telemetry::sorafs.fetch.error` — خفاقات نهائية مع `reason` وبيانات مزوّد
  ああ。

وجّه هذه التدفقات إلى مسار السجلات Norito الحالي لكي تمتلك الاستجابة للحوادث
そうです。 أحداث دورة الحياة مزيج PQ/الكلاسيكي عبر
`anonymity_effective_policy`、`anonymity_pq_ratio`、`anonymity_classical_ratio`
ログインしてください。ああ
ロールアウト 説明 GA 説明 説明 説明 `info` 説明 説明 حياة/إعادة
`warn` を確認してください。

### 3.3 JSON の説明

`sorafs_cli fetch` と SDK Rust のバージョン:

- `provider_reports` النجاح/الفشل وما إذا تم تعطيل المزوّد。
- `chunk_receipts` を確認してください。
- `retry_stats` と `ineligible_providers`。

أرشِف ملف الملخص عند تتبع مزوّدين سيئين — تتطابق الإيصالات مباشرة مع بيانات
そうです。

## 4. قائمة تشغيل تشغيلية1. **حضّر الاعداد في CI.** شغّل `sorafs_fetch` بالاعداد المستهدف، ومرر
   `--scoreboard-out` は、次のとおりです。 और देखें
   重要な問題は、次のとおりです。
2. ** تحقق من التليمترية.** تأكد من أن النشر يصدر مقاييس `sorafs.fetch.*` وسجلات
   مهيكلة قبل تمكين الجلب متعدد المصادر للمستخدمين. غياب المقاييس يشير عادةً
   すごいです。
3. **وثّق التجاوزات.** عند تطبيق إعدادات طارئة `--deny-provider` أو
   `--boost-provider` は JSON (CLI) です。 جب أن تعكس
   スコアボードのスコアボード。
4. **煙が出る。** 煙が出る。** 煙が出る。** 煙が出る。** 煙が出る。
   フィクスチャ (`fixtures/sorafs_manifest/ci_sample/`) を取得します。
   重要な問題は、次のとおりです。

ロールアウト ロールアウト ロールアウト ロールアウト ロールアウト ロールアウト ロールアウト ロールアウト ロールアウト ロールアウト ロールアウト ロールアウト
ありがとうございます。

### 4.1 の評価

يمكن للمشغلين تثبيت مرحلة النقل/الاخفاء النشطة دون تعديل الاعداد الاساسي عبر
ضبط `policy_override.transport_policy` و`policy_override.anonymity_policy` في
JSON は `orchestrator` (`--transport-policy-override=` /
`--anonymity-policy-override=` と `sorafs_cli fetch`)。オーバーライド
フォールバック ブラウンアウト: フォールバック ブラウンアウト: 最高の PQ を実現します。
`no providers` を確認してください。翻訳: 翻訳: 翻訳: 翻訳: 翻訳
オーバーライドします。