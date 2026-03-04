---
lang: ja
direction: ltr
source: docs/portal/docs/nexus/nexus-spec.fr.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
ID: ネクサススペック
title:仕様テクニック de Sora Nexus
説明: `docs/source/nexus.md` のミロワール、Iroha 3 (Sora Nexus) のアーキテクチャと概念の制約を確認します。
---

:::note ソースカノニク
Cette ページの再表示 `docs/source/nexus.md`。 Gardez les deux のコピーは、取引のバックログが到着するまでに alignees jusqu'a ce que que le portail に到着します。
:::

#! Iroha 3 - Sora Nexus Ledger: 仕様技術の概念化

Ce ドキュメントは、Nexus 台帳 Iroha 3 のアーキテクチャを提案しており、Iroha 2 つの台帳とグローバルな一意の論理を統合して、データ スペース (DS) の自動編成を提案しています。 Les Data Spaces fournissent des Domaines de confidentialite forts (「プライベート データ スペース」) および参加 ouverte (「パブリック データ スペース」)。設計は、包括的な構成を維持し、元帳のグローバルな分離を厳格に保持し、プライベート DS の機密情報を保持し、クラ (ブロック ストレージ) や WSV (World State View) の暗号化コードを介して、秘密情報のセキュリティを導入します。

Le meme depot construit Iroha 2 (reseaux auto-heberges) および Iroha 3 (SORA Nexus)。 Iroha 仮想マシン (IVM) とツールチェーン Kotodama の実行を保証し、バイトコードの制御と成果物を保持するポータブル エンタープライズ展開の自動ヘバージとグローバルな台帳を確認します。 Nexus。

目的
- 管理者とデータスペースの協力者による台帳論理のグローバル構成。
- データ スペースは、操作権限 (例: CBDC) を保持しており、DS を終了することはできません。
- Des Data Spaces は、アベック参加を超えて公開され、許可タイプのイーサリアムなしでアクセスします。
- データ スペース内のインテリジェント コンポーザブルを制御し、プライベート DS へのアクセス許可を明示的に提供します。
- プライベート DS 間のトランザクションのパフォーマンスを低下させるために、パブリックなアクティビティを分離します。
- Disponibilite des donnees a grande echelle: Kura et WSV avec effacement code pour supporter des donneeseffecments illimitees tout en gardant les donnees private-DS privees。

非目的語 (フェーズイニシャル)
- トークンの経済性と検証の動機を定義する。スケジューリングとプラグイン可能なステーキングの政治。
- 新しいバージョンの ABI の紹介;変更内容は、Ciblent ABI v1 の拡張機能のシステムコールとポインタの明示的な ABI セロン ポリシー IVM です。用語集
- Nexus 台帳: ブロックの構成要素であるデータ スペース (DS) のグローバル形式の台帳論理は、固有の履歴と関与のデータを保持します。
- データスペース (DS): 実行と在庫の管理は、妥当性検査、統治、機密事項、政治 DA、割り当てと財政の管理に基づいて行われます。二重クラスが存在します: パブリック DS とプライベート DS。
- プライベート データ スペース: アクセス許可とアクセス制御を検証します。 DS でトランザクションを終了し、終了します。エンゲージメント/メタドンはグローバル化を実現します。
- パブリック データ スペース: 許可なしでの参加。 les donnees は完了し、公開されます。
- データ スペース マニフェスト (DS マニフェスト): マニフェスト コード en Norito パラメータ DS (検証/クリア QC、機密事項、政治 ISI、パラメータ DA、保持、クォータ、政治 ZK、フランス語) を宣言します。 Le hash du manifest est ancre sur la chaine nexus。デフォルトの ML-DSA-87 (クラス Dilithium5) の署名スキーマを使用したクォーラム DS 証明書のオーバーライド。
- スペース ディレクトリ: ディレクトリ グローバル オンチェーン トレース ファイル マニフェスト DS、バージョンおよび管理/ローテーションを解決および監査に管理します。
- DSID: データスペースに注ぐ一意のグローバル識別子。オブジェクトや参照の名前空間を利用します。
- アンカー: ブロック/ヘッダー DS には、歴史的な DS au 台帳グローバルが含まれるエンゲージメント暗号化が含まれます。
- 蔵: Stockage de blocs Iroha。 Etendu ici avec Stockage de BLOB、削除コードおよび契約。
- WSV: Iroha ワールド ステート ビュー。バージョン、スナップショット、およびコードのセグメントの保存が行われます。
- IVM: Iroha 仮想マシンは、インテリジェントに対して実行を実行します (バイトコード Kotodama `.to`)。
  - AIR: 代数中間表現。 STARK 型のプルーブ計算を実行し、トランジションとフロンティアの制約条件を基にトレースを実行するための計算を実行します。

