---
lang: ja
direction: ltr
source: docs/portal/docs/sorafs/chunker-profile-authoring.es.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
ID: チャンカープロファイルオーサリング
タイトル: SoraFS のチャンカーの自動ファイルの設定
サイドバーラベル: チャンカーの自動化ツール
説明: SoraFS のプロポナーの新しいパーファイルとチャンカーのフィクスチャのチェックリスト。
---

:::メモ フエンテ カノニカ
`docs/source/sorafs/chunker_profile_authoring.md` のページを参照してください。スフィンクスの記録を保存し、記録を保存する必要があります。
:::

# SoraFS チャンカーの自動ファイルの設定

SoraFS に関するチャンカーの詳細情報が公開されています。
建築 RFC の補完 (SF-1) と登録参照 (SF-2a)
オートリアの具体的な要求、有効性の確認、およびプロプエスタの植物の作成。
パラ・ウン・エジェンプロ・カノニコ、相談
`docs/source/sorafs/proposals/sorafs_sf1_profile_v1.json`
ドライラン協会の登録
`docs/source/sorafs/reports/sf1_determinism.md`。

## 履歴書

登録情報の詳細:

- マルチハッシュ識別子に関する CDC の決定および調整に関するパラメータの発表
  建築物。
- entregar フィクスチャの再現可能 (JSON Rust/Go/TS + corpora fuzz + testigos PoR) クエリ
  los SDK のダウンストリーム puedan verificar sin ツールが medida に組み込まれています。
- ロールアウトに必要なメタデータ リストを含める (名前空間、名前、サーバー)
  y ventanas operativas; y
- 見直しを行う前に、相違点を決定するための情報を入力してください。

必要なチェックリストを準備して、準備を整えてください。

## 登録履歴書

情報を編集する前に、登録申請の確認を確認してください
`sorafs_manifest::chunker_registry::ensure_charter_compliance()` の場合:

- 状況に応じて、正しい情報が失われます。
- El ハンドル canónico (`namespace.name@semver`) の別名リストのデベ アパレサー
  y **debe** ser la primera entrada。 Siguen los alias heredados (p. ej.、`sorafs.sf1@1.0.0`)。
- Ningún のエイリアスは、他のユーザーとの衝突を処理できます。
- 別名、白紙のスペイン語を記録する必要はありません。

CLI の機能:

```bash
# Listado JSON de todos los descriptores registrados (ids, handles, aliases, multihash)
cargo run -p sorafs_manifest --bin sorafs_manifest_chunk_store -- --list-profiles

# Emitir metadatos para un perfil por defecto candidato (handle canónico + aliases)
cargo run -p sorafs_manifest --bin sorafs_manifest_chunk_store -- \
  --promote-profile=sorafs.sf1@1.0.0 --json-out=-
```

定期的な記録とプロポーションの損失を考慮したコマンドの実行
必要なメタデータと政府の議論。

## メタデータス レケリドス

|カンポ |説明 |エジェンプロ (`sorafs.sf1@1.0.0`) |
|----------|---------------|----------------------------|
| `namespace` |関連ファイルの管理。 | `sorafs` |
| `name` |読みやすい人間のエチケット。 | `sf1` |
| `semver` |主要なパラメータの意味を理解するためのカデナ。 | `1.0.0` |
| `profile_id` |識別子は、完全なエントリの数を示します。再利用可能なリソースは存在しません。 | `1` |
| `profile_aliases` |クライアントの期間に応じて、オプションのオプション (名前、省略形) を処理します。初期ハンドルのハンドルが含まれます。 | `["sorafs.sf1@1.0.0"]` |
| `profile.min_size` |バイト単位の最小長さ。 | `65536` |
| `profile.target_size` |バイト単位のチャンクの長期オブジェクト。 | `262144` |
| `profile.max_size` |バイト単位の最大長さ。 | `524288` |
| `profile.break_mask` |米国ローリング ハッシュ (16 進数) に適応するマスカラ。 | `0x0000ffff` |
| `profile.polynomial` |コンスタンテ デル ポリノミオ ギア (六角)。 | `0x3da3358b4dc173` |
| `gear_seed` | Seed usada パラ デリバラ ラ タブラ ギア 64 KiB。 | `sorafs-v1-gear` |
| `chunk_multihash.code` | Código マルチハッシュ パラダイジェスト チャンク。 | `0x1f` (ブレイク3-256) |
| `chunk_multihash.digest` |フィクスチャーのバンドルのダイジェスト。 | `13fa...c482` |
| `fixtures_root` |監督は、試合の試合内容を確認します。 | `fixtures/sorafs_chunker/sorafs.sf1@1.0.0/` |
| `por_seed` | PoR 決定に関するシード (`splitmix64`)。 | `0xfeedbeefcafebabe` (例) |

ロスメタデータスデベンアパレーサータントエンエルドキュメントデプロプエスタコモデントロデロスフィクスチャ
レジストリ、CLI および政府の自動化ツールの更新
罪を犯したマニュアルを確認します。やあ、チャンクストアの CLI を取り出してください
マニフェスト コン `--json-out=-` は、メタデータの計算を送信する際の改訂履歴を示します。

### CLI とレジストリの連絡先- `sorafs_manifest_chunk_store --profile=<handle>` — チャンクのイジェクターメタデータのボルバー、
  マニフェストのダイジェストは、PoR のパラメータをチェックします。
- `sorafs_manifest_chunk_store --json-out=-` — チャンクストアのレポート送信
  標準出力の自動比較。
