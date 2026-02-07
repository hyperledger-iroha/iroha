---
lang: ja
direction: ltr
source: docs/portal/docs/nexus/nexus-spec.pt.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
ID: ネクサススペック
title: Especacao tecnica da Sora Nexus
説明: `docs/source/nexus.md` を完全に入力し、台帳 Iroha 3 (Sora Nexus) の設計規則としてアーキテクチャを作成しました。
---

:::note フォンテ カノニカ
エスタ・ページナ・エスペルハ`docs/source/nexus.md`。マンテンハは、ポータルの未処理の取引をコピアとして実行しました。
:::

#! Iroha 3 - Sora Nexus 元帳: 特定の技術設計

Nexus 台帳パラ Iroha 3、Iroha 2 パラメータ台帳のグローバルな論理統合をデータ スペース (DS) で組織するための文書を作成します。データ スペース fornecem dominios fortes de privacidade (「プライベート データ スペース」) e participacao aberta (「パブリック データ スペース」)。プライベート DS の秘密保持を保証する台帳の保存を設計し、Kura (ブロック ストレージ) や WSV (World State View) を介してプライベート DS の管理を開始します。

メスモ リポジトリ コンピラ タント Iroha 2 (自動ホスペダス) 量子 Iroha 3 (SORA Nexus)。 Iroha 仮想マシン (IVM) とツールチェーン Kotodama を比較し、バイトコードの永続的な運用管理を実行するための、デプロイメントの自動ホストと元帳のグローバル実行の実行Nexus。

オブジェクト
- 台帳ロジックは、データ スペースの有効性を検証するためのグローバル コンポストです。
- データ スペースは、オペラ座の権限 (例: CBDC) に基づいて非公開であり、DS のプライベート権限を持ちます。
- データ スペースは、イーサリアムに広く参加し、アクセスできるようになります。
- データ スペース全体を管理し、プライベート DS へのアクセスを明示的に許可します。
- プライベート DS の国際取引を劣化させるために、公開行為を行う必要があります。
- セキュリティの強化: WSV com のコードを参照して、サポート セキュリティの有効性を制限し、プライベート DS のプライバシーを維持します。

ナオ オブジェクト (偽イニシャル)
- トークンの経済性と有効性のインセンティブを定義します。政治的なスケジューリングとステーキングはプラグイン可能です。
- ABI の uma nova versao の紹介。 Mudancas ビザ ABI v1 com 拡張は、システムコールとポインター ABI を明示的に IVM のポリシーに準拠させます。

用語集
- Nexus 台帳: 台帳ロジックのグローバル フォーマットとデータ スペース (DS) のブロックを統合し、歴史を共有し、確立します。
- データスペース (DS): 独自の有効性を管理するための制限、行政管理、プライバシー保護、DA、割り当ておよび税金の制限。二重クラスが存在します: パブリック DS とプライベート DS。
- プライベート データ スペース: アクセス権の有効性と管理。 DS を使ってトランザクションを実行します。妥協/メタダドスをグローバルに実現します。
- パブリック データ スペース: 許可された参加者。完全な情報と公開情報を保存します。
- データ スペース マニフェスト (DS マニフェスト): マニフェスト コード em Norito que declara parametros DS (validadores/chaves QC、classe de privacidade、politica ISI、parametros DA、retencao、クォータ、politica ZK、taxas)。ああ、アンコラド・ナ・カディア・ネクサスを明らかにしてください。サルボ オーバーライド、クォーラム DS の認証 ML-DSA-87 (クラス ダイリチウム 5) は、ポスト量子またはパドラオの機能を備えています。
- スペース ディレクトリ: グローバル オンチェーン クエスト ラストレイア マニフェスト DS のコントラート デ ディレトリオ、統治/政府関連イベント、および公聴会でのイベントのバージョン。
- DSID: データスペースをグローバルに識別します。オブジェクトと参照の名前空間を使用します。
- アンカー: ブロック/ヘッダー DS には、DS 青台帳のグローバルな歴史が含まれています。
- 蔵: アルマゼナメント デ ブロコス Iroha。セキュリティとセキュリティを強化し、ブロブのコードを作成し、妥協します。
- WSV: Iroha ワールド ステート ビュー。バージョン管理、スナップショット、およびアプリケーションのコードを保存できます。
- IVM: Iroha 仮想マシンは高度な制御を実行します (バイトコード Kotodama `.to`)。
  - AIR: 代数中間表現。 STARK の専門家による計算の計算、および国境管理の実行に関する記述。データスペースのモデル
