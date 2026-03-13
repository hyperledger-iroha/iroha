---
lang: ja
direction: ltr
source: docs/portal/docs/sorafs/node-storage.ar.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
ID: ノードストレージ
タイトル: تصميم تخزين عقدة SoraFS
サイドバーラベル: ニュース
説明: معمارية التخزين والحصص وخطافات دورة الحياة لعُقد Torii المستضيفة لبيانات SoraFS。
---

:::note ノート
評価は `docs/source/sorafs/sorafs_node_storage.md` です。スフィンクスは、スフィンクスと呼ばれます。
:::

## تصميم تخزين عقدة SoraFS (مسودة)

Iroha (Torii) を使用してください。
SoraFS وتخصيص جزء من القرص المحلي لتخزين وخدمة القطع.発見
`sorafs_node_client_protocol.md` 器具 SF-1b の評価
ログイン アカウント新規登録 ログイン アカウントを作成する
Torii。 جد التدريبات العملية للمشغلين في
[ランブック عمليات العقدة](./node-operations)。

### ああ

- السماح لأي مُحقق أو عملية Iroha مساعدة بتعريض قرص فائض كمزوّد SoraFS دونああ
  すごいです。
- Norito: マニフェスト وخطط القطع وجذور
  Proof-of-Retrievability（PoR）。
- ログインして、ピンを取得してください。
- إرجاع الصحة/التليمترية (عينات PoR، زمن جلب القطع، ضغط القرص) إلى الحوكمة والعملاء。

### عالية المستوى

```
┌──────────────────────────────────────────────────────────────────────┐
│                         Iroha/Torii Node                             │
│                                                                      │
│  ┌──────────────┐      ┌────────────────────┐                        │
│  │  Torii APIs  │◀────▶│   SoraFS Gateway   │◀───────────────┐       │
│  └──────────────┘      │ (Norito endpoints) │                │       │
│                        └────────┬───────────┘                │       │
│                                 │                            │       │
│                        ┌────────▼────────┐                   │       │
│                        │  Pin Registry   │◀───── manifests   │       │
│                        │ (State / DB)    │                   │       │
│                        └────────┬────────┘                   │       │
│                                 │                            │       │
│                        ┌────────▼────────┐                   │       │
│                        │  Chunk Storage  │◀──── chunk plans  │       │
│                        │  (ChunkStore)   │                   │       │
│                        └────────┬────────┘                   │       │
│                                 │                            │       │
│                        ┌────────▼────────┐                   │       │
│                        │  Disk Quota/IO  │─Pin/serve chunks─▶│ Fetch │
│                        │  Scheduler      │                   │ Clients│
│                        └─────────────────┘                   │       │
│                                                                      │
└──────────────────────────────────────────────────────────────────────┘
```

回答:

- **ゲートウェイ**: Norito HTTP ピン、フェッチ、PoR です。 Norito を確認してください。 HTTP 応答 Torii 応答。
- **ピン レジストリ**: حالة تثبيت マニフェスト المُسجلة في `iroha_data_model::sorafs` و`iroha_core`。マニフェスト マニフェスト ダイジェスト マニフェスト ダイジェスト マニフェスト マニフェスト ダイジェスト マニフェスト マニフェスト マニフェスト マニフェスト マニフェスト マニフェスト マニフェスト ダイジェスト マニフェスト マニフェスト マニフェスト マニフェスト マニフェスト マニフェスト マニフェスト マニフェスト ダイジェスト マニフェスト マニフェスト マニフェスト マニフェスト マニフェスト マニフェスト マニフェスト マニフェスト マニフェスト ダイジェスト マニフェスト マニフェスト マニフェスト マニフェスト マニフェスト マニフェスト マニフェスト マニフェスト マニフェスト ダイジェスト マニフェスト マニフェスト マニフェスト マニフェスト マニフェスト マニフェスト マニフェスト マニフェスト マニフェスト ダイジェスト マニフェスト マニフェスト マニフェスト マニフェスト マニフェスト マニフェスト マニフェスト マニフェスト マニフェストを マニフェスト マニフェスト をダイジェストにします PoR を にします。
- **チャンク ストレージ**: `ChunkStore` マニフェスト موقعة، ويبني خطط القطع باستخدام `ChunkProfile::DEFAULT`، ويخزن القطع في تخطيط حتمي.評価は、PoR وصفية كي يمكن إعادة التحقق دون قراءة الملفそうです。
- **クォータ/スケジューラ**: يفرض حدود المشغل (أقصى بايتات قرص، أقصى pins معلقة، أقصى عمليات fetch متوازية، TTL للقطع) وينسق IO حتى لا تتأثر مهام دفتر الأستاذ。スケジューラ、PoR 、および CPU の機能。

