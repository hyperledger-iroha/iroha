---
lang: zh-hans
direction: ltr
source: docs/norito_bridge_release.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: b9dc9862d4806d355fd83c885de92775712a7b32c68c010d29f4fc74229d054b
source_last_modified: "2026-01-06T05:24:53.995808+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# NoritoBridge 发布包装

本指南概述了将 `NoritoBridge` Swift 绑定发布为
可以从 Swift Package Manager 和 CocoaPods 使用的 XCFramework。的
工作流程使 Swift 工件与发布的 Rust 包版本保持同步
Iroha 的 Norito 编解码器。有关使用已发布的内容的端到端说明
应用程序内的工件（Xcode 项目接线、ChaChaPoly 用法等），请参阅
`docs/connect_swift_integration.md`。

> **注意：** 一旦 macOS 构建者具备所需的能力，此流程的 CI 自动化就会落地
> Apple 工具上线（在发布工程 macOS 构建器待办事项中进行跟踪）。
> 在此之前，必须在开发 Mac 上手动执行以下步骤。

## 先决条件

- 安装了最新稳定的 Xcode 命令行工具的 macOS 主机。
- 与工作区 `rust-toolchain.toml` 匹配的 Rust 工具链。
- Swift 工具链 5.7 或更高版本。
- CocoaPods（通过 Ruby gems）如果发布到中央规格存储库。
- 访问 Hyperledger Iroha 版本签名密钥以标记 Swift 工件。

## 版本控制模型

1. 确定 Norito 编解码器 (`crates/norito/Cargo.toml`) 的 Rust 箱版本。
2. 使用版本标识符标记工作区（例如 `v2.1.0`）。
3. 对 Swift 包和 CocoaPods podspec 使用相同的语义版本。
4. 当 Rust 箱增加其版本时，重复该过程并发布匹配的版本
   迅捷神器。测试时版本可能包含元数据后缀（例如 `-alpha.1`）。

## 构建步骤

1. 从存储库根目录调用帮助程序脚本来组装 XCFramework：

   ```bash
   ./scripts/build_norito_xcframework.sh --workspace-root "$(pwd)" \
       --output "artifacts/NoritoBridge.xcframework" \
       --profile release
   ```

   该脚本为 iOS 和 macOS 目标编译 Rust 桥接库并捆绑
   在单个 XCFramework 目录下生成静态库。
   它还发出 `dist/NoritoBridge.artifacts.json`，捕获桥接版本并
   每个平台的 SHA-256 哈希值（如果存在，则使用 `NORITO_BRIDGE_VERSION` 覆盖版本
   需要）。

2. 压缩 XCFramework 进行分发：

   ```bash
   ditto -c -k --sequesterRsrc --keepParent \
     artifacts/NoritoBridge.xcframework \
     artifacts/NoritoBridge.xcframework.zip
   ```

3. 更新 Swift 包清单 (`IrohaSwift/Package.swift`) 以指向新的
   版本和校验和：

   ```bash
   swift package compute-checksum artifacts/NoritoBridge.xcframework.zip
   ```

   定义二进制目标时，将校验和记录在 `Package.swift` 中。

4. 使用新版本、校验和和存档更新 `IrohaSwift/IrohaSwift.podspec`
   网址。

5. **如果桥获得新的导出，则重新生成标头。** Swift 桥现在公开
   `connect_norito_set_acceleration_config` 所以 `AccelerationSettings` 可以切换金属/
   GPU 后端。确保 `NoritoBridge.xcframework/**/Headers/connect_norito_bridge.h`
   压缩前匹配 `crates/connect_norito_bridge/include/connect_norito_bridge.h`。

6. 在标记之前运行 Swift 验证套件：

   ```bash
   swift test --package-path IrohaSwift
   make swift-ci
   ```

   第一个命令确保 Swift 包（包括 `AccelerationSettings`）保持不变
   绿色；第二个验证夹具奇偶校验，呈现奇偶校验/CI 仪表板，以及
   执行与 Buildkite 中强制执行的相同遥测检查（包括
   `ci/xcframework-smoke:<lane>:device_tag` 元数据要求）。

7. 在发布分支中提交生成的工件并标记提交。

## 出版

### Swift 包管理器

- 将标签推送到公共 Git 存储库。
- 确保标签可通过包索引（Apple 或社区镜像）访问。
- 消费者现在可以信赖 `.package(url: "https://github.com/hyperledger/iroha", from: "<version>")`。

### CocoaPods

1. 在本地验证 pod：

   ```bash
   pod lib lint IrohaSwift.podspec --allow-warnings
   ```

2. 推送更新后的 podspec：

   ```bash
   pod trunk push IrohaSwift.podspec
   ```

3. 确认新版本出现在 CocoaPods 索引中。

## CI注意事项

- 创建一个运行打包脚本、归档工件并上传的 macOS 作业
  生成的校验和作为工作流程输出。
- Gate 在针对新生成的框架构建的 Swift 演示应用程序上发布。
- 存储构建日志以帮助诊断故障。

## 其他自动化想法

- 一旦所有需要的目标都暴露出来，就直接使用 `xcodebuild -create-xcframework`。
- 集成签名/公证以在开发人员机器之外分发。
- 通过固定 SPM 使集成测试与打包版本保持同步
  对发布标签的依赖。