データスペースのモデル
- 識別: `DataSpaceId (DSID)` は、DS および名前空間を識別します。 Les DS peuvent etre instances a deux granularites:
  - Domain-DS: `ds::domain::<domain_name>` - 実行などのスコープはドメイン外です。
  - Asset-DS: `ds::asset::<domain_name>::<asset_name>` - 実行などの定義は一意です。
  Les deux 形式が共存します。 les トランザクション peuvent toucher plusieurs DSID de maniere atomice。
- サイクル・デ・ヴィ・デュ・マニフェスト: DS の作成、一週間の休暇 (ローテーション、政治の変更) およびスペース・ディレクトリーへの再登録。 Chaque アーティファクト DS パー スロット リファレンス ファイル、ハッシュ デュ マニフェスト ファイル、および最近。
- クラス: パブリック DS (参加開始、DA 公開) およびプライベート DS (許可、DA 機密)。政治はマニフェストとフラグを介して可能な限りハイブリッド化されます。
- DS による政治: 権限 ISI、パラメータ DA `(k,m)`、chiffrement、保持、クォータ (ブロックごとの TX の最小/最大部分)、Politique de preuve ZK/optimiste、frais。
- ガバナンス: メンバーのガバナンスおよび検証のローテーションは、マニフェストのガバナンス セクションで定義されます (オンチェーン、マルチシグ、トランザクション ネクサスおよび証明に関する外部管理の提案)。

容量と UAID のマニフェスト
- 宇宙の比較: 参加者は UAID 決定 (`UniversalAccountId` と `crates/iroha_data_model/src/nexus/manifest.rs`) でデータスペースを再計算します。 UAID のデータスペース固有のマニフェスト (`AssetPermissionManifest`) は、有効化/有効期限と許可/拒否規則のリストを示します `ManifestEntry` 生まれの `dataspace`, `program_id` `method`、`asset` および役割 AMX オプション。 Les regles は、gaggent toujours を否定します。評価対象 `ManifestVerdict::Denied` は、監査の存在意義を認められません。`Allowed` は、メタデータに対応する許可を与えられません。
- 許可: チャックエントリは、バケットの輸送を許可します。`AllowanceWindow` (`PerSlot`、`PerMinute`、`PerDay`) と `max_amount` オプションを決定します。ホストと SDK はミーム ペイロード Norito を構成しており、ハードウェアと SDK のアプリケーションは同一です。
- 監査テレメトリ: スペース ディレクトリ拡散 `SpaceDirectoryEvent::{ManifestActivated, ManifestExpired, ManifestRevoked}` (`crates/iroha_data_model/src/events/data/space_directory.rs`) のマニフェスト変更の確認。新しい表面 `SpaceDirectoryEventFilter` パーメット オー アボンヌ Torii/データ イベント モニターは、マニフェスト UAID を逃し、失効と意思決定を拒否し、配管を監視する必要はありません。エンドツーエンドの操作、SDK の移行に関するメモ、マニフェストの発行に関するチェックリスト、ユニバーサル アカウント ガイド (`docs/source/universal_accounts_guide.md`) のミロイテス セクションを参照してください。 Gardez les deux 文書は、UAID の変更に合わせて、政治と社会を一致させます。

