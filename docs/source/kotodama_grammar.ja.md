<!-- Japanese translation of docs/source/kotodama_grammar.md -->

---
lang: ja
direction: ltr
source: docs/source/kotodama_grammar.md
status: complete
translator: manual
---

# Kotodama 言語の文法とセマンティクス

このドキュメントは、Kotodama 言語の構文（字句解析・文法）、型付け規則、決定論的セマンティクス、そして Norito のポインター ABI 規約に従って IVM バイトコード（`.to`）へローアリングする仕組みを定義します。Kotodama のソースファイルは `.ko` 拡張子を使用し、コンパイラは IVM バイトコード（`.to`）と任意でマニフェストを出力します。

目次
- 概要とゴール
- 字句構造
- 型とリテラル
- 宣言とモジュール
- コントラクトコンテナとメタデータ
- 関数とパラメータ
- ステートメント
- 式
- ビルトインとポインター ABI コンストラクタ
- コレクションとマップ
- 決定論的な反復と境界
- エラーと診断
- IVM へのコード生成マッピング
- ABI・ヘッダー・マニフェスト
- ロードマップ

## 概要とゴール

- **決定論性**: プログラムはハードウェアを問わず同一の結果を生成しなければなりません。浮動小数点や非決定的なソースは禁止され、ホストとの相互作用は Norito でエンコードされた引数を持つシステムコール経由でのみ行います。
- **移植性**: コンパイル対象は物理 ISA ではなく Iroha Virtual Machine (IVM) バイトコードです。リポジトリで見られる RISC-V 風エンコーディングは IVM のデコーダ実装上の詳細であり、観測可能な挙動を変えてはなりません。
- **監査容易性**: セマンティクスは小さく明示的であり、構文から IVM オペコードやホストシステムコールへの対応が明確です。
- **境界管理**: 無境界データを扱うループには明示的な境界が必要です。マップの反復には決定論を保証する厳密な規則があります。

## 字句構造

### 空白とコメント
- 空白はトークンを区切る役割のみを持ち、それ以外の意味はありません。
- 行コメントは `//` で始まり行末まで続きます。
- ブロックコメント `/* ... */` は入れ子不可です。

### 識別子
- 先頭は `[A-Za-z_]`、続く文字は `[A-Za-z0-9_]*` です。
- 大文字小文字は区別されます。`_` 単体も識別子として有効ですが非推奨です。

### 予約語
`seiyaku`, `hajimari`, `kotoage`, `kaizen`, `state`, `struct`, `fn`, `let`, `const`, `return`, `if`, `else`, `while`, `for`, `in`, `break`, `continue`, `true`, `false`, `permission`, `kotoba`。

### 演算子と句読記号
- 算術: `+ - * / %`
- ビット演算: `& | ^ ~`、シフト `<< >>`
- 比較: `== != < <= > >=`
- 論理: `&& || !`
- 代入: `= += -= *= /= %= &= |= ^= <<= >>=`
- その他: `: , ; . :: ->`
- 括弧: `() [] {}`