- ID: `DataSpaceId (DSID)` は、DS と tudo の名前空間を識別します。 DS の詳細なインスタンスの実行:
  - ドメイン DS: `ds::domain::<domain_name>` - ドメインの区切りを実行します。
  - Asset-DS: `ds::asset::<domain_name>::<asset_name>` - ユニカの定義を実行します。
  アンバとフォルマは共存します。 transacoes podem tocar multiplos DSID de forma atomica。
- Ciclo de vida do manage: criacao de DS、atualizacoes (rotacao de chaves、mudancas de politica) e aposentadoria sao registradas no Space Directory。 DS のスロット参照情報は、最近のマニフェストのハッシュに基づいて作成されています。
- クラス: パブリック DS (participacao aberta、DA publica)、プライベート DS (許可、DA 機密)。政治的政治は旗を通じて明らかになります。
- DS の政治: ISI の許可、パラメータ DA `(k,m)`、暗号文、retencao、クォータ (ブロックごとの最小/最大の参加)、ZK/オティミスタの政治、分類。
- ガバナンカ: メンバーシップ DS と有効性を明確に定義したガバナンカ マニフェスト (プロポスト オンチェーン、マルチシグ ウ ガバナンカ エクステルナ アンコラーダ ポート トランスアコエス ネクサス エ アテスタコエス)。

UAID の容量マニフェスト
- 普遍性: 参加者が UAID 決定性 (`UniversalAccountId` または `crates/iroha_data_model/src/nexus/manifest.rs`) を受信して、todos OS データスペースを変更できます。容量マニフェスト (`AssetPermissionManifest`) ビンキュラム UAID およびデータスペース固有、有効/期限切れの規則の許可/拒否 `ManifestEntry` 区切り `dataspace`、 `program_id`、`method`、`asset` の役割は AMX オプションです。レグラスはセンペル・ガナムを否定。 o avaliador は、`ManifestVerdict::Denied` com uma razao de audiotoria を許可し、`Allowed` com でメタデータを許可します。
- 許可: `AllowanceWindow` (`PerSlot`、`PerMinute`、`PerDay`) は、`max_amount` を省略可能です。ホストと SDK はペイロード Norito を構成し、ハードウェアと SDK の永続的な識別をアプリケーションで実行します。
- Telemetria de audio: Space Directory が `SpaceDirectoryEvent::{ManifestActivated, ManifestExpired, ManifestRevoked}` (`crates/iroha_data_model/src/events/data/space_directory.rs`) を送信してマニフェストを送信します。新しい権限 `SpaceDirectoryEventFilter` は、Torii/データ イベント モニターのマニフェスト UAID の設定を許可し、拒否と認証の拒否を許可します。

エンドツーエンドの運用に関する証拠、SDK の移行に関する注意事項、マニフェストの公開チェックリスト、ユニバーサル アカウント ガイド (`docs/source/universal_accounts_guide.md`) の安全性を確認します。マンテンハは、UAID ムダレムとしての政治と文書を保持しています。

