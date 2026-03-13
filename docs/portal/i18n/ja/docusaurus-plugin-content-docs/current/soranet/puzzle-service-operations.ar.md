---
lang: ja
direction: ltr
source: docs/portal/docs/soranet/puzzle-service-operations.ar.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id:パズルサービス運営
タイトル: دليل عمليات خدمة الالغاز
サイドバーラベル: ニュース ニュース
説明: デーモン `soranet-puzzle-service` ミッション Argon2/ML-DSA。
---

:::メモ
`docs/source/soranet/puzzle_service_operations.md`。 حافظ على النسختين متزامنتين حتى يتم تقاعد الوثائق القديمة.
:::

# دليل عمليات خدمة الالغاز

デーモン `soranet-puzzle-service` (`tools/soranet-puzzle-service/`) デーモン
入場券 Argon2 والتي تعكس ポリシー リレー `pow.puzzle.*`
ML-DSA アドミッション トークンとエッジ リレー。
HTTP:

- `GET /healthz` - 活気。
- `GET /v2/puzzle/config` - يعيد معلمات PoW/パズル الفعلية المسحوبة
  JSON リレー (`handshake.descriptor_commit_hex`、`pow.*`)。
- `POST /v2/puzzle/mint` - アルゴン 2;本文 JSON
  `{ "ttl_secs": <u64>, "transcript_hash_hex": "<32-byte hex>", "signed": true }`
  يطلب TTL اقصر (يتم ضبطه ضمن نافذة ポリシー) ، ويربط التذكرة بـ トランスクリプト ハッシュ
  ويعيد تذكرة موقعة من リレー + بصمة التوقيع عندما تكون مفاتيح التوقيع مهيئة。
- `GET /v2/token/config` - عندما `pow.token.enabled = true` يعيد سياسة
  アドミッション トークン (発行者の指紋、TTL/クロック スキュー、リレー ID、
  ومجموعة 取り消し المدمجة）。
- `POST /v2/token/mint` - ML-DSA アドミッション トークン 再開ハッシュ
  本文は `{ "transcript_hash_hex": "...", "ttl_secs": <u64>, "flags": <u8> }` です。

تتم عملية التحقق من التذاكر المنتجة عبر خدمة الاختبار التكاملية
`volumetric_dos_soak_preserves_puzzle_and_latency_slo` ، والتي تختبر ايضا
スロットル リレー セキュリティ DoS セキュリティ [tools/soranet-relay/tests/adaptive_and_puzzle.rs:337]

## いいえ

JSON リレー `pow.token.*` (国際)
`tools/soranet-relay/deploy/config/relay.entry.json` كمثال) ML-DSA。
発行者による失効の確認:

```json
"pow": {
  "token": {
    "enabled": true,
    "issuer_public_key_hex": "<ML-DSA-44 public key>",
    "revocation_list_hex": [],
    "revocation_list_path": "/etc/soranet/relay/token_revocations.json"
  }
}
```

خدمة الالغاز تعيد استخدام هذه القيم وتقوم باعادة تحميل ملف 取消し بصيغة
Norito JSON ランタイム。 CLI `soranet-admission-token`
(`cargo run -p soranet-relay --bin soranet_admission_token`) ログインしてください
オフラインです `token_id_hex` 失効です
ニュースは最新情報を更新します。

発行者とフラグ CLI:

```bash
cargo run -p soranet-puzzle-service -- \
  --relay-config /etc/soranet/relay/relay.entry.json \
  --token-secret-path /etc/soranet/relay/token_issuer_secret.hex \
  --token-revocation-file /etc/soranet/relay/token_revocations.json \
  --token-revocation-refresh-secs 60
```

`--token-secret-hex` のパイプライン。イエス
ウォッチャー لملف 取り消し بالحفاظ على `/v2/token/config` محدثا؛重要な問題
`soranet-admission-token revoke` は取り消しです。