### ああ

`iroha_config`:

```toml
[sorafs.storage]
enabled = false
data_dir = "/var/lib/iroha/sorafs"
max_capacity_bytes = "100 GiB"
max_parallel_fetches = 32
max_pins = 10_000
por_sample_interval_secs = 600
alias = "tenant.alpha"            # وسم اختياري مقروء
adverts:
  stake_pointer = "stake.pool.v1:0x1234"
  availability = "hot"
  max_latency_ms = 500
  topics = ["sorafs.sf1.primary:global"]
```

- `enabled`: مفتاح مشاركة。 503 回の発見。
- `data_dir`: PoR をフェッチします。 `<iroha.data_dir>/sorafs`。
- `max_capacity_bytes`: 認証済み。ピンをピンに固定してください。
- `max_parallel_fetches`: スケジューラと IO の組み合わせ。
- `max_pins`: マニフェストをピンで留めます。
- `por_sample_interval_secs`: 問題は PoR です。 كل مهمة تأخذ `N` ورقة (قابلة للضبط لكل マニフェスト) وتصدر أحداث تليمترية。 `N` メタデータ `profile.sample_multiplier` (`1-4`)。 يمكن أن تكون القيمة رقما/نصا واحدا أو كائنا مع は、لكل ملف تعريف، مثل `{"default":2,"sorafs.sf2@1.0.0":3}` をオーバーライドします。
- `adverts`: 広告 `ProviderAdvertV1` (ステーク ポインター、QoS トピック)。最高のパフォーマンスを見せてください。

評価:

- `[sorafs.storage]` معرف في `iroha_config` كـ `SorafsStorage` ويتم تحميله من ملف إعداد العقدة.
- 評価 `iroha_core` و`iroha_torii` 開発者は、ビルダーを作成する必要があります。ああ。
- توجد は、للتطوير/الاختبار (`SORAFS_STORAGE_*`、`SORAFS_STORAGE_PIN_*`) をオーバーライドします。ああ。

### CLI を使用する

Torii HTTP قيد التوصيل، يشحن クレート `sorafs_node` واجهة CLI خفيفة حتى يتمكن [crates/sorafs_node/src/bin/sorafs-node.rs:1]

```bash
cargo run -p sorafs_node --bin sorafs-node ingest \
  --data-dir ./storage/sorafs \
  --manifest ./fixtures/manifest.to \
  --payload ./fixtures/payload.bin \
  --plan-json-out ./plan.json
```

- `ingest` はマニフェスト `.to` は Norito はペイロードを示します。 عيد بناء خطة القطع من ملف تعريف chunking للـマニフェスト، ويفرض تطابق Digest، ويحفظ ملفات القطع، ويصدرر認証済み JSON 認証 `chunk_fetch_specs` 認証済みの認証を取得します。
- `export` يقبل معرف マニフェスト ويكتب マニフェスト/ペイロード المخزن إلى القرص (مع خطة JSON اختيارية) لضمان قابلية إعادة試合の試合。Norito JSON の stdout スクリプト。 CLI のマニフェストとペイロードの往復の負荷の管理Torii.【crates/sorafs_node/tests/cli.rs:1】

> HTTP
>
> 評価 Torii 評価 `NodeHandle`:
>
> - `GET /v2/sorafs/storage/manifest/{manifest_id_hex}` — يعيد マニフェスト Norito المخزن (base64) ダイジェスト/メタデータ。【crates/iroha_torii/src/sorafs/api.rs:1207】
> - `GET /v2/sorafs/storage/plan/{manifest_id_hex}` — يعيد خطة القطع الحتمية JSON (`chunk_fetch_specs`) لأدوات ダウンストリーム。【crates/iroha_torii/src/sorafs/api.rs:1259】
>
> スクリプト スクリプト スクリプト HTTP スクリプト スクリプト スクリプト スクリプト スクリプト スクリプト スクリプト HTTP スクリプト【crates/iroha_torii/src/sorafs/api.rs:1207】【crates/iroha_torii/src/sorafs/api.rs:1259】

### دورة حياة العقدة

