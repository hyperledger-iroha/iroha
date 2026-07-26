<!-- Japanese translation of docs/source/docker_build.md -->

---
lang: ja
direction: ltr
source: docs/source/docker_build.md
status: complete
translator: manual
---

# Docker ビルダーイメージ

このコンテナは `Dockerfile.build` で定義されており、CI やローカルでのリリースビルドに必要なツールチェーン依存関係が一式同梱されています。イメージはデフォルトで非 root ユーザーとして実行されるため、Arch Linux の `libgit2` パッケージにおいてもグローバルな `safe.directory` 回避策に頼らずに Git 操作を継続できます。

## ビルド引数

- `BUILDER_USER` – コンテナ内で作成されるログインユーザー名（既定値: `iroha`）。
- `BUILDER_UID` – ユーザーID（既定値: `1000`）。
- `BUILDER_GID` – プライマリグループ ID（既定値: `1000`）。

ホストのワークスペースをマウントする際は、生成物が書き込み可能なままとなるよう、UID/GID を合わせて渡してください。

```bash
docker build \
  -f Dockerfile.build \
  --build-arg BUILDER_UID="$(id -u)" \
  --build-arg BUILDER_GID="$(id -g)" \
  --build-arg BUILDER_USER="iroha" \
  -t iroha-builder .
```

ツールチェーンディレクトリ（`/usr/local/rustup`, `/usr/local/cargo`, `/opt/poetry`）は指定したユーザーの所有となるため、コンテナが root 権限を降ろした後でも Cargo、rustup、Poetry コマンドは問題なく利用できます。

## ビルドの実行

イメージを使用する際は、ワークスペースを `/workspace`（コンテナの `WORKDIR`）にマウントしてください。例:

```bash
docker run --rm -it \
  -v "$PWD":/workspace \
  iroha-builder \
  cargo build --workspace
```

イメージには `docker` グループが含まれているため、ホストの PID やソケットをマウントする CI ワークフローでも `docker buildx bake` のようなネストした Docker コマンドを利用できます。必要に応じてグループマッピングは環境に合わせて調整してください。
