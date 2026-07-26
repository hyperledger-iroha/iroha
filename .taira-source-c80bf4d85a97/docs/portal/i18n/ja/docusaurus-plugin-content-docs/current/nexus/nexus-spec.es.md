---
lang: ja
direction: ltr
source: docs/portal/docs/nexus/nexus-spec.es.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
ID: ネクサス仕様
title: Especacion tecnica de Sora Nexus
説明: `docs/source/nexus.md` の完全性、Iroha 3 (Sora Nexus) に関する建築物の制限に関する制限。
---

:::ノート フエンテ カノニカ
エスタページナリフレジャ`docs/source/nexus.md`。ポータルの取引関連のバックログをすべて管理します。
:::

#! Iroha 3 - Sora Nexus 元帳: Especacion tecnica de diseno

Nexus 台帳の Iroha 3、展開 Iroha 2 は、台帳のグローバルな論理的統合をデータ スペース (DS) で管理します。データ スペースは、dominios fuertes de privacidad (「プライベート データ スペース」) と abierta (「パブリック データ スペース」) の参加を証明します。プライベート DS の秘密情報を保護し、プライベート DS の秘密情報を管理し、クラ (ブロック ストレージ) と WSV (World State View) を介してデータを管理します。

ミスモ リポジトリ コンピラ タント Iroha 2 (レデス オートアロハダ) は、Iroha 3 (SORA Nexus) です。 Iroha 仮想マシン (IVM) と Kotodama のツールチェーンの比較、バイトコード永続ポータブル エントリの自動アロハドスと電子帳票の制御の実行グローバルデ Nexus。

オブジェクト
- 台帳ロジックではなく、データ スペースの検証に協力するためのグローバルな計算を行います。
- データ スペースは、操作に関する許可 (p. ej.、CBDC)、DS のプライバシーを保護します。
- データ スペースは、イーサリアムに対して公開され、許可されています。
- データ スペース内のインテリジェントなコンポーザブルを管理し、プライベート DS のアクティビティに応じて明示的に許可します。
- プライベート DS 間の国際取引を劣化させることなく、公開活動を行うことができます。
- データの管理: プライベート DS のプライバシーを制限するために、WSV のコードを作成し、有効なデータを保持します。

オブジェクトなし (イニシャルのイニシャル)
- トークンの経済性と有効性のインセンティブを定義する。スケジューリングと息子の賭け事の政治。
- ABI の新しいバージョンの紹介;ロス カンビオスは、ABI v1 の拡張機能を明示的にシステムコールとポインタに割り当て、ABI は IVM の政治政策を確立しました。用語集
- Nexus 台帳: データ スペース (DS) のグローバル形式の台帳ロジックとコンポーネント ブロックを歴史的に管理し、侵害を防ぎます。
- データスペース (DS): ドミニオ・アコタド・デ・エジェクシオンとアルマセナミエント・コンサス・プロピオス・バリダドレス、ゴベルナンザ、秘密保護法、政治的DA、州とタリファスの政治。 dos クラスが存在します: パブリック DS とプライベート DS。
- プライベート データ スペース: Validadores はアクセス権を管理します。 DS でのトランザクションとヌンカ サレン デルのデータの保存。ソロでの妥協/メタデータのグローバル化。
- パブリック データ スペース: 許可された参加。ロス・ダトス・コンプリートトスとエル・エスタド・ソン・パブリックス。
- データ スペース マニフェスト (DS マニフェスト): Norito パラメータ パラメトロス DS に関するマニフェスト コード (validadores/llaves QC、clase de privacidad、politica ISI、parametros DA、retencion、cuotas、politica ZK、tarifas)。カデナ ネクサスとのマニフェストをハッシュします。 ML-DSA-87 (ダイリチウム 5 分類) の安全性を保証するため、欠陥後の安全性を確認するために、DS 認証を取得します。
- スペース ディレクトリ: グローバル オンチェーン クエリ ラストレア マニフェスト DS、バージョン、イベント、解決策、および聴覚に関するディレクトリのコントラト。
- DSID: データスペースのグローバルな識別子。オブジェクトや参照の名前空間を使用します。
- アンカー: DS のブロック/ヘッダー DS には、歴史と歴史の関連性を含む DS の元帳グローバルが含まれます。
- クラ: Almacenamiento de bloques de Iroha。不正な侵入を防止するためのブロックを拡張します。
- WSV: Iroha ワールド ステート ビュー。バージョン管理、スナップショット、およびコードの保存を拡​​張できます。
- IVM: Iroha 仮想マシンの制御機能の破棄 (バイトコード Kotodama `.to`)。
  - AIR: 代数中間表現。スタークの計算に関する計算を行って、国境と国境の制限を制限するための操作を説明します。