オーニボーの建築
1) グローバル クーシュ ド コンポジション (Nexus チェーン)
- データ スペース (DS) の独自のブロック Nexus を 1 秒で完了するためのトランザクション アトミック クーブラントを維持します。 Chaque トランザクションのコミットが、世界状態のグローバル統一 (DS によるルートのベクトル) に適合しました。
- コンティエント・デ・メタドンネ・ミニマル・プラス・プリューブ/QC 集合体は、構成保証、ファイナライト、および不正行為の検出を保証します (DSID は、DS 前 / 後からのルーツ、エンゲージメント DA、DS による有効性の確認、および DS アベック認証 ML-DSA-87)。 Aucune donnee privee n'est が含まれます。
- コンセンサス: BFT グローバル パイプライン デ タイユ 22 (3f+1 avec f=7) を委員会し、エポックごとの VRF/ステークごとに、審査のプールから約 200,000 人の検証可能性を選択します。ネクサス シーケンスのトランザクションを実行し、ブロック全体をファイナライズします。

2) Couche データスペース (パブリック/プライベート)
- DS のトランザクション グローバルのフラグメントを実行し、ブロック Nexus で 1 秒間、DS のローカル WSV および有効なブロックの成果物の成果物 (DS の合意と契約 DA の準備) を実行します。
- Les Private DS の管理者による保管とトランジット エントリの検証が自動化されます。 DS の契約と有効な PQ 終了の準備をします。
- Les Public DS exportent les corps complets de donnees (DA 経由) および les preuves de validite PQ。

3) データスペースを越えたトランザクション アトミック (AMX)
- モデル: トランザクション ユーティリティ DS を利用します (例: ドメイン DS とプラス資産 DS)。ブロックごとにコミットするのは Nexus のユニークな内容です。オークン・エフェット・パルティエル。
- 1 秒間のコミット準備: トランザクション候補を注ぎ込み、DS はミーム スナップショットと並行して実行し (DS とスロットのデビュー)、有効な PQ パー DS (FASTPQ-ISI) を作成し、DA を作成します。 DS は検証と証明書の要求を要求し、DA は一時的に到着します (objectif <=300 ms)。スロットのトランザクションを再計画する必要があります。
- 一貫性: les ensembles 講義/エクリチュール ソントは宣言します。スロットのデビューのルートとコミットの競合を検出します。 DS によるロックなしでの実行の最適化は、グロボーの停止を防ぎます。コミット ネクサスの規則に基づいて、最小限の制限を課します (DS を実行する必要があります)。
- Confidentialite: プライベート DS の輸出先の独自性は、DS の事前/事後の補助的なものです。 Aucune donnee privee brute ne quitte le DS。

4) Disponibilite des donnees (DA) avec effacement コード
- ブロックとスナップショットの WSV コム デ ブロックの消去コードを保管します。ブロブは、シャードを大きくすることなく公開されます。ブロブはプライベート DS を独占し、ソント ストックを一意に管理し、プライベート DS、チャンク シフレの平均値を取得します。
- 技術者は、DS およびブロック Nexus に基づいて、人工物やブロック Nexus との契約を登録し、継続的に酒を飲むことなく、サンプリングおよび療養のための保証を永続的に行います。

ブロックとコミットの構造
- プレユーブ データ スペースのアーティファクト (スロット 1 ごと、DS ごと)
  - チャンピオン: dsid、slot、pre_state_root、post_state_root、ds_tx_set_hash、kura_da_commitment、wsv_da_commitment、manifest_hash、ds_qc (ML-DSA-87)、ds_validity_proof (FASTPQ-ISI)。
  - Les Private-DS は軍団なしで成果物を輸出します。 DA による回復を永続的に行うパブリック DS。

- ブロック Nexus (ケイデンス 1 秒)
  - チャンピオン: block_number、parent_hash、slot_time、tx_list (トランザクション アトミック、クロス DS アベック DSID タッチ)、ds_artifacts[]、nexus_qc。
  - 機能: トランザクションのアトミックではなくアーティファクトの確認を DS が要求することを確定します。私は、世界の状態を世界全体で見るために、ルーツのベクトルを取得するのに 1 時間かかりました。コンセンサスとスケジュール設定