### リテラル
- 整数: 10 進 (`123`)、16 進 (`0x2A`)、2 進 (`0b1010`)。実行時はすべて符号付き 64 ビットです。サフィックス無しリテラルは型推論され、既定では `int` となります。
- 文字列: ダブルクォートで囲み、`\` エスケープを使用できます。UTF-8です。
- 真偽値: `true` と `false`。

## 型とリテラル

### スカラー型
- `int`: 64 ビット 2 の補数。加減乗は 2^64 を法とするラップ演算、除算は IVM で定義済みの符号付き／符号なし命令にマッピングされ、コンパイラが適切な命令を選択します。
- `bool`: 論理値。実行時は `0`/`1` にローアリングされます。
- `string`: 不変の UTF-8 文字列。システムコール引数では Norito TLV として表現し、VM 内部ではバイトスライスと長さで扱います。

### 複合型
- `struct Name { field: Type, ... }`: 利用者定義の直積型。式では `Name(a, b, ...)` という関数呼び出し形式でコンストラクトします。フィールドアクセス `obj.field` をサポートし、内部的にはタプル風の位置フィールドにローアリングされます。Durable 状態の ABI は Norito エンコードで固定されており、Iroha Core のオーバーレイ実装（`kotodama_struct_overlay` テスト）でフィールド順とレイアウトがロックされています。
- `Map<K, V>`: 決定論的な連想マップ。反復や反復中のミューテーションに制限があります（後述）。
- `Tuple (T1, T2, ...)`: 無名の直積型。複数戻り値に利用します。

### 特殊なポインター ABI 型（ホスト向け）
`AccountId`, `AssetDefinitionId`, `Name`, `Json`, `NftId`, `Blob` などはランタイムの第一級型ではなく、INPUT 領域内の Norito TLV エンベロープを指す型付き不変ポインターを生成するコンストラクタです。システムコール引数として使用するか、値をコピーすることのみが可能で、ミューテーションはできません。

### 型推論
- ローカルな `let` は初期化式から型を推論します。関数パラメータは明示的な型指定が必要です。戻り値型は曖昧でない限り推論可能です。

## 宣言とモジュール

### トップレベル項目
- コントラクト: `seiyaku Name { ... }` は関数、状態、構造体、メタデータを含みます。
- 1 ファイルに複数のコントラクトを記述できますが非推奨です。マニフェストでは主要な `seiyaku` が既定エントリとして扱われます。
- `struct` 宣言によりコントラクト内で利用者定義型を宣言します。

### 可視性
- `kotoage fn` は公開エントリポイントを示します。可視性はディスパッチャのパーミッションに影響し、コード生成には影響しません。

## コントラクトコンテナとメタデータ

### 構文
```kotodama
seiyaku Name {
  meta {
    abi_version: 1,
    vector_length: 0,
    max_cycles: 0,
    features: ["zk", "simd"],
  }

  state int counter;

  hajimari() { counter = 0; }

  kotoage fn inc() { counter = counter + 1; }
}
```

### セマンティクス
- `meta { ... }` は出力される IVM ヘッダーの既定値を上書きします。`abi_version`、`vector_length`（0 は未設定）、`max_cycles`（0 はコンパイラ既定値）、`features` はヘッダのフィーチャビット（ZK トレース、ベクトル告知など）を切り替えます。未サポートのフィーチャは警告付きで無視されます。`meta {}` を省略した場合、コンパイラは `abi_version = 1` を出力し、その他の項目はオプションの既定値を用います。
- `state` はランタイムが保持する耐久変数を宣言します。ABI は Norito エンコードでオンチェーン保存され、順序とフィールド名を保つ必要があります。関数からのミューテーションはホストシステムコールを通じて ISI を生成し、VM コードが直接 WSV を変更することはできません。

## トリガー宣言

トリガー宣言はエントリーポイントマニフェストにスケジューリング用のメタデータを付与し、
コントラクトインスタンスの有効化時に自動登録（無効化時に削除）されます。`seiyaku`
ブロック内で解析されます。

### 構文
```
register_trigger wake {
  call run;
  on time pre_commit;
  repeats 2;
  metadata { tag: "alpha"; count: 1; enabled: true; }
}
```

### 注記
- `call` は同一コントラクト内の公開 `kotoage fn` を参照する必要があります。
  `namespace::entrypoint` はマニフェストに記録されますが、クロスコントラクトの
  コールバックは現時点では拒否されます（ローカルのみ）。
- 対応フィルタ: `time pre_commit` と `time schedule(start_ms, period_ms?)`、および
  `execute trigger <name>`（by-call）。data/pipeline フィルタは未対応です。
- metadata 値は JSON リテラル（`string`, `number`, `bool`, `null`）または `json!(...)`。
- ランタイムが注入する metadata キー: `contract_namespace`, `contract_id`,
  `contract_entrypoint`, `contract_code_hash`, `contract_trigger_id`。

## 関数とパラメータ

### 構文
- 宣言: `fn name(param1: Type, param2: Type, ...) -> Ret { ... }`
- 公開関数: `kotoage fn name(...) { ... }`
- イニシャライザ: `hajimari() { ... }`（デプロイ時にランタイムから呼び出され、VM 自身が直接呼びません）。
- アップグレードフック: `kaizen(args...) permission(Role) { ... }`。

### パラメータと戻り値
- 引数は ABI に従いレジスタ `r10..r22` に値または INPUT ポインター（Norito TLV）として渡され、追加の引数はスタックにあふれます。
- 関数は 0 個または 1 個のスカラ、もしくはタプルを返します。スカラ戻り値は `r10` に配置され、タプルは規約に従ってスタック／OUTPUT に配置されます。

## ステートメント

- 変数束縛: `let x = expr;`、`let mut x = expr;`（ミュータブル指定はコンパイル時にのみ検証され、ランタイムではローカル変数の再代入が可能です）。
- 代入: `x = expr;` と複合代入 `x += 1;` など。
- 制御構造: `if (cond) { ... } else { ... }`、`while (cond) { ... }`、C 風 `for (init; cond; step) { ... }`。
- マップループ: `for (k, v) in map { ... }`（決定論的。後述）。
- フロー制御: `return expr;`、`break;`、`continue;`。
- 呼び出し: `name(args...);` または `call name(args...);`（両方受理され、コンパイラが統一します）。
- アサーション: `assert(cond, msg_id?);`、`assert_eq(a, b, msg_id?);` は非 ZK ビルドでは IVM `ASSERT*` に、ZK モードでは ZK 制約にマッピングされます。

## 式

### 優先順位（高 → 低）
1. メンバ／インデックス: `a.b`, `a[b]`
2. 単項演算: `! ~ -`
3. 乗除剰余: `* / %`
4. 加減: `+ -`
5. シフト: `<< >>`
6. 大小比較: `< <= > >=`
7. 等値比較: `== !=`
8. ビット積・排他和・和: `& ^ |`
9. 論理積・論理和: `&& ||`
10. 三項（予約済み）: `cond ? a : b`（未実装）

### 呼び出しとタプル
- 呼び出しは位置引数を用います: `f(a, b, c)`。
- タプルリテラル: `(a, b, c)`。分解: `let (x, y) = pair;`。

### 文字列とバイト列
- 文字列は UTF-8 です。生バイトを必要とする関数は、コンストラクタを通じて `Blob` ポインターを受け取ります（ビルトイン参照）。

## ビルトインとポインター ABI コンストラクタ

### ポインターコンストラクタ（Norito TLV を INPUT に生成し型付きポインターを返す）
- `account_id(string) -> AccountId*`
- `asset_definition(string) -> AssetDefinitionId*`
- `asset_id(string) -> AssetId*`
- `domain(string)` または `domain_id(string) -> DomainId*`
- `name(string) -> Name*`
- `json(string) -> Json*`
- `nft_id(string) -> NftId*`
- `blob(bytes|string) -> Blob*`
- `norito_bytes(bytes|string) -> NoritoBytes*`

プレリュードにはこれらを呼び出すマクロも含まれます。
- `account!("ih58...")` / `account_id!("ih58...")`
- `asset_definition!("rose#wonderland")` / `asset_id!("rose#wonderland")`
- `domain!("wonderland")` / `domain_id!("wonderland")`
- `name!("example")`
- `json!("{\"hello\":\"world\"}")` や `json!{ hello: "world" }`
- `nft_id!("dragon#demo")`, `blob!("bytes")`, `norito_bytes!("...")`

マクロはコンパイル時にリテラルを検証し、上記のコンストラクタへ展開されます。

### 実装状況
- 上記コンストラクタは文字列リテラル引数を受け取り、INPUT 領域に型付き Norito TLV エンベロープを配置して、不変ポインターを返します。これらはシステムコール引数として利用できます。
- 拡張形:
  - `json(Blob[NoritoBytes]) -> Json*` は `JSON_DECODE` システムコールを経由します。
  - `name(Blob[NoritoBytes]) -> Name*` は `NAME_DECODE` システムコールを経由します。
  - ポインター引数のパススルー: `name(Name) -> Name*`, `blob(Blob) -> Blob*`, `norito_bytes(Blob) -> Blob*`。
  - メソッドシュガーをサポート: `s.name()`, `s.json()`, `b.blob()`, `b.norito_bytes()`。

### ホスト／システムコール系ビルトイン（SCALL へマッピング。番号は `ivm.md` 参照）
- `mint_asset(AccountId*, AssetDefinitionId*, numeric)`
- `burn_asset(AccountId*, AssetDefinitionId*, numeric)`
- `transfer_asset(AccountId*, AccountId*, AssetDefinitionId*, numeric)`
- `set_account_detail(AccountId*, Name*, Json*)`
- `nft_mint_asset(NftId*, AccountId*)`
- `nft_transfer_asset(AccountId*, NftId*, AccountId*)`
- `nft_set_metadata(NftId*, Json*)`
- `nft_burn_asset(NftId*)`
- `authority() -> AccountId*`
- `register_domain(DomainId*)`
- `unregister_domain(DomainId*)`
- `transfer_domain(AccountId*, DomainId*, AccountId*)`
- `contains(Map<K,V>, K) -> bool`

### ユーティリティ系ビルトイン
- `info(string|int|tuple)`: 構造化イベント／メッセージを OUTPUT に出力します。
- `hash(blob) -> Blob*`: Norito エンコードされたハッシュを Blob として返します。
- `build_submit_ballot_inline(election_id, ciphertext, nullifier32, backend, proof, vk)` と `build_unshield_inline(asset, to, amount, inputs32, backend, proof, vk)`: インライン ISI ビルダー。引数はすべてコンパイル時リテラル（文字列リテラル、またはリテラルから生成したポインターコンストラクタ）でなければなりません。`nullifier32` と `inputs32` は 32 バイト固定（生文字列または `0x` 16進）、`amount` は非負。戻り値は NoritoBytes を指す `Blob*` です。

### 備考
- ビルトインは薄いラッパーであり、コンパイラはレジスタ転送と `SCALL` にローアリングします。
- ポインターコンストラクタは純粋関数であり、VM は呼び出し中 INPUT の Norito TLV が不変であることを保証します。
- ポインター ABI フィールドを持つ構造体（例: `DomainId`, `AccountId`）を用いることで、システムコール引数をまとめて扱えます。コンパイラは `obj.field` を適切なレジスタ／値へマッピングし、追加の割り当てを発生させません。

## コレクションとマップ

### 型: `Map<K, V>`
- キーと値は Norito TLV にシリアル化可能でなければなりません。サポートされるキー型: `int`、`string`、サポート型からなるタプルおよび構造体。値型: サポートされる任意の型。
- 操作:
  - インデックスアクセス: `map[key]` は値の取得／設定（設定はホストシステムコールを通じて実行されます）。
  - 存在確認: `contains(map, key) -> bool`（ローアリング時のヘルパー。専用システムコールになる場合があります）。
  - 反復: `for (k, v) in map { ... }` は決定論的な順序とミューテーション規則を持ちます。

### 決定論的反復の規則
- 反復集合はループ開始時点のキーのスナップショットです。
- 順序は Norito エンコードされたキーの昇順バイト列順序で固定されます。
- ループ中に反復対象マップを構造的に変更（挿入・削除・クリア）すると決定論的に `E_ITER_MUTATION` トラップが発生します。
- 境界指定が必須です。マップに `@max_len` などの上限が宣言されているか、`#[bounded(n)]` 属性、または `.take(n)` / `.range(..)` を用いて明示する必要があります。そうでない場合、コンパイラは `E_UNBOUNDED_ITERATION` を発生させます。