データスペースのモデル
- 識別情報: `DataSpaceId (DSID)` は、Todo の名前空間を指定する DS の識別情報です。ロス DS のインスタンスと粒度:
  - ドメイン DS: `ds::domain::<domain_name>` - ドメインからの取り出し。
  - Asset-DS: `ds::asset::<domain_name>::<asset_name>` - 活動の定義を削除します。
  アンバス・フォルマスは共存します。 las transacciones pueden tocar multiples DSID de forma atomica。
- マニフェストの更新: DS の作成、実際の政治 (ロタシオン、政治活動) およびスペース ディレクトリの登録。 DS のアーティファクト、スロット参照、マニフェストのハッシュ参照。
- 分類: 公開 DS (参加自由、DA 公開) およびプライベート DS (許可、DA 機密)。マニフェストのフラグを介して政治活動を行うことができます。
- DS の政治: permisos ISI、parametros DA `(k,m)`、cifrado、retencion、cuotas (ブロックの最小/最大参加)、politica de pruebas ZK/optimistas、tarifas。
- ゴベルナンザ: メンブレシア DS と有効期限の定義、マニフェスト ポル ラ セクシオン デ ゴベルナンザ デル (チェーン上のプロプエスタ、トランザクション ネクサスとアテスタシオンのマルチシグ オ ゴベルナンザ エクステルナ アンクラダ)。

容量と UAID のマニフェスト
- 普遍的なもの: UAID 決定性 (`UniversalAccountId` と `crates/iroha_data_model/src/nexus/manifest.rs`) のデータスペースに関する参加者の決定。容量損失 (`AssetPermissionManifest`) ビンキュラン、UAID、データスペース固有、エポカの有効化/有効期限、規則の許可/拒否 `ManifestEntry` クエリ アコタン `dataspace`、`program_id`、 `method`、`asset` の役割は AMX オプションです。ラス・レグラスはシエンプレ・ガナンを否定する。評価者は、一致するメタデータに対して `ManifestVerdict::Denied` を許可し、オーディオを許可しません。
- 許可: `max_amount` は省略可能です。ホストの SDK 消費者ミスモ ペイロード Norito は、SDK のアプリケーションで永続的に識別されます。
- Telemetria de audio: Space Directory は、`SpaceDirectoryEvent::{ManifestActivated, ManifestExpired, ManifestRevoked}` (`crates/iroha_data_model/src/events/data/space_directory.rs`) を明示的に表示します。 `SpaceDirectoryEventFilter` は、Torii/データ イベント モニターのマニフェスト UAID の実際のサブスクリプションを許可し、取り消しと決定の拒否と配管の個人化を許可します。エンドツーエンドの操作性の証拠、SDK の移行に関する注意事項、マニフェストの公開チェックリスト、ユニバーサル アカウント ガイド (`docs/source/universal_accounts_guide.md`) のセクションの詳細。 UAID の政治に関する文書は完全に保護されています。