- コンセンサス Nexus チェーン: BFT グローバル パイプライン (クラス Sumeragi) 22 のノードの平均 (3f+1 平均 f=7) ブロックの 1 秒と最終的な 1 秒を表示します。 VRF/ステークパルミを介したエポックごとの委員会のソントセレクションのメンバーは〜20万人の候補者。回転維持は分散化と非難による抵抗です。
- コンセンサス データ スペース: DS は、スロットごとに成果物を作成するための適切な BFT エントリの検証を実行します (プレビュー、エンゲージメント DA、DS QC)。レーン リレーの寸法は、`3f+1` のユーティリティ パラメータ `fault_tolerance` のデータスペースとエポックのプールの検証を決定し、`(dataspace_id, lane_id)` に設定されています。 Les Private DS の権限。公共DSは、反シビルの政治を永久に監視します。ル・コミテ・グローバル・ネクサス・レスト・インチェンジ。
- トランザクションのスケジュール設定: トランザクションの使用方法、アトミック宣言、DSID およびアンサンブルの講義/エクリチュールを管理します。 DS はスロットと並行して実行されます。トランザクションを含む、ブロック 1 秒間のファイル アーティファクト DS 検証および証明書 DA ソンタル時間 (<=300 ミリ秒)。
- パフォーマンスの分離: DS とメモリプールおよび実行を独立させます。 DS が生成するトランザクションのクォータは、DS が実行するブロックのコミットと、プライベート DS の遅延を監視する行頭ブロックを監視します。

ドニーと名前空間のモデル
- ID は DS に従って修飾されます。toutes les entites (ドメイン、コンテス、actif、ロール) は `dsid` に従って修飾されます。例: `ds::<domain>::account`、`ds::<domain>::asset#precision`。
- 参照グローバル: 参照グローバル、タプル `(dsid, object_id, version_hint)` およびオンチェーン ダン ラ クシュ ネクサスおよびダン デス記述子 AMX は、DS 間で使用されます。
- シリアル化 Norito: DS 間でのファイル メッセージ (記述子 AMX、preuves) はコーデック Norito を使用します。パ・ド・セルド・アン・プロダクション。

インテリジェントと拡張機能を対照 IVM
- 実行コンテキスト: ジョウター `dsid` 実行コンテキスト IVM。 Kotodama の実行者は、データ スペース特有の作業を行っています。
- DS 間のプリミティブ アトミック:
  - `amx_begin()` / `amx_commit()` の境界は、ホスト IVM のトランザクション アトミック マルチ DS です。
  - `amx_touch(dsid, key)` は、スロット間のスナップショットの競合を検出するための読み取り/書き込みの意図を宣言します。
  - `verify_space_proof(dsid, proof, statement)` -> ブール値
  - `use_asset_handle(handle, op, amount)` -> 結果 (政治的な自動化およびハンドルの有効な操作許可)
- アセットハンドルとフライ:
  - ISI の政治/DS の役割を自動化するための作戦行動。 DS のトークン デ ガスで支払​​います。機能トークンのオプションと政治に加えて、豊富な機能 (複数の承認者、レート制限、ジオフェンシング) が必要なだけでなく、変更のないモデルのアトミックも可能です。
- Determinisme: システムコールのシステムコールの純粋性と決定性、エントリ、アンサンブルの読み取り/書き込みを AMX が宣言します。環境の一時保存。Preuves de validite post-quantique (ISI 一般化)
- FASTPQ-ISI (PQ、信頼できるセットアップなし): 引数ハッシュ ベースで、ハードウェア クラスの GPU を使用した 20K のバッチを使用して、ファミリ ISI の設計転送を一般化します。
  - プロファイル操作:
    - `fastpq_prover::Prover::canonical` 経由で本番環境の構成要素をテストし、本番環境のバックエンドを初期化します。ル・モックは引退を決定します。 [crates/fastpq_prover/src/proof.rs:126]
    - `zk.fastpq.execution_mode` (構成) および `irohad --fastpq-execution-mode` は、CPU/GPU の実行を決定する永続的な補助オペレーターと、オブザーバーのフックを登録し、要求/解決策/バックエンドの監査をトリプルします。 [crates/iroha_config/src/parameters/user.rs:1357] [crates/irohad/src/main.rs:270] [crates/irohad/src/main.rs:2192] [crates/iroha_telemetry/src/metrics.rs:8887]
