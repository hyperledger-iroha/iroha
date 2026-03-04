---
lang: ja
direction: ltr
source: docs/portal/docs/sorafs/chunker-profile-authoring.pt.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
ID: チャンカープロファイルオーサリング
タイトル: SoraFS の自動チャンカーの情報
サイドバーラベル: チャンカーの自動化ツール
説明: SoraFS のチャンカーの新しい装備に関するチェックリスト。
---

:::note フォンテ カノニカ
エスタ・ページナ・エスペルハ`docs/source/sorafs/chunker_profile_authoring.md`。マンテンハ・アンバスはコピア・シンクロニザダスとして。
:::

# SoraFS のチャンカーの自動化のガイア

SoraFS に関する詳細な情報が公開されています。
RFC アーキテクチャの補完 (SF-1) と登録参照 (SF-2a)
com requisitos concretos de autoria、etapas de validacao、modelos de proposta。
カノニコ、ヴェジャの例
`docs/source/sorafs/proposals/sorafs_sf1_profile_v1.json`
予行演習関連のログ
`docs/source/sorafs/reports/sf1_determinism.md`。

## ヴィサオ・ゲラル

登録情報の詳細を確認する:

- マルチハッシュ ID の設定に関する CDC 決定パラメータの発表
  建築家。
- entregar フィクスチャの再現 (JSON Rust/Go/TS + corpora fuzz + testemunhas PoR) クエリ
  os SDK のダウンストリーム possam verificar sem ツールの sob medida。
- ガバナンカのメタダドス プロントを含める (名前空間、名前、サーバー) junto com orientacao
  ジャネラス作戦の展開。 e
- コンセルホの見直しを行う前に、相違点を決定するための一連の情報を確認してください。

シンガポールはチェックリストを準備し、事前に準備する必要があります。

## 登録履歴

事前登録を行って、申請書を登録するかどうかを確認してください
`sorafs_manifest::chunker_registry::ensure_charter_compliance()`:

- ID は、単調な問題を解決するための確実な情報を提供します。
- O ハンドル canonico (`namespace.name@semver`) は、別名リストを作成します。
  **最初のエントラーダを開発**してください。エイリアス alternativos (例、`sorafs.sf1@1.0.0`) vem depois。
- Nenhum の別名 podecolidir com アウトロ ハンドル canonico ou aparecer mais de uma vez です。
- 別名 devem ser nao vazios e aparados de espacos em branco。

CLI のヘルパー:

```bash
# Listagem JSON de todos os descritores registrados (ids, handles, aliases, multihash)
cargo run -p sorafs_manifest --bin sorafs_manifest_chunk_store -- --list-profiles

# Emitir metadados para um perfil default candidato (handle canonico + aliases)
cargo run -p sorafs_manifest --bin sorafs_manifest_chunk_store -- \
  --promote-profile=sorafs.sf1@1.0.0 --json-out=-
```

Esses comandos mantem as propostas alinhadas com a carta do registro e fornecem os
統治の必要性を議論するメタダドス キャノニコス。

## メタダドス レケリドス

|カンポ |説明 |例 (`sorafs.sf1@1.0.0`) |
|------|-----------|----------------------------|
| `namespace` |適切な関係を実現するためのロジック。 | `sorafs` |
| `name` |ロトゥーロ・レジベル・パラ・ヒューノス。 | `sf1` |
| `semver` |パラメトロスと接続されたセマンティカの説明。 | `1.0.0` |
| `profile_id` |識別子は、モノトーンのアトリビュードと完全なエントラを識別します。近くにあるものを予約し、存在する数を再利用します。 | `1` |
| `profile_aliases` | adicionais opcionais (nomes alternativos、abreviacoes) expostos a clientes durante a negociacao を処理します。 canonico como primeira entrada のハンドルを含む。 | `["sorafs.sf1@1.0.0"]` |
| `profile.min_size` |ミニモはバイトをチャンクします。 | `65536` |
| `profile.target_size` |バイトをすべてチャンク化します。 | `262144` |
| `profile.max_size` |バイトを最大限にチャンクします。 | `524288` |
| `profile.break_mask` |マスカラ アダプティバ ウサダ ペロ ローリング ハッシュ (16 進数)。 | `0x0000ffff` |
| `profile.polynomial` |コンスタンテ ド ポリノミオ ギア (16 進数)。 | `0x3da3358b4dc173` |
| `gear_seed` |シードusadaパラ派生タベラギア64 KiB。 | `sorafs-v1-gear` |
| `chunk_multihash.code` | Codigo マルチハッシュパラはチャンクをダイジェストします。 | `0x1f` (ブレイク3-256) |
| `chunk_multihash.digest` |フィクスチャのカノニコをバンドルするダイジェスト。 | `13fa...c482` |
| `fixtures_root` | OS フィクスチャの再生に関するディレトリオ。 | `fixtures/sorafs_chunker/sorafs.sf1@1.0.0/` |
| `por_seed` |シードパラアモスストラジェム PoR 決定論 (`splitmix64`)。 | `0xfeedbeefcafebabe` (例) |

OS メタダドス デベム アパレサー タント ドキュメントなし、プロポスト クアント デントロ ドス フィクスチャ ゲラドス
CLI のレジストリ、ツール、管理の自動確認、OS の有効性の確認
クルサメントス・マヌアイス。必要に応じて、OS CLI をチャンクストアとマニフェスト COM で実行します。
`--json-out=-` 送信時のメタデータ計算の見直しに関する注意事項。

### CLI レジストロへの接続

- `sorafs_manifest_chunk_store --profile=<handle>` - チャンクのメタデータを再実行します。
  ダイジェストはマニフェストを実行して、PoR comos parametros propostos をチェックします。
