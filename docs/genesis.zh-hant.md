---
lang: zh-hant
direction: ltr
source: docs/genesis.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 9c2eab4379aa346ab7d111e1c51c0230238f260647187f1a33c1819640b9bf2c
source_last_modified: "2026-01-28T14:25:37.056140+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# 創世配置

`genesis.json` 文件定義 Iroha 網絡啟動時運行的第一個事務。該文件是一個包含以下字段的 JSON 對象：

- `chain` – 唯一鏈標識符。
- `executor`（可選）——執行器字節碼的路徑 (`.to`)。如果存在的話，
  genesis 包括一個升級指令作為第一筆交易。如果省略，
  不執行升級並使用內置執行器。
- `ivm_dir` – 包含 IVM 字節碼庫的目錄。如果省略，則默認為 `"."`。
- `consensus_mode` – 清單中公佈的共識模式。必需的;對於公共 Sora Nexus 數據空間使用 `"Npos"`，對於其他 Iroha3 數據空間使用 `"Permissioned"`/`"Npos"`。 Iroha2 默認為 `"Permissioned"`。
- `transactions` – 按順序執行的創世交易列表。每個條目可能包含：
  - `parameters` – 初始網絡參數。
  - `instructions` – 結構化 Norito 指令（例如 `{ "Register": { "Domain": { "id": "wonderland" }}}`）。不接受原始字節數組，並且此處拒絕 `SetParameter` 指令 - 通過 `parameters` 塊提供種子參數，並讓規範化/簽名註入指令。
  - `ivm_triggers` – 使用 IVM 字節碼可執行文件觸發。
  - `topology` – 初始對等拓撲。每個條目將對等 ID 和 PoP 保存在一起：`{ "peer": "<public_key>", "pop_hex": "<hex>" }`。 `pop_hex` 在撰寫時可以省略，但在簽名之前必須存在。
- `crypto` – 從 `iroha_config.crypto` 鏡像的加密快照（`default_hash`、`allowed_signing`、`allowed_curve_ids`、`sm2_distid_default`、`sm_openssl_preview`）。 `allowed_curve_ids` 鏡像 `crypto.curves.allowed_curve_ids`，因此清單可以通告集群接受哪些控制器曲線。工具強制實施 SM 組合：列出 `sm2` 的清單還必須將哈希值切換為 `sm3-256`，而在沒有 `sm` 功能的情況下編譯的版本完全拒絕 `sm2`。標準化將 `crypto_manifest_meta` 自定義參數注入到簽名的創世中；如果注入的有效負載與通告的快照不一致，節點將拒絕啟動。

示例（`kagami genesis generate default --consensus-mode npos` 輸出，指令已修剪）：

```json
{
  "chain": "00000000-0000-0000-0000-000000000000",
  "ivm_dir": "defaults",
  "transactions": [
    {
      "parameters": { "sumeragi": { "block_time_ms": 2000 } },
      "instructions": [
        { "Register": { "Domain": { "id": "wonderland" } } }
      ],
      "ivm_triggers": [],
      "topology": [
        {
          "peer": "ed25519:...",
          "pop_hex": "ab12cd..."
        }
      ]
    }
  ],
  "consensus_mode": "Npos",
  "crypto": {
    "default_hash": "blake2b-256",
    "allowed_signing": ["ed25519", "secp256k1"],
    "allowed_curve_ids": [1],
    "sm2_distid_default": "1234567812345678",
    "sm_openssl_preview": false
  }
}
```

### 為 SM2/SM3 播種 `crypto` 塊

使用 xtask 幫助程序一步生成關鍵清單和準備粘貼的配置片段：

```bash
cargo xtask sm-operator-snippet \
  --distid CN12345678901234 \
  --json-out sm2-key.json \
  --snippet-out client-sm2.toml
```

`client-sm2.toml` 現在包含：

```toml
# Account key material
public_key = "sm2:8626530010..."
private_key = "A333F581EC034C1689B750A827E150240565B483DEB28294DDB2089AD925A569"
# public_key_pem = """\
-----BEGIN PUBLIC KEY-----
...
-----END PUBLIC KEY-----
"""
# private_key_pem = """\
-----BEGIN PRIVATE KEY-----
...
-----END PRIVATE KEY-----
"""

[crypto]
default_hash = "sm3-256"
allowed_signing = ["ed25519", "secp256k1", "sm2"]  # remove "sm2" to stay in verify-only mode
allowed_curve_ids = [1]               # add new curve ids (e.g., 15 for SM2) when controllers are allowed
sm2_distid_default = "CN12345678901234"
# enable_sm_openssl_preview = true  # optional: only when deploying the OpenSSL/Tongsuo path
```