アルト ニベル建築
1) グローバル構成機器 (Nexus チェーン)
- データ スペース (DS) を管理し、ブロック Nexus で 1 つのセグンド クエリの最終処理を実行します。 Cada transacao は、世界国家のグローバル統一 (DS のベクトル) を実現することを約束しました。
- 保証コンポサビリダードの最小メタデータ/QC 集合体、最終的な不正行為の検出 (DSID のトークン、DS の事前認証のルート、DS の認証の認証、DS の証明書の認証、ML-DSA-87) を確認します。ネンフム ダド プライベート エ インクルード。
- コンセンサス: BFT グローバル EM パイプライン デ タマンホ 22 (3f+1 com f=7) をコミテ、VRF/ステークのメカニスモとステークで最大 200,000 の有効性を確認できるプールの選択。 1 秒以内にトランザクションを実行し、最終的な処理を実行してください。

2) データスペースのカメラ (パブリック/プライベート)
- グローバルな DS のフラグメントを実行し、WSV ローカルで DS とブロックの検証に関する成果物を作成します (DS の集合体と妥協 DA のプロバス) は、ブロック Nexus で累積したデータを保持しません。
- プライベート DS の暗号文やレポート、トランジット エントレ バリデーレス オートリザドス。 DS を使用して PQ を検証するための妥協と検証を行います。
- パブリック DS の完全なデータのエクスポート (DA 経由) と PQ の検証。3) Transacoes atomicas クロスデータスペース (AMX)
- モデル: 複数の DS を転送するためのデータ (例、ドメイン DS は資産 DS を使用します)。エラはアトミカメント エミューム ユニコ ブロコ Nexus を中止しました。ナオ・ハ・エフェイトス・パルシアイス。
- Prepare-Commit dentro de 1s: パラカダトランザクション候補データ、DS トロイの実行、およびパラレロコントラオメスモスナップショット (ルート DS の最初のスロットなし)、DS または DS (FASTPQ-ISI) の PQ 検証のプロデューゼム、検証、および妥協 DA。 O Comite nexus commita a transacao apenas se todas as provas DS exigidas verificarem eos certificados DA chegarem a Temp (alvo <=300 ms);カソ・コントラリオ、近くのスロットでのトランザクションと再プログラム。
- Consistencia: conjuntos de leitura/escrita sao declarados;コンフリクトを検出して、コミットなしでルートをスロットに挿入します。 DS evita の sem ロックを実行すると、グローバルが失速します。アトミシダーデとコミット ネクサスの再評価 (tudo ou nada entre DS)。
- Privacidade: プライベート DS エクスポート、DS プレ/ポストのプライベート DS エクスポート/コンプロミス ヴィンキュラドス AOS ルート。ネンフム ダド プリバド クリュ サイ ド DS。

4) ディスポニビリダーデ デ ダドス (DA) com codificacao de apagamento
- ブロックのブロックとスナップショット WSV のブロックのコードを保存します。 Blobs publicos sao amplamente shardados; blob privados sao armazenados apenas em validadores private-DS、com chunks criptografados。
- コンプロミッソス DA は、タント エム アートファトス DS 量子 EM ブロコス Nexus を保持しており、可能性のあるアモストラジェムと回復性の保証は、詳細なコンテウド プリバードを明らかにします。

コミットの開始
- データ スペースの提供 (スロット 1 秒、DS ごと)
  - Campos: dsid、slot、pre_state_root、post_state_root、ds_tx_set_hash、kura_da_commitment、wsv_da_commitment、manifest_hash、ds_qc (ML-DSA-87)、ds_validity_proof (FASTPQ-ISI)。
  - Private-DS は、主要な資料をエクスポートします。公的 DS は DA を通じて身体回復を許可します。

- Nexus ブロック (カデンシア デ 1 秒)
  - Campos: block_number、parent_hash、slot_time、tx_list (transacoes atomicas、cross-DS com DSIDs tocados)、ds_artifacts[]、nexus_qc。
  - 機能: トランザクション アトミカス キュホス アルティファト DS 要求として検証を終了します。 DS は、世界の状態をグローバルに認識し、ルーツを確立します。