- `sorafs_manifest_chunk_store --json-out=-` - チャンクストアパラメタの送信
  標準出力と自動比較。
- `sorafs_manifest_stub --chunker-profile=<handle>` - Planos CAR のマニフェストの確認
  embutem は canonico mais エイリアスを処理します。
- `sorafs_manifest_stub --plan=-` - 再認識 o `chunk_fetch_specs` 前パラ
  verificar はムダンカのアポをオフセット/ダイジェストします。必要に応じて、発言コマンド (ダイジェスト、PoR の提起、マニフェストのハッシュ) を登録します。
OS は、文字通りの再現性を修正します。

## 決定性と検証のチェックリスト

1. **リジェネレーター設備**
   ```bash
   cargo run --locked -p sorafs_chunker --bin export_vectors \
     --signature-out=fixtures/sorafs_chunker/manifest_signatures.json
   ```
2. **スイートの実行者** - `cargo test -p sorafs_chunker` ハーネスの差分
   クロスランゲージ (`crates/sorafs_chunker/tests/vectors.rs`) 開発 ficar verdes com os
   ノボスの備品にはルーガーはありません。
3. **コーパス ファズ/バック プレッシャーを再実行** - `cargo fuzz list` e o harness を実行します。
   ストリーミング (`fuzz/sorafs_chunker`) コントラ OS 資産が再生されます。
4. **検証テストの取得可能性の証明** - 実行
   `sorafs_manifest_chunk_store --por-sample=<n>` usando perfil proposto econfirme
   対応するマニフェスト デ フィクスチャを生成します。
5. **CI のドライラン** - `ci/check_sorafs_fixtures.sh` ローカルメントを実行します。 o スクリプト
   開発者は、`manifest_signatures.json` を備えたノボス フィクスチャを成功させました。
6. **クロスランタイムの確認** - Go/TS コンシューマまたは JSON のキュー OS バインディングを確保する
   再生と放出の制限とダイジェストは同一です。

ツール WG の提案に従って、OS のコマンドと OS のダイジェスト結果を文書化します。
sem adivinhacoes を再実行します。

### マニフェスト/PoR の確認

再生フィクスチャの保管、マニフェスト保証を完了するためのパイプラインの実行
メタダドス CAR とプロバス PoR の継続は次のとおりです。

```bash
# Validar metadados de chunk + PoR com o novo perfil
cargo run -p sorafs_manifest --bin sorafs_manifest_chunk_store -- \
  --profile=sorafs.sf2@1.0.0 \
  --json-out=- --por-json-out=- fixtures/sorafs_chunker/input.bin

# Gerar manifest + CAR e capturar chunk fetch specs
cargo run -p sorafs_manifest --bin sorafs_manifest_stub -- \
  fixtures/sorafs_chunker/input.bin \
  --chunker-profile=sorafs.sf2@1.0.0 \
  --chunk-fetch-plan-out=chunk_plan.json \
  --manifest-out=sf2.manifest \
  --car-out=sf2.car \
  --json-out=sf2.report.json

# Reexecutar usando o plano de fetch salvo (evita offsets obsoletos)
cargo run -p sorafs_manifest --bin sorafs_manifest_stub -- \
  fixtures/sorafs_chunker/input.bin \
  --chunker-profile=sorafs.sf2@1.0.0 \
  --plan=chunk_plan.json --json-out=-
```

米国の備品を代表するクアルケール コーパスのデータの置換
(例、1 GiB のストリーム決定性) 付属の os ダイジェストの結果としてプロポスタが生成されます。

## プロポスタのモデル

As propostas sao submetidas como registros Norito `ChunkerProfileProposalV1` registraros em
`docs/source/sorafs/proposals/`。 O テンプレート JSON abaixo illustra または formato esperado
(必要に応じて代用してください):


Forneca um relatorio Markdown 通信 (`determinism_report`) のキャプチャ
一連のコマンドをダイジェストし、有効な期間の内容を確認します。

## フルクソ デ ガバナンカ

1. **Submeter PR com proposta + fixtures.** os 資産 gerados、proposta を含む
   Norito は `chunker_registry_data.rs` と一致します。
2. **ツール WG を改訂します。** 検証チェックリストを再実行して確認します。
   regras do registro (sem reutilizacao de id, determinismoSatisfeito) のように、proposta segue を実行してください。
3. **Envelope do conselho.** Uma vez aprovado, membros do conselho assinam o Diet da
   提案書 (`blake3("sorafs-chunker-profile-v1" || canonical_bytes)`) に関する試験
   assinaturas ao エンベロープは、アルマゼナド ジュント aos フィクスチャを実行します。
4. **登録を公開します。** 登録、ドキュメント、フィクスチャをマージします。 O CLI
   デフォルトの永続性は、政府が移行を宣言する前に実行する必要はありません。
5. **Rastreamento de deprecacao.** Apos a janela de migracao, atualize o registro para

## ディカス・デ・オートリア

- Prefira は、大量の処理を最小限に抑え、その可能性を制限します。
- マニフェストとゲートウェイの管理者を管理するマルチハッシュのコードを作成します。うまを含む
  操作性のクアンドファイザーはありません。
- 種子としてのマンテンハは、人間のための合法的手段であり、単純な聴覚のための世界的なユニカスです。
- ベンチマークのアルマゼント技術 (スループットの比較など)
  `docs/source/sorafs/reports/` パラリファレンス フューチュラ。

ロールアウトの継続的な運用、移行元帳の確認などの期待
(`docs/source/sorafs/migration_ledger.md`)。ランタイム、ベジャの適合性に関する規則
`docs/source/sorafs/chunker_conformance.md`。