### 境界ヘルパー
- `#[bounded(n)]`: たとえば `for (k, v) in map #[bounded(2)] { ... }` のように指定します。
- `.take(n)`: 先頭から `n` 件を反復します。
- `.range(start, end)`: 半開区間 `[start, end)` の要素を反復します。セマンティクスは `start` と `n = end - start` を指定するのと同等です。

### 動的境界に関する注意
- リテラル境界: `n`、`start`、`end` が整数リテラルの場合、固定回数の反復としてコンパイルされます。
- 非リテラル境界: `ivm` クレートで `kotodama_dynamic_bounds` フィーチャを有効にすると、動的な `n`、`start`、`end` 式を許可し安全性のためのランタイムアサーション（非負かつ `end >= start`）を挿入します。ローアリングでは最大 K 回のガード付き反復を生成し、`if (i < n)` チェックで余分な本体実行を防ぎます（既定 K=2）。K は `CompilerOptions { dynamic_iter_cap, .. }` で調整可能です。
- コンパイル前に `koto_lint` を実行すると Kotodama の警告を確認できます。本体のコンパイラは構文解析と型検査の後で必ずローアリングまで進みます。
- エラーコードは [Kotodama Compiler Error Codes](./kotodama_error_codes.md) に記載されており、`koto_compile --explain <code>` で解説を確認できます。