コンセンサスとスケジューリング
- Nexus チェーンのコンセンサス: BFT グローバル EM パイプライン (クラス Sumeragi) 22 のコミット (3f+1 com f=7) 1 秒のミランド ブロックと 1 秒の最終。メンバーは、VRF/ステークエントリーを介して、約 200,000 人の候補者がエポカの選択を行っています。ロタカオマンテムデスセントラルリザカオと抵抗と検閲。
- データ スペースの合意: スロットごとに独自の BFT を実行できるかどうかを確認します (プロバス、コンプロミッソ DA、DS QC)。レーン リレーの寸法 `3f+1` を使用して `fault_tolerance` を実行し、データスペースとシード VRF リガダの `(dataspace_id, lane_id)` を使用してプールの有効性を決定します。プライベート DS SAO 権限。公共 DS は、反シビルの政治的活動を許可します。おお、地球規模の永久的つながりを実現する委員会よ。
- トランザクションのスケジューリング: トランザクションのアトミカス宣言と DSID の詳細および説明/解説を使用します。 DS はスロットをパラレロ デントロで実行します。 o コミテ ネクサスには、トランザクションのブロックが 1 秒間実行され、DS 検証と認証が行われます (<=300 ミリ秒)。
- Isolamento de desempenho: DS チームのメンバーと独立した執行者を支援します。 DS のクォータは、プライベート DS の遅延を保護するために、ブロックおよびエビターの行頭ブロッキングによってコミットされた DS ポデム サーバーの制限量とトランザクション数を制限します。

名前空間のモデル化
- DS の ID 資格: エンティダードとしての todas (ドミニオ、コンタス、アティボス、ロール) `dsid` の資格。例: `ds::<domain>::account`、`ds::<domain>::asset#precision`。
- 参照グローバル: グローバル参照グローバル e uma tupla `(dsid, object_id, version_hint)` 電子ポデ サー コロカダ オンチェーン ナ カメラ ネクサス および説明 AMX パラ USO クロス DS。
- シリアル番号 Norito: クロス DS (記述子 AMX、プロバス) としてのメンサーゲン コーデック Norito。生産性を向上させます。高度なインテリジェントと拡張性 IVM
- 実行コンテキスト: 追加 `dsid` および実行コンテキスト IVM。 Contratos Kotodama は、特定のデータ スペースを実行します。
- Primitivas atomicas クロス DS:
  - `amx_begin()` / `amx_commit()` 区切り値 uma transacao atomica multi-DS ホストなし IVM。
  - `amx_touch(dsid, key)` スロットのスロットのスナップショットを宣言します。
  - `verify_space_proof(dsid, proof, statement)` -> ブール値
  - `use_asset_handle(handle, op, amount)` -> 結果 (有効な政治許可ハンドルの操作)
- アセットは分類群を処理します:
  - 政治活動に関する政治活動 ISI/DS の役割。タクサス・サン・パガスのトークン・デ・ガスはDSを行いません。ケイパビリティ トークンは、主要な政治 (複数の承認者、レート制限、ジオフェンシング) を管理し、アトミコのモデルを管理します。
- 決定論: 新しいシステムコールとしての決定と、AMX 宣言の統合および統合としての決定性。テンポや雰囲気を楽しむことができます。ポスト量子検証の証明 (ISI 一般)
- FASTPQ-ISI (PQ、信頼できるセムセットアップ): 引数、ハッシュベースの一般的な設計、ファミリア ISI エンクアントミラプロバサブセグンドパラロット、エスカラ 20k、ハードウェアクラス GPU としてのデータ転送設計。
  - Perfil 操作:
    - `fastpq_prover::Prover::canonical` を介した生産証明者、生産バックエンドの初期設定;モックデタミニスティックフォイリモビデオ。 [crates/fastpq_prover/src/proof.rs:126]
    - `zk.fastpq.execution_mode` (構成) `irohad --fastpq-execution-mode` は、CPU/GPU の形式決定性を決定するための実行を許可し、オブザーバー フック レジストラ トリプラス ソリシタダ/リソルビダ/バックエンド パラオーディオを実行します。 [crates/iroha_config/src/parameters/user.rs:1357] [crates/irohad/src/main.rs:270] [crates/irohad/src/main.rs:2192] [crates/iroha_telemetry/src/metrics.rs:8887]