アルト ニベル建築
1) グローバル構成機能 (Nexus チェーン)
- Mantiene は、データ スペース (DS) でのカノニコ ユニコ デ ブロック Nexus の 1 つのセグンド ケ ファイナリザン トランザクション アトミカス ケ アバルカン ウノ マス データ スペース (DS) を要求します。世界状態のグローバルユニフィカドのトランザクション確認 (DS のベクトル)。
- 保証コンポサビリダのミニモス プルエバス/QC アグレガドのコンティエネ、最終処理と不正行為の検出 (DSID のトカドス、DS のルート デ スタド/デスピュー、侵害 DA、DS の検証済みプルエバ、DS のクォーラム証明書の認証) ML-DSA-87)。データのプライバシーは含まれません。
- コンセンサス: BFT グローバル コン パイプライン デ タマノ 22 (3f+1 con f=7) をコミテ、国連メカニスモ VRF/ステーク ポー エポカを介して、プール デ ハスタ ~200,000 の有効性を選択。 1 秒間のトランザクションと最終的な接続を確立します。

2) データスペースの容量 (パブリック/プライベート)
- グローバルな DS デ トランザクションのフラグメントの取り出し、WSV ローカル デル DS の実際のブロックでの有効な成果物の生成 (DS でのプルエバス アグレガダと妥協 DA) 1 つのブロック Nexus デ 1 セグンド。
- プライベート DS cifran datos en reposo y en transito entre validadores autorizados;ソロ妥協とプルエバス デ バリデス PQ サレン デル DS。
- パブリック DS は、(DA 経由で) 完全なデータをエクスポートし、PQ を検証します。

3) トランザクション アトミカス クロスデータスペース (AMX)
- モデル: 複数の DS へのトランザクション取引 (英語、ドメイン DS および資産 DS を参照)。単独ブロック Nexus の中止を確認してください。干し草の影響はありません。
- 準備 - コミット デントロ デ 1 : パラカード トランザクション候補、ロス DS トカドス エジェクタン アン パラレロ コントラ エル ミスモ スナップショット (ルート DS またはスロット)、DS または DS (FASTPQ-ISI) の検証済み PQ のプルエバを生成し、DA を妥協します。 El comite nexusconfirma la transaccion Solo si todas las pruebas DS requeridas verificany los certificados DA llegan atiempo (objetivo <=300 ms);デ・ロ・コントラリオ、ラ・トランザクション・セ・リプログラム・パラ・エル・シグエンテ・スロット。
- 一貫性: los conjuntos de lectura/escritura se declaran;競合が発生したことを検出し、スロットでの競合の原因をコミットします。 DS エビタ ブロック グローバルの排出オプティミスタ シン ロック。 la atomicidad se impone por la regla de commit nexus (todo nada entre DS)。
- プライバシー: プライベート DS エクスポート、ルート DS の事前/事後、ソロ プルエバス/妥協点の制限。販売データはありません privada cruda del DS。

4) Disponibilidad de datos (DA) コンコーディフィカシオン デ ボラード
- クラ アルマセナ ブロックとスナップショット WSV のブロックとブロックのコード。ブロブの公開は断片化され、増幅されます。ブロブ プライベート セ アルマセナン ソロ デントロ デ バリダドレス プライベート DS、コンチャンク シフラドス。
- Los compromisos DA se registran Tanto en artefactos DS como en bloques Nexus, habilitando muestreo y garantias de recuperacion sin revelar contenido privado.

ブロックとコミットの構造
- データスペースのプルエバアーティファクト（スロット1、DS用）
  - Campos: dsid、slot、pre_state_root、post_state_root、ds_tx_set_hash、kura_da_commitment、wsv_da_commitment、manifest_hash、ds_qc (ML-DSA-87)、ds_validity_proof (FASTPQ-ISI)。
  - Private-DS はデータを保存するための成果物をエクスポートします。パブリック DS は DA 経由での回復を許可します。

- ブロック Nexus (カデンシア デ 1 秒)
  - Campos: block_number、parent_hash、slot_time、tx_list (トランザクション アトミカ、クロス DS、DSID、トカド)、ds_artifacts[]、nexus_qc。
  - 機能: 最終的なトランザクション アトミカス キュヨス アーティファクト DS 要求検証; DS の世界状態の実際のベクトルとルーツ、グローバル、ソロパソ。合意とスケジューリング
