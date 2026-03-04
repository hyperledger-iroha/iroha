---
lang: zh-hant
direction: ltr
source: docs/source/docker_build.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: a5ac4be3d387269898112d465ec404490f67c6c2b9267c0a0781d0de70cf783d
source_last_modified: "2025-12-29T18:16:35.951567+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# Docker 生成器映像

該容器在 `Dockerfile.build` 中定義，並捆綁所有工具鏈
CI 和本地發布版本所需的依賴項。該圖像現在作為
默認情況下非 root 用戶，因此 Git 操作繼續與 Arch Linux 一起使用
`libgit2` 軟件包，無需求助於全局 `safe.directory` 解決方法。

## 構建參數

- `BUILDER_USER` – 在容器內創建的登錄名（默認值：`iroha`）。
- `BUILDER_UID` – 數字用戶 ID（默認值：`1000`）。
- `BUILDER_GID` – 主要組 ID（默認值：`1000`）。

當您從主機掛載工作區時，傳遞匹配的 UID/GID 值，以便
生成的工件保持可寫：

```bash
docker build \
  -f Dockerfile.build \
  --build-arg BUILDER_UID="$(id -u)" \
  --build-arg BUILDER_GID="$(id -g)" \
  --build-arg BUILDER_USER="iroha" \
  -t iroha-builder .
```

工具鏈目錄（`/usr/local/rustup`、`/usr/local/cargo`、`/opt/poetry`）
由配置的用戶擁有，因此 Cargo、rustup 和 Poetry 命令完全保留
一旦容器刪除 root 權限，就可以使用。

## 運行構建

調用時將您的工作區附加到 `/workspace`（容器 `WORKDIR`）
圖像。示例：

```bash
docker run --rm -it \
  -v "$PWD":/workspace \
  iroha-builder \
  cargo build --workspace
```

該映像保留 `docker` 組成員身份，因此嵌套 Docker 命令（例如
`docker buildx bake`) 仍然可用於掛載主機 PID 的 CI 工作流程
和插座。根據您的環境需要調整組映射。

## Iroha 2 與 Iroha 3 文物

工作區現在為每個發布行發出單獨的二進製文件以避免衝突：
`iroha3`/`iroha3d`（默認）和 `iroha2`/`iroha2d` (Iroha 2)。使用助手來
產生所需的對：

- `make build`（或 `BUILD_PROFILE=deploy bash scripts/build_line.sh --i3`）用於 Iroha 3
- `make build-i2`（或 `BUILD_PROFILE=deploy bash scripts/build_line.sh --i2`）用於 Iroha 2

選擇器引腳功能集（`telemetry` + `schema-endpoint` 加上
線路特定的 `build-i{2,3}` 標誌），因此 Iroha 2 版本不會意外拾取
Iroha 僅 3 個默認值。

通過 `scripts/build_release_bundle.sh` 構建的發布包選擇正確的二進製文件
當 `--profile` 設置為 `iroha2` 或 `iroha3` 時，自動命名。