- アリトメティザカオ:
  - KV-Update AIR: Poseidon2-SMT 経由で WSV のマップとキー値の情報を共有します。 ISI は、読み取り、チェック、書き込みの基本的な機能 (コンタス、アティボス、ロール、ドミニオ、メタデータ、サプライ) を拡張します。
  - オペコードの制限: ISI での AIR COM のセレトラの制限 (保存、単調な制御、許可、範囲チェック、メタデータ制限の設定)。
  - 検索の引数: ハッシュ パラメタの許可/役割、正確なアティボスと政治的セキュリティ パラメータのビット単位の制限を透明にします。
- 妥協と確立:
  - Prova SMT アグレガダ: チャベス トカダス (事前/後) としてのトダス、`old_root`/`new_root` とフロンティア コンプリミド コム兄弟の重複排除。
  - 不変量: グローバルな不変量 (例、合計供給量) sao impostas via igualdade de multiconjuntos entre linhas de efeito e contadores rastreados。
- システム・デ・プローバ:
  - FRI (DEEP-FRI) の妥協策 (8/16)、8 対 16 の接戦。ハッシュポセイドン2;トランスクリプト Fiat-Shamir com SHA-2/3。
  - オプションの再帰: 必要に応じて、マイクロ ロットのマイクロ ロットを確認して、ローカル DS を再帰的に再帰します。
- Escopo e exemplos cobertos:
  - Ativos: 転送、ミント、書き込み、アセット定義の登録/登録解除、精度の設定 (制限)、メタデータの設定。
  - Contas/Dominios: 作成/削除、キー/しきい値の設定、署名者の追加/削除 (apenas estado; verificacoes de assinatura sao atestadas por validadores DS、nao provadas dentro do AIR)。
  - 役割/権限 (ISI): 役割と権限を付与/取り消します。検索と政治チェックの偽装は単調です。
  - Contratos/AMX: マルカドールは AMX を開始/コミットし、機能のミント/無効化を開始します。政治活動と政治活動の推進。
- AIR パラプリザーバーのレイテンシアをチェックします。
  - Assinaturas e criptografia pesada (例、assinaturas ML-DSA de usuario) DS QC なしの検証済み DS と認証。政治の一貫性と適合性を検証するための法的根拠。 PQ と Rapidas をすべて実行します。
- Metas de desempenho (イラスト、CPU 32 コア + uma GPU moderna):
  - 20,000 ISI のキータッチ ペケノ コム (<=8 チャベス/ISI): 検証時間は ~0.4 ～ 0.9 秒、検証時間は ~150 ～ 450 KB、検証時間は ~5 ～ 15 ミリ秒。
  - ISIs mais pesadas (mais chaves/contrastes ricos): マイクロロット (例、10x2k) + recursao para manter <1 秒、スロットごと。
- DS マニフェストの設定:
  - `zk.policy = "fastpq_isi"`
  - `zk.hash = "poseidon2"`、`zk.fri = { blowup: 8|16, arity: 8|16 }`
  - `state.commitment = "smt_poseidon2"`
  - `zk.recursion = { none | local }`
  - `attestation.signatures_in_proof = false` (DS QC による検証)
  - `attestation.qc_signature = "ml_dsa_87"` (パドラオ; 明示的な宣言を行う代替案)
- フォールバック:
  - ISI は、STARK geral (`zk.policy = "stark_fri_general"`) を使用して複雑な/個人的な情報を提供し、atestacao QC 経由で 1 秒間保証され、無効な場合は無効になります。
  - Opcoes nao PQ (ex.、Plonk com KZG) exigem Trusted setup e nao sao mais supportadas no build Padrao.ao AIR の紹介 (パラ Nexus)