- Nexus チェーンのコンセンサス: BFT グローバル コンパイプライン (クラス Sumeragi) 22 ノード (3f+1 con f=7) のコンコミットで、1 秒のブロックを実行できます。 VRF/ステークエントリーを介した委員会の選択は約 200,000 人の候補者に達します。マンティエンの降下と抵抗と検閲。
- データ スペースの合意: スロットごとに、DS の取り出しと BFT の有効性を検証します (プルエバス、妥協 DA、DS QC)。レーン リレーの寸法が I1​​8NI00000074X で設定され、`fault_tolerance` データスペースの形式が決定され、データスペースのシードと VRF リガドのデータスペースがプールされ、検証されます。プライベートDSの息子は許可されています。パブリックDSは、反シビルの政治的活動を許可します。エルコミテグローバルネクサスノーカンビア。
- トランザクションのスケジュール: 非常に便利なトランザクションのアトミカス デクラランド DSID と講義/解説のスケジューリング。ロス DS エヘクタン アン パラレロ デントロ デル スロット。 EL Comite Nexus には、トランザクションとブロックの 1 秒間の DS 認証と DA ソン プンタレス (<=300 ミリ秒) が含まれます。
- 交換内容: DS の記憶内容と独立した出力を確認します。 DS の制限は、プライベート DS の遅延を防ぐために、DS のブロックでブロックを確認する必要があります。

データのモデルと名前空間
- DS の ID calificados: todas las entidades (dominios、cuentas、activos、roles) は `dsid` のカリフォルニアです。例: `ds::<domain>::account`、`ds::<domain>::asset#precision`。
- グローバルな参照: グローバルな参照、`(dsid, object_id, version_hint)` および、オンチェーン、キャパ ネクサス、AMX パラメタ、クロス DS の記述。
- シリアル化 Norito: クロス DS (記述子 AMX、プルエバ) 米国コーデック Norito。製造過程においては、一切の努力は必要ありません。

IVM のインテリジェントな拡張機能の比較
- 取り出しコンテキスト: `dsid` および IVM の取り出しコンテキスト。 Los contratos Kotodama は、特定のデータ スペースを参照してください。
- Primitivas atomicas クロス DS:
  - `amx_begin()` / `amx_commit()` トランザクション アトミカ マルチ DS とホスト IVM のデリミタ。
  - `amx_touch(dsid, key)` スロットのスナップショットの紛争検出に関する講義/エスクリトゥーラの宣言。
  - `verify_space_proof(dsid, proof, statement)` -> ブール値
  - `use_asset_handle(handle, op, amount)` -> 結果 (政治的に許可された操作は有効です)
- 関税の取り扱い:
  - ISI/Rol del DS による政治活動の自動化;異教徒のタリファスとガスデルDSのトークン。安全性と政治のトークン (複数の承認者、レート制限、ジオフェンシング) は、アグリゲール マス アデランテの罪であり、原子モデルでもあります。
- 決定主義: 決定性のある新しいシステムコールと、AMX 宣言の決定性と接続性。危険な影響を及ぼします。Pruebas de validez post-cuanticas (ISI ジェネラリザドス)
- FASTPQ-ISI (PQ、信頼できるセットアップ): ハードウェア クラスの GPU で 20k のサブセグンド パラロットを実行し、ハッシュ ベースの一般的な転送を実行します。
  - Perfil 操作:
    - `fastpq_prover::Prover::canonical` を介して、製造コンストラクションのロスノード、製造バックエンドの開始を確認します。エルモックデターミニスタフューリモビデオ。 [crates/fastpq_prover/src/proof.rs:126]
    - `zk.fastpq.execution_mode` (構成) y `irohad --fastpq-execution-mode` は、CPU/GPU の形式決定ミエントラス エル オブザーバー フック レジストラ トリプル ソリシタドス/結果/バックエンド パラ聴取を許可します。 [crates/iroha_config/src/parameters/user.rs:1357] [crates/irohad/src/main.rs:270] [crates/irohad/src/main.rs:2192] [crates/iroha_telemetry/src/metrics.rs:8887]
