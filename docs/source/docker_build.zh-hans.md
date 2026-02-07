---
lang: zh-hans
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

该容器在 `Dockerfile.build` 中定义，并捆绑所有工具链
CI 和本地发布版本所需的依赖项。该图像现在作为
默认情况下非 root 用户，因此 Git 操作继续与 Arch Linux 一起使用
`libgit2` 软件包，无需求助于全局 `safe.directory` 解决方法。

## 构建参数

- `BUILDER_USER` – 在容器内创建的登录名（默认值：`iroha`）。
- `BUILDER_UID` – 数字用户 ID（默认值：`1000`）。
- `BUILDER_GID` – 主要组 ID（默认值：`1000`）。

当您从主机挂载工作区时，传递匹配的 UID/GID 值，以便
生成的工件保持可写：

```bash
docker build \
  -f Dockerfile.build \
  --build-arg BUILDER_UID="$(id -u)" \
  --build-arg BUILDER_GID="$(id -g)" \
  --build-arg BUILDER_USER="iroha" \
  -t iroha-builder .
```

工具链目录（`/usr/local/rustup`、`/usr/local/cargo`、`/opt/poetry`）
由配置的用户拥有，因此 Cargo、rustup 和 Poetry 命令完全保留
一旦容器删除 root 权限，就可以使用。

## 运行构建

调用时将您的工作区附加到 `/workspace`（容器 `WORKDIR`）
图像。示例：

```bash
docker run --rm -it \
  -v "$PWD":/workspace \
  iroha-builder \
  cargo build --workspace
```

该映像保留 `docker` 组成员身份，因此嵌套 Docker 命令（例如
`docker buildx bake`) 仍然可用于挂载主机 PID 的 CI 工作流程
和插座。根据您的环境需要调整组映射。

## Iroha 2 与 Iroha 3 文物

工作区现在为每个发布行发出单独的二进制文件以避免冲突：
`iroha3`/`iroha3d`（默认）和 `iroha2`/`iroha2d` (Iroha 2)。使用助手来
产生所需的对：

- `make build`（或 `BUILD_PROFILE=deploy bash scripts/build_line.sh --i3`）用于 Iroha 3
- `make build-i2`（或 `BUILD_PROFILE=deploy bash scripts/build_line.sh --i2`）用于 Iroha 2

选择器引脚功能集（`telemetry` + `schema-endpoint` 加上
线路特定的 `build-i{2,3}` 标志），因此 Iroha 2 版本不会意外拾取
Iroha 仅 3 个默认值。

通过 `scripts/build_release_bundle.sh` 构建的发布包选择正确的二进制文件
当 `--profile` 设置为 `iroha2` 或 `iroha3` 时，自动命名。