- Traco de execucao: matriz com largura (colunas de registros) と comprimento (passos)。 ISI を処理するためのロジックを制御します。コルナス アルマゼナムの前後の値、セレトレとフラグ。
- リストリコ:
  - Restricoes de transicao: impoem relacoes de linha a linha (例、post_balance = pre_balance - amount para uma linha de debito quando `sel_transfer = 1`)。
  - Restricoes de Fronteira: primiras/ultimas linhas としてのリガム I/O publico (old_root/new_root、contadores)。
  - 検索/並べ替え: メンバーシップを保証し、複数の接続を管理するためのコンプロメティダ (許可、パラメータ設定) のセキュリティ回路を確保します。
- 妥協と検証:
  - 満足度を保証するための認証コードのハッシュとポリノミオス デ バイクソ グラウ ケ サオ バリドスを介してトレースを侵害しないことを証明します。
  - FRI (ハッシュベース、ポスト量子) com poucas aberturas Merkle 経由の検証者 checa baixo grau。 o custo e logaritmico nos passos。
- 例 (転送): レジストリには、pre_balance、amount、post_balance、nonce および seletore が含まれます。制限は、古い/新しいルーツの前後にある SMT アグレガダ リーグの形式を制限します。

ABI システムコールの進化 (ABI v1)
- adicionar のシステムコール (イラスト名):
  - `SYS_AMX_BEGIN`、`SYS_AMX_TOUCH`、`SYS_AMX_COMMIT`、`SYS_VERIFY_SPACE_PROOF`、`SYS_USE_ASSET_HANDLE`。
- Tipos ポインター - ABI の追加:
  - `PointerType::DataSpaceId`、`PointerType::AmxDescriptor`、`PointerType::AssetHandle`、`PointerType::ProofBlob`。
- 必要な条件:
  - `ivm::syscalls::abi_syscall_list()` (manter ordenacao)、ゲート ポル ポリティカに追加してください。
  - `VMError::UnknownSyscall` 番号ホストの数をマップします。
  - Atualizar テスト: システムコール リスト ゴールデン、ABI ハッシュ、ポインタ タイプ ID ゴールデンおよびポリシー テスト。
  - ドキュメント: `crates/ivm/docs/syscalls.md`、`status.md`、`roadmap.md`。

プライバシーモデル
- プライベート コンテンツ: トランザクションの内容、差分、スナップショット、WSV のプライベート DS ヌンカ デイクサム、サブコンジュントのプライベート バリデーレス。
- 公開公開: ヘッダーの公開、DA の検証、PQ のエクスポートの保証。
- Provas ZK opcionais: プライベート DS podem produzir provas ZK (ex., saldo suficiente, politicaSatisfeita) は、クロス DS のセキュリティに関する詳細を確認します。
- アクセス制御: ISI/政治の不正行為を制御し、DS を実行する役割を果たします。ケイパビリティ トークンは、デポジットの導入に必要な情報を提供します。

Isolamento de desempenho と QoS
- コンセンサス、メンプールと DS の分離戦略。
- DS パラメータのスケジューリング関連のクォータは、アンカーとエビターの行頭ブロックを含むテンポを制限します。
- DS (コンピューティング/メモリ/IO) の再帰的操作、ホスト IVM の偽装。パブリック DS を使用してプライベート DS を使用することもできます。
- Chamadas クロス DS は、プライベート DS を実行するためのエスペラス シンクロンです。