- アリトメチザシオン:
  - KV-Update AIR: Poseidon2-SMT を介した WSV のマップとキー値の情報の互換性。 Cada ISI は、ファイルの読み出し、チェック、書き込みの接続を拡張します (クエンタス、アクティボス、ロール、ドミニオ、メタデータ、サプライ)。
  - オペコードの制限: ISI の規則を無視する AIR コン列の制限 (保存、単調な制御、許可、範囲チェック、メタデータの実際の処理)。
  - ルックアップの引数: タブは、ハッシュ パラメータの許可/役割、ビットごとの制約の精度と政治パラメータを透明にします。
- 現実的な実現のための妥協点:
  - プルエバ SMT アグレガダ: ラス クラベス トカダス (事前/後) とプルエバン コントラ `old_root`/`new_root` を使用して、フロンティアと兄弟の重複を排除します。
  - Invariantes: invariantes globales (p. ej.、supply total por activity) se imponen via igualdad de multiconjuntos entre filas de efecto y contadores rastreados。
- プルエバのシステム:
  - 感染ポリノミアレス・エスティロ FRI (DEEP-FRI) アリダダ戦 (8/16) 8-16 で大敗。ハッシュポセイドン2; SHA-2/3 でのフィアット シャミールの転写。
  - 再帰オプション: ローカルの再帰集約、DS パラメタのマイクロロット、必要なスロットの再帰。
- アルカンセとエジェンプロス・クビエルトス:
  - Activos: 転送、ミント、書き込み、アセット定義の登録/登録解除、精度の設定 (acotado)、メタデータの設定。
  - Cuentas/Dominios: 作成/削除、キー/しきい値の設定、署名者の追加/削除 (solo estado; las verificaciones de farma se atestan por validadores DS、no se prueban dentro del AIR)。
  - 役割/権限 (ISI): 役割と権限を付与/取り消します。検索や政治チェックのタブラスへの影響は単調です。
  - Contratos/AMX: マルカドールは AMX を開始/コミットし、機能のミント/取り消しは開始されます。政治活動と政治活動の両方を実行します。
- Fuera del AIR パラ プリザーバーのレイテンシアをチェックします。
  - 法律証明書 (p. ej.、法律 ML-DSA de usuarios) は、DS および DS QC の有効性を検証します。ラ・プルエバ・デ・バリデス・キュブレ・ソロ・コンスティステンシア・デ・スタドとクンプリミエント・デ・ポリティカス。エスト・マンティエン・プルエバス・PQ・イ・ラピダス。
- レンディミエントの内容 (イラスト、32 コアの CPU + 現代的な GPU):
  - キータッチペケーノでの 20k ISI ミックスタスク (<=8 クラーベ/ISI): プルエバで ~0.4 ～ 0.9 秒、プルエバで ~150 ～ 450 KB、検証で ~5 ～ 15 ミリ秒。
  - ISI mas pesadas (mas claves/constraints ricas): マイクロロット (p. ej.、10x2k) + スロットごとの再帰パラメータ <1 秒。
- DS マニフェストの構成:
  - `zk.policy = "fastpq_isi"`
  - `zk.hash = "poseidon2"`、`zk.fri = { blowup: 8|16, arity: 8|16 }`
  - `state.commitment = "smt_poseidon2"`
  - `zk.recursion = { none | local }`
  - `attestation.signatures_in_proof = false` (DS QC の認証)
  - `attestation.qc_signature = "ml_dsa_87"` (欠陥がある; 明示的に宣言する代替案)