將 `public_key`/`private_key` 值複製到賬戶/客戶端配置中，並更新 `genesis.json` 的 `crypto` 塊，使其與代碼片段匹配（例如，將 `default_hash` 設置為 `sm3-256`，添加`"sm2"` 至 `allowed_signing`，並包括右側的 `allowed_curve_ids`）。 Kagami 將拒絕哈希/曲線設置和簽名列表不一致的清單。

> **提示：** 當您只想檢查輸出時，使用 `--snippet-out -` 將代碼片段流式傳輸到標準輸出。還使用 `--json-out -` 在標準輸出上發出密鑰清單。

如果您更喜歡手動執行較低級別的 CLI 命令，則等效流程為：

```bash
# 1. Produce deterministic key material (writes JSON to disk)
cargo run -p iroha_cli --features sm -- \
  crypto sm2 keygen \
  --distid CN12345678901234 \
  --output sm2-key.json

# 2. Re-hydrate the snippet that can be pasted into client/config files
cargo run -p iroha_cli --features sm -- \
  crypto sm2 export \
  --private-key-hex "$(jq -r .private_key_hex sm2-key.json)" \
  --distid CN12345678901234 \
  --snippet-output client-sm2.toml \
  --emit-json --quiet
```

> **提示：** 上面使用 `jq` 來節省手動複製/粘貼步驟。如果不可用，則打開 `sm2-key.json`，複製 `private_key_hex` 字段，直接傳遞給 `crypto sm2 export`。

> **遷移指南：** 將現有網絡轉換為 SM2/SM3/SM4 時，請按照
> [`docs/source/crypto/sm_config_migration.md`](source/crypto/sm_config_migration.md)
> 用於分層 `iroha_config` 覆蓋、清單重新生成和回滾
> 規劃。

## 生成並驗證

1. 生成模板：
   ```bash
   cargo run -p iroha_kagami -- genesis generate \
     [--executor <path/to/executor.to>] \
     --consensus-mode npos \
     --ivm-dir <ivm/dir> \
     --genesis-public-key <PUBLIC_KEY> > genesis.json
   ```
`--consensus-mode` 控制哪些共識參數 Kagami 種子到 `parameters` 塊中。公共 Sora Nexus 數據空間需要 `npos` 並且不支持分階段切換；其他 Iroha3 數據空間可能使用許可或 NPoS。 Iroha2 默認為 `permissioned`，並且可以通過 `--next-consensus-mode`/`--mode-activation-height` 上演 `npos`。當選擇 `npos` 時，Kagami 會播種 `sumeragi_npos_parameters` 負載，驅動 NPoS 收集器扇出、選舉策略和重新配置窗口；規範化/簽名將它們轉換為簽名塊中的 `SetParameter` 指令。
2. （可選）編輯 `genesis.json`，然後驗證並簽名：
   ```bash
   cargo run -p iroha_kagami -- genesis sign genesis.json \
     --public-key <PUBLIC_KEY> \
     --private-key <PRIVATE_KEY> \
     --out-file genesis.signed.nrt
   ```

   要發出 SM2/SM3/SM4 就緒清單，請傳遞 `--default-hash sm3-256` 並包含 `--allowed-signing sm2`（對於其他算法，請重複 `--allowed-signing`）。如果需要覆蓋默認的區別標識符，請使用 `--sm2-distid-default <ID>`。

   當您僅使用 `--genesis-manifest-json`（無簽名創世塊）啟動 `irohad` 時，節點現在會自動從清單中播種其運行時加密配置；如果您還提供了創世塊，則清單和配置仍然必須完全匹配。

- 驗證說明：
  - Kagami 將 `consensus_handshake_meta`、`confidential_registry_root` 和 `crypto_manifest_meta` 作為 `SetParameter` 指令注入標準化/有符號塊中。如果握手元數據或加密快照與編碼參數不一致，`irohad` 將重新計算這些有效負載的共識指紋，並導致啟動失敗。將這些內容保留在清單中的 `instructions` 之外；它們是自動生成的。
- 檢查標準化塊：
  - 運行 `kagami genesis normalize genesis.json --format text` 以查看最終的有序交易（包括注入的元數據），而無需提供密鑰對。
  - 使用 `--format json` 轉儲適合比較或評論的結構化視圖。

`kagami genesis sign` 檢查 JSON 是否有效，並生成一個 Norito 編碼塊，可供通過節點配置中的 `genesis.file` 使用。生成的 `genesis.signed.nrt` 已經採用規範的線形式：版本字節後跟描述有效負載佈局的 Norito 標頭。始終分發此框架輸出。首選 `.nrt` 後綴作為簽名的有效負載；如果您不需要在創世時升級執行器，則可以省略 `executor` 字段並跳過提供 `.to` 文件。