- 算術演算:
  - KV-Update AIR: WSV はマップのキーと値のタイプを特徴付け、Poseidon2-SMT を介して関与します。 Chaque ISI は、(コンプ、アクション、ロール、ドメイン、メタデータ、供給) 読み取り、チェック、書き込みのアン プチ アンサンブルを開催します。
  - オペコードごとの制約: ルール テーブル AIR の avec コロン セレクターは、ISI による規則を課します (保存、単調な計算、権限、範囲チェック、メタデータの負担の軽減)。
  - ルックアップの引数: テーブルの透明性は、パーミッション/ロール、ビットごとのルールの制約を制約する政治的証拠の精度と動作のパラメータを考慮します。
- 婚約と一週間の予定:
  - Preuve SMT 同意者: toutes les cles touchees (pre/post) Sont prouvees contre `old_root`/`new_root` en utilisant unfrontier compresse avec兄弟重複排除。
  - 不変条件: 不変条件 globaux (p. ex., Supply total par actif) は、eglite de multiensemble entre lignes d'effet et compteurs suivis を介して課されます。
- システム・ド・プリューブ:
  - エンゲージメントポリノミオースタイル FRI (DEEP-FRI) アベックフォルテアリテ (8/16) およびブローアップ 8-16。ハッシュポセイドン2;トランスクリプト Fiat-Shamir avec SHA-2/3。
  - 再帰オプション: 集約再帰ロケール DS は、スロットに必要なマイクロバッチのコンプレッサーを実行します。
- Portee と couverts の例:
  - Actifs: 転送、ミント、書き込み、アセット定義の登録/登録解除、精度の設定 (ボーン)、メタデータの設定。
  - コンテス/ドメイン: 作成/削除、キー/しきい値の設定、署名者の追加/削除 (一意性、DS の検証のための署名のチェック、AIR の検証)。
  - 役割/権限 (ISI): 役割と権限を付与/取り消します。テーブルのルックアップと政治的なチェックを介して単調に課します。
  - Contrats/AMX: マーキュールは AMX の開始/コミット、機能のミント/取り消しはアクティブになります。政治の変遷と政治の進歩を証明します。
- AIR の保存期間をチェックします:
  - 署名と暗号法 (例、ML-DSA を使用した署名) は、DS と認証者による DS QC の検証を行います。 la preuve de validite couvre seulement la coherence d'etat および la conformite de politique。 Cela garde des preuves PQ et Rapides。
- パフォーマンスの組み合わせ (例、CPU 32 コア + 最新の GPU):
  - 20k ISI ミックスの平均キータッチ プチ (<=8 クル/ISI): プルーフで ~0.4 ～ 0.9 秒、プルーフで ~150 ～ 450 KB、検証で ~5 ～ 15 ミリ秒。
  - ISI プラス ルルド (プラス デ クレ/コントレイント リッチ): マイクロバッチ (例、10x2k) + 再帰注入ガーダー <1 秒/スロット。
- DS マニフェストの構成:
  - `zk.policy = "fastpq_isi"`
  - `zk.hash = "poseidon2"`、`zk.fri = { blowup: 8|16, arity: 8|16 }`
  - `state.commitment = "smt_poseidon2"`
  - `zk.recursion = { none | local }`
  - `attestation.signatures_in_proof = false` (DS QC による署名検証者)
  - `attestation.qc_signature = "ml_dsa_87"` (デフォルト; 明示的な宣言を行う代替案)