- フォールバック:
  - ISI complejas/personalizadas pueden usar un STARK General (`zk.policy = "stark_fri_general"`) con prueba diferida y Finalizacion de 1 s via atestacion QC + 無効なスラッシュ。
  - オプションでは PQ (p. ej.、Plonk con KZG) は信頼できるセットアップを必要とせず、ビルド時に問題が発生する可能性があります。AIR の紹介 (パラ Nexus)
- Traza de ejecucion: matriz con ancho (columnas de registros) y longitud (pasos)。 ISI の手続きに関する必要な手続きを実行します。前後の値、セレクター、フラグを列挙します。
- 制限事項:
  - トランジションの制限: フィラとフィラの関係に関する制限 (p. ej.、post_balance = pre_balance - 金額パラウナフィラデビトクアンド `sel_transfer = 1`)。
  - 国境制限: vinculan E/S publica (old_root/new_root、contadores) a la primera/ultima fila。
  - ルックアップ/並べ替え: 複数の結合要素とコントラ タブラス コンプロメティダの構成要素 (権限、活動パラメータ) の回路の動作を確認します。
- 侵害の検証:
  - 証明者は、コードフィカシオネス ハッシュを介して、ポリノミオス デ バホ グラド ケ ソン バリドス サイラス制限を解釈します。
  - 検証者は、FRI (ハッシュベース、クアンティコ後) のメルクルの権限を介してバホグラドを計算します。ロスパソスでのコストとログリトミコ。
- 例 (転送): pre_balance、amount、post_balance、nonce y selectore を含むレジストリ。否定的/乱雑な制限はなく、保守的で単調な管理、マルチプルエバの管理、SMT アグレガダ ヴィンキュラ ホジャス、古い/新しいルーツの前後に制限されています。

ABI システムコールの進化 (ABI v1)
- Syscalls agregar (nombres ilustrativos):
  - `SYS_AMX_BEGIN`、`SYS_AMX_TOUCH`、`SYS_AMX_COMMIT`、`SYS_VERIFY_SPACE_PROOF`、`SYS_USE_ASSET_HANDLE`。
- Tipos Pointer-ABI アグリガー:
  - `PointerType::DataSpaceId`、`PointerType::AmxDescriptor`、`PointerType::AssetHandle`、`PointerType::ProofBlob`。
- 要求事項:
  - Agregar a `ivm::syscalls::abi_syscall_list()` (mantener orden)、gatear por politica。
  - ホスト上の `VMError::UnknownSyscall` の数をマップします。
  - 実際のテスト: システムコール リスト ゴールデン、ABI ハッシュ、ポインター タイプ ID ゴールデン、ポリシー テスト。
  - ドキュメント: `crates/ivm/docs/syscalls.md`、`status.md`、`roadmap.md`。

プライバシーモデル
- データのプライバシーの保持: トランザクション、差分、スナップショットの WSV とプライベート DS のサブコンピューティングのプライベート バリデーション。
- 公開公開: ソロヘッダー、DA および検証済み PQ のエクスポートの妥協。
- Pruebas ZK opcionales: プライベート DS pueden producir pruebas ZK (p. ej.、balance suficiente、politicacumplida) habilitando acciones Cross-DS sin revelar estado interno.
- 政治的権限を管理し、ISI/DS を管理します。損失トークンは、オプションの容量と、最新の情報を紹介します。

管理と QoS の管理
- 合意、メンプールとアルマセナミエント、DS の分離。
- DS のスケジューリング関連性は、アンカーや行頭ブロックを制限したり、含めたりする必要があります。
- DS (コンピューティング/メモリ/IO) の再帰制御、ホスト IVM の制御。パブリック DS については、プライベート DS を事前に確認する必要はありません。
- Llamadas クロス DS asincronas evitan esperas sincronas largas dentro de ejecucion private-DS。