ダドスのディスポニビリダーデとアルマゼナメントのデザイン
1) アパガメントの文書
- Usar Reed-Solomon のシステム (例、GF(2^16)) のパラコード アパガメントとブロブのスナップショット WSV: parametros `(k, m)` com `n = k + m` シャード。
- パラメトロス パドラオ (プロポスト、パブリック DS): `k=32, m=16` (n=48)、16 個のシャードを最大 1.5 倍の拡張で取得することを許可します。プライベート DS: `k=16, m=8` (n=24) は許可を得る必要があります。 Ambos は DS マニフェストによって構成されます。
- Blobs publicos: シャードは、管理番号 DA/validadores com checagens de disponibilidade por amostragem で配布されます。 Compromissos DA nos ヘッダーは、ライト クライアントの検証を許可します。
- Blobs privados: シャード暗号化電子配布およびプライベート DS の有効性管理 (管理者による管理)。 cadeia global carrega apenas compromisos DA (sem localizacoes de shards ou Chaves)。

2) 妥協と戦略
- パラ cada blob: 計算上の uma raiz Merkle sobre shards e inclui-la em `*_da_commitment`。マンター PQ のエビタンド コンプロミソス デ カーブ エリプティカ。
- DA 認証者: 認証者は VRF の地域 (例、64 地域) で証明書 ML-DSA-87 を発行し、シャードの認証を取得します。アテスタカオのメタ遅延時間 DA <=300 ミリ秒。バスカー シャードの証明書を検証するためのコミュニティを作成してください。

3) インテグラカオ・コム・クラ
- Blocos armazenam corpos de transacao como blob codificados com apagamento e compromissos Merkle。
- BLOB の妥協点のヘッダー カレガム。パブリックDSの場合はREDE DAを介して回復し、プライベートDSではCanais privadosを介して回復します。4) インテグラカオ コム WSV
- スナップショット WSV: DS のスナップショットのチャンク化されたチェックポイントを定期的に実行し、ヘッダーの登録を管理します。全体のスナップショット、管理変更ログ。スナップショットはシャードを公開します。スナップショットは永続的に有効なプライバシーを保持します。
- Acesso com prova: contratos podem fornecer (ou solicitar) provas de estado (Merkle/Verkle) ancoradas por compromisos de snapshot.プライベート DS のポデムは、ゼロ知識でプロバス クルアスを実現します。

5) 枝刈り
- パブリック DS からの枝刈り: DA 経由でスナップショット WSV を取得します (水平展開)。プライベート DS のポデムは、国際的な情報を定義し、永久に安全な輸出を保証します。互換性のあるネットワークは、Nexus と DS の妥協点を保持します。

レデ・エ・パペイス・デ・ノス
- Validadores globais: 合意関係への参加、validam blocos Nexus e artefatos DS、realizam checagens DA para public DS。
- データスペースの有効性: executam consenso DS、executam contratos、gerenciam Kura/WSV local、lidam com DA para seu DS。
- Nos DA (オプション): armazenam/publicam blob publicos、facilitam amostragem。プライベート DS では、共同ローカル情報の有効性を確認し、情報を管理します。