## エラーと診断

### コンパイル時診断（例）
- `E_UNBOUNDED_ITERATION`: マップのループに境界がありません。
- `E_MUT_DURING_ITER`: ループ本体で反復対象マップを構造的に変更しました。
- `E_BAD_POINTER_USE`: ポインター ABI コンストラクタの結果を第一級型として誤用しました。
- `E_UNRESOLVED_NAME`, `E_TYPE_MISMATCH`, `E_ARITY_MISMATCH`, `E_DUP_SYMBOL`。

### ランタイム VM エラー（抜粋。詳細は `ivm.md`）
- `E_NORITO_INVALID`, `E_OOB`, `E_UNALIGNED`, `E_SCALL_UNKNOWN`, `E_ASSERT`, `E_ASSERT_EQ`, `E_ITER_MUTATION`。

### エラーメッセージ
- 診断には安定した `msg_id` が付与され、`kotoba {}` 翻訳テーブルが存在する場合に対応づけられます。

## IVM へのコード生成マッピング

### パイプライン
1. 字句解析・構文解析で AST を生成。
2. セマンティック解析で名前解決、型検査、シンボルテーブル構築。
3. 単純な SSA 風 IR へローアリング。
4. IVM GPR（呼び出し規約上の `r10+`）へのレジスタ割り当てとスタックへのスピル。
5. バイトコード生成: IVM ネイティブ命令と RV 互換エンコーディングを適宜混在させ、`abi_version`、フィーチャ、ベクトル長、`max_cycles` を含むメタデータヘッダーを出力。