日付とアルマセナミエントのディスポニビリダード
1) ボラードの文書
- Usar Reed-Solomon sistematico (p. ej.、GF(2^16)) ブロックのコードのパラメタとブロックのスナップショット WSV: パラメトロス `(k, m)` と `n = k + m` シャード。
- 欠陥のパラメータ (propuestos、パブリック DS): `k=32, m=16` (n=48)、最大 1.5 倍の拡張で 16 シャードの回復を許可。プライベートDS: `k=16, m=8` (n=24) 権限を与えられます。 DS マニフェストの Ambos 構成可能ファイル。
- Blobs publicos: シャード配布および多数のノード DA/validadores con checks de disponibilidad por muestreo。 DA ヘッダーの侵害により、ライト クライアントの検証が許可されます。
- Blobs privados: シャード cifrados および distribuidos Solo entre validadores private-DS (o custodios designados)。ラ カデナ グローバル ソロ lleva compromisos DA (sin ubicaciones de shards ni llaves)。

2) 妥協と要求
- パラ cada blob: 計算上のマークル ルートの詳細なシャードには、`*_da_commitment` が含まれます。 Mantener PQ evitando compromisos de curva eliptica。
- DA 認証者: VRF の地域認証者 (p. ej.、64 のリージョン) は、シャードの出口に関する認証 ML-DSA-87 を発行します。アテスタシオンの遅延時間 DA <=300 ミリ秒。追加のシャードに関する証明書の有効性を確認します。3) クラの統合
- アルマセナンのブロックは、マークルの不正なブロックのコードをトランザクション処理します。
- ヘッダーを失った場合、ブロブの侵害が発生しました。 los cuerpos se recuperan via la red DA para public DS y via canales privados para private DS。

4) WSV との統合
- スナップショット WSV: チェックポイントを定期的に削除し、DS のスナップショット、チャンク、ヘッダーの登録情報を記録します。スナップショットを保持し、変更ログを保持します。ロススナップショットは断片的に公開されています。ロス スナップショットは永続的なプライバシーを保持します。
- Acceso con pruebas: los contratos pueden proporcionar (o solicitar) pruebas de estado (Merkle/Verkle) ancladas por compromisos de snapshot.プライベート DS は、プルエバス クルーダの安全保障を保護します。

5) 保持と枝刈り
- パブリック DS での罪の枝刈り: Retener todos los cuerpos Kura y スナップショット WSV (DA 経由) (エスカラド水平)。プライベート DS は内部の定義を保持し、不変のデータを永久にエクスポートします。 La capa nexus retiene todos los bloques Nexus y los compromisos de artefactos DS。

ノードの赤の役割
- Validadores Globales: 参加者とエルコンセンサスの関係、検証ブロック Nexus y artefactos DS、realizan チェック DA パラパブリック DS。
- Validadores de Data Space: ejecutan consenso DS、ejecutan contratos、gestionan Kura/WSV local、manejan DA para su DS。
- Nodos DA (オプション): almacenan/publican blob publicos、facilitan muestreo。プライベートDS、ロス・ノードス・ダ・セ・コービカン・コン・バリドーレス・オ・カストディオス・コンフィアブル。