- フォールバック:
  - ISI 複合体/個人は STARK 一般 (`zk.policy = "stark_fri_general"`) の avec preuve Differentee et Finalite 1 s via attestation QC + slashing sur preuves 無効です。
  - PQ 以外のオプション (例、Plonk avec KZG) 信頼できるセットアップとデフォルトのビルドのサポートが必要です。はじめに AIR (Nexus を注ぐ)
- 実行のトレース: 行列 aveclargeur (colonnes de registres) et longueur (etapes)。 ISI の論理を確立する必要があります。 les Colonnes contiennent valeurs の事前/事後、セレクターとフラグ。
- 制約:
  - 移行の制約: リーニュの関係を強制します (例、post_balance = pre_balance - 借方のリニュアル額 `sel_transfer = 1`)。
  - 限界限界: プレミア/デルニエール ラインのクライアント I/O パブリック (old_root/new_root、compteurs)。
  - ルックアップ/置換: 回路を介さずにテーブルの外観とマルチアンサンブルの合法性を保証します (許可、パラメータ)。
- エンゲージメントと検証:
  - 証明者は、暗号化ハッシュと多ノームの解釈を介してトレースを実行し、制限付きの妥当性を確認します。
  - 検証者は、FRI (ハッシュベース、量子化後) による avec quelques ouvertures Merkle を介して le faible degre を検証します。 le cout est logarithmique en etapes。
- 例 (転送): pre_balance、amount、post_balance、nonce および selecteurs を含むレジスタを登録します。 Les contraintes impent non-negativite/range、conservation et monotonicite de nonce、tandis qu'une multi-preuve SMT agregee lie les feuilles pre/post aux root old/new。

Evolution ABI とシステムコール (ABI v1)
- ajouter をシステムコールします (図解による命名):
  - `SYS_AMX_BEGIN`、`SYS_AMX_TOUCH`、`SYS_AMX_COMMIT`、`SYS_VERIFY_SPACE_PROOF`、`SYS_USE_ASSET_HANDLE`。
- タイプ ポインター - ABI ajouter:
  - `PointerType::DataSpaceId`、`PointerType::AmxDescriptor`、`PointerType::AssetHandle`、`PointerType::ProofBlob`。
- 1 時間の欠席には次のものが必要です。
  - Ajouter a `ivm::syscalls::abi_syscall_list()` (garder l'ordre)、gate par politique。
  - `VMError::UnknownSyscall` とホスト間のマッパーの数値。
  - 時間ごとのテストの測定: システムコール リスト ゴールデン、ABI ハッシュ、ポインタ タイプ ID ゴールデン、ポリシー テスト。
  - ドキュメント: `crates/ivm/docs/syscalls.md`、`status.md`、`roadmap.md`。

機密保持モデル
- 所有権の競合: トランザクション、差分データ、およびスナップショット WSV のプライベート DS は、完全に検証された権限を持っています。
- 公開公開: ヘッダー、契約書、および有効な PQ ソントのエクスポートに関する契約。
- Preuves ZK optionnelles: les Private DS peuvent produire des preuves ZK (p. ex., solde suffisant, politique satisfaite) permettant des action sans reveler l'etat interne クロスDS sans reveler l'etat interne。
- アクセス制御: ISI 政治/DS の役割に対する権限を課す。機能トークンはオプションとプベント、および必要なオプションと遅延を含みます。

パフォーマンスと QoS の分離
- コンセンサス、メンプール、およびストックは DS ごとに分離されます。
- DS のスケジューリング関連のクォータは、アンカーを含める時間を制限し、行頭ブロックを回避します。
- DS (コンピューティング/メモリ/IO) ごとのリソースと制御の予算。ホストごとに IVM を課します。パブリック DS は消費者向けの予算であるプライベート DS と競合します。
- プライベート DS の実行において、DS 間での非同期が長期間にわたって監視されていることが明らかであることを訴えます。

ドネと在庫のデザインの管理
1) 消火コード
- リードソロモン体系 (例、GF(2^16) など) を使用して、ブロックの消去コードを保存し、WSV のスナップショットを作成します。パラメータ `(k, m)` avec `n = k + m` シャード。
- デフォルトのパラメータ (提案、公開 DS): `k=32, m=16` (n=48)、永久剤ラ回復ジュスカ 16 シャード、平均 ~1.5 倍の拡張。プライベート DS: `k=16, m=8` (n=24) のアンサンブル許可を与えてください。 DS マニフェストに従って構成可能な詳細。
- BLOB パブリック: シャードは、nombreux noeuds DA/validateurs avec checks de disponibilite par サンプリングを介して配布されます。ライトクライアントに対しては、エンゲージメント DA とヘッダーが永続的に検証されます。
- BLOB の所有権: シャード chiffres と、プライベート DS (管理者が設計した) の検証を一意に配布します。 LA チェーン グローバル ネ ポート ケ デ エンゲージメント DA (シャード ニクルの設置なし)。