### マッピングの要点
- 算術・論理は IVM の ALU 命令にマップされます。
- 分岐・制御は条件分岐とジャンプに変換され、適宜圧縮形式を利用します。
- ローカル変数用のメモリは VM スタックにスピルされ、アライメントが強制されます。
- ビルトインはレジスタ転送と `SCALL`（8 ビット番号）にローアリングされます。
- ポインターコンストラクタは Norito TLV を INPUT 領域に配置し、そのアドレスを生成します。
- アサーションは `ASSERT` / `ASSERT_EQ` にマップされ、非 ZK 実行ではトラップ、ZK ビルドでは制約を生成します。

### 決定論制約
- 浮動小数点および非決定的システムコールは禁止です。
- SIMD / GPU アクセラレーションはバイトコードから見て透過であり、ビット単位で一致しなければなりません。コンパイラはハードウェア専用命令を生成しません。

## ABI・ヘッダー・マニフェスト

### コンパイラが設定する IVM ヘッダーフィールド
- `version`: IVM バイトコード形式のバージョン（major.minor）。
- `abi_version`: システムコール表とポインター ABI スキーマのバージョン。
- `feature_bits`: フィーチャフラグ（例: `ZK`, `VECTOR`）。
- `vector_len`: 論理ベクトル長（0 は未設定）。
- `max_cycles`: アドミッション境界および ZK のパディングヒント。

### マニフェスト（任意のサイドカー）
- `code_hash`、`abi_hash`、`meta {}` ブロックからのメタデータ、コンパイラバージョン、再現性のためのビルドヒントを含みます。

## ロードマップ

- **KD-231 (2026年4月)**: 反復境界に対するコンパイル時レンジ解析を導入し、ループがアクセスセットを静的に公開できるようにする。
- **KD-235 (2026年5月)**: ポインターコンストラクタの明確化のため、`string` と区別された第一級 `bytes` 型を追加する。
- **KD-242 (2026年6月)**: フィーチャフラグ付きでビルトイン集合（暗号ハッシュ、署名検証 opcode 等）を拡充し、決定論的フォールバックを提供する。
- **KD-247 (2026年6月)**: エラー `msg_id` を安定化させ、`kotoba {}` テーブルにリンクすることでローカライズ済み診断と同期させる。

### マニフェストのエミッション

- Kotodama コンパイラ API は `ivm::kotodama::compiler::Compiler::compile_source_with_manifest` を通じて、コンパイル済み `.to` と併せて `ContractManifest` を返すことができます。
- フィールド:
  - `code_hash`: IVM ヘッダーとリテラルを除くコードバイトのハッシュ。アーティファクトの同一性を束縛します。
  - `abi_hash`: プログラムの `abi_version` における許可システムコール面の安定ダイジェスト（`ivm.md` および `ivm::syscalls::compute_abi_hash` 参照）。
  - 任意の `compiler_fingerprint` と `features_bitmap` はツールチェーン向けに予約されています。
  - マニフェストはアドミッション時の検証やレジストリ向けを想定しています。ライフサイクルは `docs/source/new_pipeline.md` を参照してください。