メホラスとシステマを考えてみましょう
- Desacoplar のシーケンス/メモリプール: メモリプール DAG (p. ej.、Estilo Narwhal) を採用し、BFT コンパイプラインとラ キャパ ネクサス パラ バジャール レイテンシアとメモリ モデル ロジックのスループットを確保します。
- Cuotas DS と公平性: cuotas por DS por bloque y caps de peso para evitar の先頭ブロッキングと、プライベート DS の予測可能な遅延遅延。
- アテスタシオン DS (PQ): ML-DSA-87 (クラス ダイリチウム 5) の欠陥による定足数 DS 証明書の欠落。検査後の大規模な企業は、スロットごとの QC に関して EC ごとに受け入れられます。 DS は、ML-DSA-65/44 (マス ペケーノ) または DS マニフェストの EC 宣言を明示的に規定しています。公共 DS には ML-DSA-87 を推奨します。
- DA 認証者: パブリック DS の場合、地域認証者は VRF の認証情報 DA を発行します。シャード クルードの管理に関する証明書の関連付けを行っています。プライベート DS マンティネン アテスタシオネス DA インテルナス。
- 再帰とエポカのプルエバ: オプションのアグリガー バリオス マイクロロット デントロ デ アン DS エン ウナ プルエバの再帰的スロット/エポカ パラ マンテナー タマノ デ プルエバとティエンポの検証を確立します。
- Escalado de LANES (si se necesita): グローバル ユニコ セブエルベとボテラ デ ボテラの共同作業を開始し、マージ決定に関するパラレル レーンの紹介を行います。 Esto preserva un orden global unico mientras escala horizo​​ntalmente。
- 高速決定性: 証明カーネル SIMD/CUDA 機能フラグ パラ ハッシュ/FFT フォールバック CPU ビット正確なパラ プリザーバー決定性クロスハードウェア。
- レーンの活性化 (プロプエスタ): ハビリタール 2 ～ 4 レーン si (a) p95 の最終処理は 3 分以上連続で 1.2 秒を超える、o (b) ブロック内占有率は 5 分以上で 85% を超える、o (c) tx 要求の入力 > 1.2xラ・キャパシダ・デ・ブロック・アン・ニベレス・ソステニドス。 DSID のハッシュの形式を決定するためのトランザクションは、ブロック ネクサスとの結合に使用されます。

タリファスと経済 (初期値)
- ガスのユニダード: DS コンピューティング/IO メディドのトークン ガス。異邦人のタリファス、国内のガス活動。変換はアプリケーションに対する DS の責任です。
- 包含の優先順位: 1 秒間の保存の公平性および SLO に対する DS のラウンドロビン接続。 DS のデントロ、入札料金はデセンパタールです。
- Futuro: 世界のタリファと政治を最小限に抑え、MEV を核として原子爆弾を開発し、PQ を探索します。Flujo クロスデータスペース (例)
1) Un usuario envia una transaccion AMX que toca public DS P y private DS S: mover activo X desde S a beneficiario B cuya cuenta esta en P.
2) スロットのデントロ、スロット コントラ エル スナップショットの断片化されたスロット。権限を検証し、社内で実際に実行され、PQ と侵害 DA (sin filtrar datos privados) を生成します。 P は、実際の通信を準備します (ページ、ミント/バーン/ロックと政治政治)。
3) El comite nexus verifica ambas pruebas DS y certificados DA;スロットの検証を行い、ブロック Nexus でトランザクションを確認し、実際のアンボス ルート DS と世界状態のベクトルをグローバルに確認します。
4) 証明書が無効であること、取引が中止されること (罪の影響)、スロットに関連するクライアントの再実行を許可すること。 Ningun は、S enningun paso でのプライベート販売を行っています。

- セグリダードを考慮してください
- Ejecucion determinista: las syscalls IVM permanecen determinista; DS 間で結果が失われ、AMX コミットとファイナリゼーションが失われ、リロードやタイミングが失われます。
- アクセス制御: プライベート DS での ISI の制限は、トランザクション操作や操作の許可を必要としません。ロストークンデキャパシダコードディフィカンデレチョスデグラノフィノパラユーソクロスDS。
- 機密情報: プライベート DS に関するエンドツーエンドの情報、アルマセナドス ソロ エントレ ミエンブロス オートリザドス、外部からの ZK オプションのシャード情報。
- DoS の解除: プライベート DS の進行状況に影響を与える輻輳を回避するためのメモリ/コンセンサス/アルマセナミエント。

Iroha のコンポーネントの Cambios
- iroha_data_model: `DataSpaceId` の紹介、DS のカリフィカド ID、記述子 AMX (講義/解説)、プルエバ/妥協 DA の説明。ソロ Norito のシリアル化。
- ivm: agregar syscalls ytipos pointer-ABI para AMX (`amx_begin`, `amx_commit`, `amx_touch`) y pruebas DA; ABI 政治政治 v1 の実際のテスト/ドキュメント。