2) エンゲージメントとサンプリング
- チャックブロブを注ぐ: 計算機は、シャードなどのマークルを含め、`*_da_commitment` を計算します。 Rester PQ は、クールベ楕円形での必然的な関与を示します。
- DA 認証者: VRF ごとの地域の認証者 (例、64 地域あたり) ML-DSA-87 認証者がシャードのサンプリングを行っています。証明の遅延の目的 DA <=300 ミリ秒。シャードの有効な証明書を作成します。3) 統合蔵
- ブロック ストックエント レス コール デ トランザクション コム ブロック、削除コード、アベック エンゲージメント マークル。
- ヘッダーは BLOB との関わりを示唆します。 les corps Sont recuperables を介して le reseau DA を介してパブリック DS を注ぎ、des canaux prives を介してプライベート DS を注ぎます。

4) 統合WSV
- スナップショット WSV: 定期的、DS のスナップショット チャンクとコードの平均的なエンゲージメントをヘッダーに登録するチェックポイント。スナップショットと変更ログは維持されません。スナップショットは、大きなシャードではなく公開されます。スナップショットを保存し、検証を保存します。
- Access porteur de preuves: les contrats peuvent fournir (ou 需要者) des preuves d'etat (Merkle/Verkle) は、スナップショットの取引を許可します。 Les Private DS peuvent fournir des attestations ゼロ知識オー・リュー・ド・プリューヴ・ブルート。

5) 保持と枝刈り
- パブリック DS を注ぐパス ド プルーニング: コンサーバー トゥス レ コープス クラと DA (スケーラビライト ホリゾンタル) 経由の WSV スナップショット。 Les Private DS peuvent definir une retention interne、mais les Engagement は、retent immuables をエクスポートします。 La couche nexus は、ブロック Nexus と芸術品のエンゲージメント DS を保存します。

Reseau et role de noeuds
- グローバルな検証: 参加者とコンセンサスの関係、ブロック Nexus および成果物 DS、パブリック DS を注ぐ効果のあるチェック DA。
- Validateurs Data Space: 実行コンセンサス DS、実行コントラット、ゲレント クラ/WSV ローカル、ゲレント DA プール ルール DS。
- Noeuds DA (オプション): ブロブ公開の在庫/公開、サンプリングを容易にします。プライベート DS を注ぎ、noeuds DA は、安全性を検証し、管理者を協力してローカライズします。