- `sorafs_manifest_stub --chunker-profile=<handle>` — 平面 CAR を確認するためのマニフェスト
  embeben el ハンドル canónico más los alias。
- `sorafs_manifest_stub --plan=-` — 消化器系のボルバー `chunk_fetch_specs` 前のパラ
  verificar はカンビオのオフセット/ダイジェストを作成します。

コマンドの登録 (ダイジェスト、Raíces PoR、ハッシュ・デ・マニフェスト) en la propuesta para que
テキストの複製を参照してください。

## 決定性検証のチェックリスト

1. **リジェネレーター設備**
   ```bash
   cargo run --locked -p sorafs_chunker --bin export_vectors \
     --signature-out=fixtures/sorafs_chunker/manifest_signatures.json
   ```
2. **パリパリスイートの出力** — `cargo test -p sorafs_chunker` とアルネスの差分
   entre lenguajes (`crates/sorafs_chunker/tests/vectors.rs`) デベン・スター・アン・ベルデ・コン・ロス
   ヌエボスの試合はルーガルで行われます。
3. **コーポラ ファズ/バック プレッシャーの再現** — 出力 `cargo fuzz list` およびアルネス デ
   ストリーミング (`fuzz/sorafs_chunker`) 活動を再開します。
4. **検証テストの取得可能性の証明** — ejecuta
   `sorafs_manifest_chunk_store --por-sample=<n>` usando el perfil propuesto yconfirma que las
   人種はマニフェストの条件と一致します。
5. **CI のドライラン** — `ci/check_sorafs_fixtures.sh` ローカルメンテを呼び出します。エルスクリプト
   デベ テナー エクスト コン ロス ヌエボス フィクスチャと `manifest_signatures.json` が存在します。
6. **クロスランタイムの確認** — Go/TS コンシューマ エル JSON のバインディングを確保する
   制限を与えず、同一の内容を消化します。

ツーリング WG プエダのプロプエスタに関する文書のダイジェスト結果
罪の想念の繰り返し。

### マニフェスト/PoR の確認

再生成のフィクスチャ、マニフェストの完全なパイプラインの取り出し
メタデータ CAR と las pruebas PoR sigan siendo の一貫性:

```bash
# Validar metadata de chunk + PoR con el nuevo perfil
cargo run -p sorafs_manifest --bin sorafs_manifest_chunk_store -- \
  --profile=sorafs.sf2@1.0.0 \
  --json-out=- --por-json-out=- fixtures/sorafs_chunker/input.bin

# Generar manifest + CAR y capturar chunk fetch specs
cargo run -p sorafs_manifest --bin sorafs_manifest_stub -- \
  fixtures/sorafs_chunker/input.bin \
  --chunker-profile=sorafs.sf2@1.0.0 \
  --chunk-fetch-plan-out=chunk_plan.json \
  --manifest-out=sf2.manifest \
  --car-out=sf2.car \
  --json-out=sf2.report.json

# Reejecutar usando el plan de fetch guardado (evita offsets obsoletos)
cargo run -p sorafs_manifest --bin sorafs_manifest_stub -- \
  fixtures/sorafs_chunker/input.bin \
  --chunker-profile=sorafs.sf2@1.0.0 \
  --plan=chunk_plan.json --json-out=-
```

Reemplaza el archive de entrada con cualquier corpus representativo usado por tus fixtures
(p. ej.、el stream determinista de 1 GiB) y adjunta los digestes a la propuesta。

## プランティーリャ デ プロプエスタ

登録情報を参照してください Norito `ChunkerProfileProposalV1` 登録情報
`docs/source/sorafs/proposals/`。 La plantilla JSON de abajo Ilustra la forma esperada
(必要な海を守る必要があります):


マークダウン対応レポート (`determinism_report`) のキャプチャ ラ
コマンドを実行し、チャンクのダイジェストを確認し、検証期間を延長します。

## フルホ デ ゴベルナンサ

1. **Enviar PR con propuesta + 備品。** ロスアセットジェネラドス、ラプロプエスタを含む
   Norito は `chunker_registry_data.rs` を実現します。
2. **ツールの改訂 WG.** 検証チェックリストの改訂
   登録内容を確認してください (再利用罪、
   決定性は満足です）。
3. **Sobre del consejo.** 最高のアプロバド、ロス ミエンブロス デル コンセホ ファームマン エル ダイジェスト
   ラ プロプエスタ (`blake3("sorafs-chunker-profile-v1" || canonical_bytes)`) とアネクサンサス
   ロスの試合に向けての準備を整えます。
4. **登録情報の公開** 実際の登録情報、ドキュメントとフィクスチャを結合します。エル
   CLI による永久的な不正行為の以前の移行の宣言
   リスタ。
5. **非推奨の決定** 登録上の現実的な移行の防止
   移住台帳。

## コンセホス デ オートリア

- 大量のチャンクを最小限に抑えて、その可能性を制限する必要があります。
- マニフェスト ゲートウェイのマルチハッシュ コンスミドールのコードを作成します。ウナを含む
  ノタ・オペラティヴァ・クアンド・ロ・ハガス。
- タブラギアの種を読みやすく、人間の視点からグローバルに簡単に理解できるようにする
  オーディトリアス。
- ベンチマークに関する適切な成果物 (英語ページ、スループットの比較)
  `docs/source/sorafs/reports/` パラリファレンス フューチュラ。

予想される運用期間、ロールアウト、移行元帳の確認
(`docs/source/sorafs/migration_ledger.md`)。ランタイム版の適合性に関する規則
`docs/source/sorafs/chunker_conformance.md`。