メルホリアスとシステムの検討
- シーケンシング/メモリプールの設計: DAG およびメモリプール (例: Narwhal など) のサポートと BFT およびパイプライン、カメラとの接続、レドゥジル レイテンシア、およびモデル ロジックの詳細なスループット。
- クォータ DS と公平性: DS のクォータとブロコのクォータ、およびエビタルのヘッドオブライン ブロッキングとプライベート DS の事前保証期間の制限。
- Atestacao DS (PQ): パドラオによる定足数 DS 認証 ML-DSA-87 (classe Dilithium5)。ポスト量子電子は、EC の主要な機能であり、スロットごとに QC の情報を共有します。 DS のポデムは、ML-DSA-65/44 (メノレス) を使用して、DS マニフェストなしで EC 宣言を明示的に実行します。パブリック DS sao fortemente encorajados a manter ML-DSA-87。
- DA 認証者: パラパブリック DS、USAR 認証者は、VRF のアモストラドス地域で発行証明書 DA を発行します。シャードの管理に関する証明書の有効性を確認してください。プライベート DS マンテム アスタコス DA インテルナス。
- Recursao e provas por epoca: opcionalmente agregar varios micro-lotes dentro deum DS em uma prova recursiva por slot/epoca para manter tamanho de prova etemp de verificacao estaveis sob alta carga.
- レーンのエスカロナメント (必要性): ユニコ コミテのグローバル ウイルス ガルガロの説明、決定的なマージの順序に関するレーンの紹介。 Isso preserva uma unica ordem global enquanto escala horizo​​ntalmente。
- Aceleracao deterministica: fornecer カーネル SIMD/CUDA com 機能フラグ、パラ ハッシュ/FFT com フォールバック、CPU ビット Exato パラ プリザーバー、クロスハードウェアの確定性。
- レーン制限 (プロポスト): ハビリタール 2 ～ 4 レーン SE (a) 連続 3 分で決勝 p95 が 1.2 秒を超え、(b) 連続 3 分で 1.2 秒を超え、5 分以上で 85% を超え、(c) TX エクイギリア分類群が 1.2 倍を超えるニベイス・スステンタドスの容量。レーンは、DSID のハッシュの形式を決定するためのバケットをトランザクションとして結合し、ブロック ネクサスをマージしません。

分類と経済 (パドロエス・イニシアイス)
- ガスのユニデード: DS com compute/IO medido のトークン デ ガス。タクサス・サン・パガスはDSを行いません。 DS と責任を持って話し合ってください。
- 優先順位: ラウンドロビンでの DS COM クォータと DS パラプリザーバーの公平性および SLO の 1 秒。 DS のデントロ、手数料入札のデセンパタール。
- Futuro: 世界的な税金と政治の市場を最小限に抑え、MEV の研究開発を進め、原子爆弾を開発し、PQ を設計します。Fluxo クロスデータスペース (例)
1) Um usuario envia uma transacao AMX tocando public DS P e private DS S: mover o ativo X de S para o beneficiario B cuja conta esta em P.
2) Dentro スロット、P e S executam seu フラグメント コントラ スナップショット スロット。自動検証とディスポニビリダーデの検証、国際的なプロデューサと検証のプロデューサ、および妥協 DA (nenhumdado privado vaza) を確認します。 P は、政治に準拠した通信を準備します (例: ミント/バーン/ロック、政治に準拠)。
3) nexus verifica ambas を provas DS および certificados DA としてコミットします。スロットを確認し、承認されたトランザクションは、ブロコ Nexus でアトミカメントをコミットし、1 秒で、アトゥアリザンド アンボス ルート DS は世界状態グローバルのベクトルを持ちません。
4) ファルタンド/無効な DA 認証を取得し、近くのスロットでクライアント ポーデを再開するためのトランザクション アボート (セミフェイトス) を実行します。ネンフム ダド プリバド サイ デ S em nenhum passo。

- セグランカを考慮してください
- 決定性の実行: システムコール IVM 永続的な決定性。クロス DS のサングリアのコミット AMX と最終処理の結果、レロジオとタイミングの確認。
- アクセス制御: ISI およびプライベート DS の制限時間内でのサブメーターのトランザクションおよびオペラ座の許可を許可します。 DS 間で使用できる能力トークンのコードフィックス フィノス。
- 機密情報: プライベート DS のエンドツーエンドの暗号化、外部の ZK オプシオナイ、および外部の ZK オプションの情報を含むシャード コードの情報。
- DoS の解除: プライベート DS の混雑を妨げ、公共の場に影響を与えます。

ムダンカスのコンポーネント Iroha
- iroha_data_model: `DataSpaceId` の紹介、DS の資格 ID、説明 AMX (conjuntos leitura/escrita)、tipos de prova/compromisos DA。シリアル番号 Norito。
- ivm: 追加のシステムコール、AMX のヒント ポインター - ABI (`amx_begin`、`amx_commit`、`amx_touch`)、および provas DA。 atualizar testes/docs ABI 準拠ポリティカ v1。