改善と考慮事項システム
- シーケンス/メモリプールの切り離し: アダプタとメモリプール DAG (例、スタイル Narwhal) のアダプターと BFT パイプラインを使用して、遅延と遅延を軽減し、モデル ロジックを変更せずにスループットを向上させます。
- クォータ DS と公平性: DS ごとのブロックとキャップのクォータは、ラインヘッド ブロッキングを回避し、プライベート DS の予測可能な遅延を保証します。
- 認証 DS (PQ): 既定の ML-DSA-87 (classe Dilithium5) を利用するクォーラム DS 証明書。 EC は、量的およびボリュームの後の署名を、UN QC パー スロットとして受け入れられます。 ML-DSA-65/44 (プラス プチ) と署名を使用して、DS マニフェストで EC SI を宣言する必要があります。 les Public DS ソント強化は、ガーダー ML-DSA-87 を奨励します。
- DA 認証者: パブリック DS を提供し、VRF の証明書 DA に対する認証者地域の echantillonnes を利用します。サンプルのサンプルの証明書の有効性を確認します。プライベート DS 庭園、認証 DA インターン。
- エポックごとの再帰とプリューブ: スロット/エポックごとに再帰的に再帰的に実行される DS とプリューブのマイクロバッチのオプションのアグリガー プルシュール。
- レーンのスケーリング (必要な場合): グローバルで一意の逸脱を解除し、K レーンのシーケンスと並行してマージを決定します。 Cela は、世界的にユニークな水平性を強調しています。
- 加速の決定性: カーネルの SIMD/CUDA ゲートはフォールバック機能を備えた SIMD/CUDA ゲートにより、フォールバック CPU のビット精度の保持機能によりクロスハードウェア間で決定性を実現します。
- レーンの活性化 (提案): アクティブ 2-4 レーン si (a) la Finalite p95 depasse 1.2 秒ペンダント >3 分連続、ou (b) l'occupation par bloc depasse 85% ペンダント >5 分、ou (c) le debit entrant de tx requerrait >1.2x la capacite de bloc a des niveauxソウテンス。 DSID のハッシュ解析などを決定し、ブロック ネクサスをマージするトランザクションのバケット化を行います。

Frais et economie (価値のあるイニシャル)
- ガスの統合: DS の平均コンピューティング/IO メトリクスによるトークン デ ガス。 les frais Sont payes en actif de Gas natif du DS. La 変換は、適用可能な entre DS est une preoccupation です。
- 優先順位: 公平性と SLO 1 を維持するために DS ごとのラウンドロビンの平均クォータ。 DS では、前払い料金を支払う必要があります。
- 将来: 世界的なマルシェの手数料と政治のミニミザント MEV プヴァン エトルは、原子炉の設計を変更することなく探求します。データ空間を越えたワークフロー（例）
1) Un utilisateur soumet unetransaction AMX touchant un public DS P et un private DS S: deplacer l'actif X de S vers le beneficiaire B dont le compte est dans P.
2) スロットごとに、スロットのスナップショットを制御するペットの実行フラグメント。 S Verifie l'autorisation et la disponibilite、jourson etat interne、et produit une preuve de validite PQ et unengagement DA (aucune donnee privee ne fuite) を確認します。 P は、時事特派員 (例: ミント/バーン/ロッキング ダン P selon la politique) と sa preuve を準備します。
3) Le comite nexus verifie les deux preuves DS et les certificats DA;スロットのドゥ検証、ブロック Nexus でのトランザクションのコミット、1 時間のドゥ ルート DS の世界状態のグローバル状態の確認。
4) 証明書 DA は手動/無効であり、トランザクションの中止 (無効)、クライアントがスロットの要求を行う必要があります。 Aucune donnee privee ne quitte S a aucun moment。

- セキュリティ上の考慮事項
- 実行決定: システムコール IVM 残りの決定。 DS 間の結果は、時計やタイミングの研究を通じて、AMX コミットと最終決定に影響を与えます。
- アクセス制御: ISI およびプライベート DS の権限は、トランザクションおよび自動操作の制限を制限します。機能トークンは、DS 間での使用量をエンコードするデドロワ フィンです。
- Confidentialite: プライベート DS をエンドツーエンドで収集し、破片を破棄して、自動化されたメンバーの固有情報をストックし、証明書を外部に提供する ZK オプションを保持します。
- レジスタンス DoS: プライベート DS の進行状況に影響を与える混雑時の隔離/コンセンサス/ストック管理。

コンポーネントの変更 Iroha
- iroha_data_model: 導入 `DataSpaceId`、ID は DS、記述子 AMX (アンサンブル講義/エクリチュール)、タイプ ド プリューブ/エンゲージメント DA に該当します。シリアル化 Norito 固有。
- ivm: AMX (`amx_begin`、`amx_commit`、`amx_touch`) および preuves DA を含むポインター ABI のシステムコールとタイプの説明。 1 週間のテスト/ドキュメント ABI selon la politique v1。