1.***:
   - ログインしてください。 - ログインしてください。 PoR はマニフェストをマニフェストします。
   - テスト SoraFS (Norito JSON POST/GET لـ pin وfetch وأخذ عينات PoR والتليمترية)。
   - 最高のPoRを提供します。
2. **発見/広告**:
   - توليد مستندات `ProviderAdvertV1` باستخدام السعة/الصحة الحالية، وتوقيعها بالمفتاح المعتمد من発見。 `profile_aliases` を確認してください。
3. **ピン**:
   - マニフェスト موقعا (يشمل خطة القطع وجذر PoR وتواقيع المجلس)。エイリアス (`sorafs.sf1@1.0.0` مطلوب) を作成し、マニフェストを作成します。
   - 最高です。認証済み (Norito منظم)。
   - は、`ChunkStore` をダイジェストで表示します。 PoR メタデータとマニフェスト レジストリ。
4. **フェッチ**:
   - 最高のパフォーマンス。スケジューラー `max_parallel_fetches` 、 `429` 、
   - テスト (Norito JSON) の評価 - 評価 - 評価 (Norito JSON) 評価と評価ありがとうございます。
5. **評価PoR**:
   - يختار العامل マニフェスト بنسبة للوزن (مثل البايتات المخزنة) ويجري أخذ عينات حتميا باستخدام شجرة PoRありがとうございます。
   - 広告を表示する 広告を表示するああ。
6. **//////////////////////**:
   - ピンをピンに留めます。 يمكن للمشغلين تكوين سياسات إخلاء (مثل TTL وLRU) عند توافق نموذج الحوكمة؛ピンを解除してください。

### تكامل إعلان السعة والجدولة

- تعيد Torii تحديثات `CapacityDeclarationRecord` من `/v2/sorafs/capacity/declare` إلى `CapacityManager` المضمن، بحيث يبني كلチャンカー/レーンを表示します。 يكشف المدير لقطات read-only للتليمترية (`GET /v2/sorafs/capacity/state`) ويفرض حجوزات لكل ملف تعريف أو レーン قبل قبول أوامر جديدة.【crates/sorafs_node/src/capacity.rs:1】【crates/sorafs_node/src/lib.rs:60】
- エンドポイント `/v2/sorafs/capacity/schedule` と `ReplicationOrderV1` が必要です。チャンカー/レーンの管理管理者は、`ReplicationPlan` を使用して、オーケストレーションを実行します。 بالأوامر الخاصة بمزوّدين آخرين باستجابة `ignored` لتسهيل سير العمل متعدد المشغلين.【crates/iroha_torii/src/routing.rs:4845】
- フック الإكمال (مثل ما يحدث بعد نجاح الإدخال) باستدعاء `POST /v2/sorafs/capacity/complete` لإطلاق الحجوزات عبر `CapacityManager::complete_order`。 `ReplicationRelease` (チャンカー/レーン) のオーケストレーションを行う投票。 سيُوصل هذا بخط أنابيب مخزن القطع عند اكتمال منطق 【crates/iroha_torii/src/routing.rs:4885】【crates/sorafs_node/src/capacity.rs:90】
- 評価 `TelemetryAccumulator` 評価 `NodeHandle::update_telemetry` 評価 評価 - PoR/稼働時間 وفي `CapacityTelemetryV1` 内部構造スケジューラ.【crates/sorafs_node/src/lib.rs:142】【crates/sorafs_node/src/telemetry.rs:1】

### عرض المزيد- **الحوكمة**: توسيع `sorafs_pin_registry_tracker.md` بتليمترية التخزين (معدل نجاح PoR، استغلال القرص)。広告を掲載しています。
- **SDK の概要**: ブートストラップ ブートストラップ (別名) (エイリアス)そうです。
- **التليمترية**: دمج مع مكدس المقاييس الحالي (Prometheus / OpenTelemetry) حتى تظهر مقاييس التخزين في لوحاتああ。
- ** セキュリティ**: セキュリティ、非同期、バックプレッシャー、サンドボックス化io_uring は、tokio の المحدودة لمنع العملاء الخبيثين من استنزاف الموارد。

يحافظ هذا التصميم على اختيارية وحدة التخزين وحتميتها، ويمنح المشغلين المفاتيح اللازمة SoraFS を参照してください。 `iroha_config` و`iroha_core` و`iroha_torii` وبوابة Norito، بالإضافة और देखें