簽署 NPoS 清單（`--consensus-mode npos` 或僅限 Iroha2 的分階段切換）時，`kagami genesis sign` 需要 `sumeragi_npos_parameters` 有效負載；使用 `kagami genesis generate --consensus-mode npos` 生成它或手動添加參數。
默認情況下，`kagami genesis sign` 使用清單的 `consensus_mode`；通過 `--consensus-mode` 來覆蓋它。

## Genesis 可以做什麼

Genesis 支持以下操作。 Kagami 以明確定義的順序將它們組裝成事務，以便對等點確定地執行相同的序列。

- 參數：設置 Sumeragi 的初始值（塊/提交時間、漂移）、塊（最大 txs）、交易（最大指令、字節碼大小）、執行器和智能合約限制（燃料、內存、深度）以及自定義參數。 Kagami 通過 `parameters` 塊播種 `Sumeragi::NextMode` 和 `sumeragi_npos_parameters` 有效負載（NPoS 選舉、重新配置），以便啟動可以從鏈上狀態應用共識旋鈕；簽名塊攜帶生成的 `SetParameter` 指令。
- 原生指令：註冊/註銷域名、賬戶、資產定義；鑄造/銷毀/轉移資產；轉移域名和資產定義所有權；修改元數據；授予權限和角色。
- IVM 觸發器：執行 IVM 字節碼的寄存器觸發器（請參閱 `ivm_triggers`）。觸發器的可執行文件相對於 `ivm_dir` 進行解析。
- 拓撲：通過任何事務中的 `topology` 數組提供初始對等點集（通常是第一個或最後一個）。每個條目都是 `{ "peer": "<public_key>", "pop_hex": "<hex>" }`； `pop_hex` 在撰寫時可以省略，但在簽名之前必須存在。
- 執行器升級（可選）：如果存在 `executor`，創世會插入一條升級指令作為第一個交易；否則，創世會直接從參數/指令開始。

### 交易排序

從概念上講，創世交易按以下順序處理：

1）（可選）執行器升級
2) 對於 `transactions` 中的每筆交易：
   - 參數更新
   - 原生指令
   - IVM 觸發註冊
   - 拓撲條目

Kagami 和節點代碼確保此排序，以便例如參數在同一事務中的後續指令之前應用。

## 推薦的工作流程

- 從 Kagami 的模板開始：
  - 僅內置 ISI：`kagami genesis generate --ivm-dir <dir> --genesis-public-key <PK> --consensus-mode npos > genesis.json`（Sora Nexus 公共數據空間；對於 Iroha2 或私有 Iroha3 使用 `--consensus-mode permissioned`）。
  - 使用自定義執行器升級（可選）：添加 `--executor <path/to/executor.to>`
  - 僅 Iroha2：要在未來切換到 NPoS，請傳遞 `--next-consensus-mode npos --mode-activation-height <HEIGHT>`（為當前模式保留 `--consensus-mode permissioned`）。
- `<PK>` 是 `iroha_crypto::Algorithm` 識別的任何多重哈希，包括使用 `--features gost` 構建 Kagami 時的 TC26 GOST 變體（例如 `gost3410-2012-256-paramset-a:...`）。
- 編輯時驗證：`kagami genesis validate genesis.json`
- 部署簽名：`kagami genesis sign genesis.json --public-key <PK> --private-key <SK> --out-file genesis.signed.nrt`
- 配置對等點：將 `genesis.file` 設置為已簽名的 Norito 文件（例如 `genesis.signed.nrt`），將 `genesis.public_key` 設置為用於簽名的相同 `<PK>`。注意事項：
- Kagami 的“默認”模板註冊示例域和帳戶，鑄造一些資產，並僅使用內置 ISI 授予最低權限 - 不需要 `.to`。
- 如果您確實包含執行程序升級，則它必須是第一筆交易。 Kagami 在生成/簽名時強制執行此操作。
- 在簽名之前使用 `kagami genesis validate` 捕獲無效的 `Name` 值（例如空格）和格式錯誤的指令。

## 使用 Docker/Swarm 運行

提供的 Docker Compose 和 Swarm 工具可以處理這兩種情況：

- 沒有執行程序：compose 命令會刪除丟失/空的 `executor` 字段並對文件進行簽名。
- 使用執行器：它將相對執行器路徑解析為容器內的絕對路徑並對文件進行簽名。

這使得在沒有預先構建 IVM 示例的機器上進行開髮變得簡單，同時仍然允許在需要時升級執行器。