اضبط `pow.signed_ticket_public_key_hex` في JSON الخاص بالـ リレー للاعلان عن المفتاح
ML-DSA-44 は PoW チケットを取得します。 `/v2/puzzle/config`
BLAKE3 (`signed_ticket_public_key_fingerprint_hex`) クライアント
من تثبيت 検証者。リレー ID とトランスクリプト バインディング
取り消しPoW チケット 74 個のチケットを入手
署名付きチケット検証者。秘密の秘密 `--signed-ticket-secret-hex`
او `--signed-ticket-secret-path` عند تشغيل خدمة الالغاز;キーペア
`pow.signed_ticket_public_key_hex` を参照してください。
`POST /v2/puzzle/mint` يقبل `"signed": true` (واختياريا `"transcript_hash_hex"`)
署名済みチケット Norito バイト数。最高のパフォーマンス
`signed_ticket_b64` و`signed_ticket_fingerprint_hex` は、指紋を再生します。ユイシュス
`signed = true` 秘密の秘密。

## دليل تدوير المفاتيح

1. ** 記述子コミット الجديد.** ガバナンスリレー記述子コミット في
   ディレクトリバンドル。 16 進数 `handshake.descriptor_commit_hex` 16 進数
   JSON リレー メッセージを送信します。
2. **مراجعة حدود سياسة الالغاز.** تاكد ان قيم
   `pow.puzzle.{memory_kib,time_cost,lanes}` 認証済み。やあ
   على المشغلين ابقاء تهيئة Argon2 حتمية عبر リレー (حد ادنى 4 MiB ذاكرة،
   1 <= レーン <= 16)。
3. **システム管理、ガバナンス管理。
   カットオーバーのバージョン。ホットリロードを実行するऔर देखें
   記述子はコミットを実行します。
4. ** التحقق.** اصدر تذكرة عبر `POST /v2/puzzle/mint` وتاكد ان `difficulty` و`expires_at`
   طابقان السياسة الجديدة。ソーク (`docs/source/soranet/reports/pow_resilience.md`)
   حدود الكمون المتوقعة للمرجعية。 عندما تكون التوكنات مفعلة، اجلب
   `/v2/token/config` 発行者の指紋の取り消し
   طابقان القيم المتوقعة。

## いいえ、いいえ。

1. اضبط `pow.puzzle.enabled = false` في تكوين リレー المشترك. `pow.required = true`
   ハッシュキャッシュフォールバックを実現します。
2. 説明 `pow.emergency` 記述子 Argon2
   オフライン。
3. ニュースを中継します。
4. راقب `soranet_handshake_pow_difficulty` لضمان ان الصعوبة تنخفض الى قيمة hashcash
   `/v2/puzzle/config` يبلغ `puzzle = null` を確認してください。

## और देखें- **レイテンシ SLO:** `soranet_handshake_latency_seconds` وابق P95 اقل من 300 ミリ秒。
  オフセットは、スロットルとガードをソークします。
  【docs/source/soranet/reports/pow_resilience.md:1】
- **クォータ プレッシャー:** `soranet_guard_capacity_report.py` リレー メトリクス
  クールダウン `pow.quotas` (`soranet_abuse_remote_cooldowns`、
  `soranet_handshake_throttled_remote_quota_total`).【docs/source/soranet/relay_audit_pipeline.md:68】
- **パズルの配置:** يجب ان يطابق `soranet_handshake_pow_difficulty` الصعوبة
  `/v2/puzzle/config` です。 يشير الاختلاف الى تكوين リレー قديم او فشل اعادة
  ああ。
- **トークンの準備状況:** به اذا انخفض `/v2/token/config` الى `enabled = false`
  `revocation_source` のタイムスタンプ。やあ
  失効 Norito CLI 認証 سحب توكن للحفاظ على
  エンドポイント。
- **サービスの健全性:** افحص `/healthz` وفق cadence liveness المعتادة ونبه اذا
  `/v2/puzzle/mint` HTTP 500 (يدل على عدم تطابق معلمات Argon2 او
  RNG)。トークンの鋳造とHTTP 4xx/5xx `/v2/token/mint`
  ページングを確認してください。

## और देखें

リレーは、`handshake` スロットル クールダウンをリレーします。
パイプライン `docs/source/soranet/relay_audit_pipeline.md`
テストを行ってください。認証済み
スクリーンショット تكوين Norito مع
ロールアウトが完了しました。入場トークンを取得する
`token_id_hex` وادراجها في ملف 取消 عند انتهائها او
ああ。