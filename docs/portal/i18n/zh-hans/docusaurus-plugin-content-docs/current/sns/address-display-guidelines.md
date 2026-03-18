---
id: address-display-guidelines
lang: zh-hans
direction: ltr
source: docs/portal/docs/sns/address-display-guidelines.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
title: Sora Address Display Guidelines
sidebar_label: Address display
description: UX and CLI requirements for I105 vs I105 Sora address presentation (ADDR-6).
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

从 '@site/src/components/ExplorerAddressCard' 导入 ExplorerAddressCard；

:::注意规范来源
此页面镜像 `docs/source/sns/address_display_guidelines.md`，现在提供服务
作为规范门户副本。源文件会保留下来用于翻译 PR。
:::

钱包、浏览器和 SDK 示例必须将帐户地址视为不可变
有效负载。 Android 零售钱包示例位于
`examples/android/retail-wallet` 现在演示所需的 UX 模式：

- **双复制目标。** 提供两个显式复制按钮 - I105（首选）和
  仅压缩的 Sora 形式（`i105`，第二好）。 I105 始终安全地对外共享
  并为 QR 有效负载供电。压缩变体必须包含内联
  警告，因为它仅适用于 Sora 感知的应用程序。 Android 零售
  钱包示例将 Material 按钮及其工具提示连接到
  `examples/android/retail-wallet/src/main/res/layout/activity_main.xml`，和
  iOS SwiftUI 演示通过内部 `AddressPreviewCard` 镜像相同的 UX
  `examples/ios/NoritoDemo/Sources/ContentView.swift`。
- **等宽，可选文本。** 使用等宽字体渲染两个字符串并
  `textIsSelectable="true"`，以便用户无需调用 IME 即可检查值。
  避免可编辑字段：IME 可以重写假名或注入零宽度代码点。
- **隐式默认域提示。**当选择器指向隐式
  `default` 域名，表面有标题提醒操作员无需后缀。
  当选择器时，浏览器还应该突出显示规范域标签
  编码摘要。
- **I105 QR 有效负载。** QR 代码必须对 I105 字符串进行编码。如果二维码生成
  失败，显示明确的错误而不是空白图像。
- **剪贴板消息传递。** 复制压缩表单后，发出祝酒词或
  小吃栏提醒用户它仅适用于 Sora，并且容易出现 IME 损坏。

遵循这些护栏可以防止 Unicode/IME 损坏并满足
钱包/浏览器用户体验的 ADDR-6 路线图接受标准。

## 截图装置

在本地化审查期间使用以下固定装置以确保按钮标签，
工具提示和警告在各个平台上保持一致：

- Android参考：`/img/sns/address_copy_android.svg`

  ![Android双拷参考](/img/sns/address_copy_android.svg)

- iOS 参考号：`/img/sns/address_copy_ios.svg`

  ![iOS双副本参考](/img/sns/address_copy_ios.svg)

## SDK 帮助程序

每个 SDK 都公开了一个方便的助手，可返回 I105（首选）和压缩的（`sora`，第二好）
与警告字符串一起形成，以便 UI 层可以保持一致：

- JavaScript：`AccountAddress.displayFormats(networkPrefix?: number)`
  （`javascript/iroha_js/src/address.js`）
- JavaScript 检查器：`inspectAccountId(...)` 返回压缩警告
  字符串并在调用者提供 `i105` 时将其附加到 `warnings`
  字面意思，因此浏览器/钱包仪表板可以显示仅限 Sora 的通知
  在粘贴/验证流程期间而不是仅在它们生成时
  压缩形式本身。
- Python：`AccountAddress.display_formats(network_prefix: int = 753)`
- 斯威夫特：`AccountAddress.displayFormats(networkPrefix: UInt16 = 753)`
- Java/Kotlin：`AccountAddress.displayFormats(int networkPrefix = 753)`
  （`java/iroha_android/src/main/java/org/hyperledger/iroha/android/address/AccountAddress.java`）

使用这些帮助器而不是在 UI 层中重新实现编码逻辑。
JavaScript 帮助程序还在 `domainSummary` 上公开了 `selector` 有效负载
（`tag`、`digest_hex`、`registry_id`、`label`），因此 UI 可以指示是否
选择器是 Local-12 或注册表支持的，无需重新解析原始有效负载。

## Explorer 仪器演示

<浏览器地址卡/>

探索者应该镜像钱包遥测和可访问性工作：

- 应用 `data-copy-mode="i105|i105_default|qr"` 复制按钮，以便前端可以发出使用计数器
  与 Torii 侧 `torii_address_format_total` 指标一起。上面的演示组件调度
  带有 `{mode,timestamp}` 的 `iroha:address-copy` 事件 - 将其连接到您的分析/遥测中
  管道（例如，推送到 Segment 或 NORITO 支持的收集器），以便仪表板可以关联服务器
  地址格式与客户端复制行为的使用。同时镜像 Torii 域计数器
  (`torii_address_domain_total{domain_kind}`) 在同一个 feed 中，以便 Local-12 退休评论可以
  直接从 `address_ingest` 导出 30 天 `domain_kind="local12"` 零使用证明
  Grafana 板。
- 将每个控件与不同的 `aria-label`/`aria-describedby` 提示配对，以解释是否
  文字可以安全地共享（`I105`）或仅限 Sora（压缩的 `sora`）。将隐式域标题包含在
  辅助技术的描述表面与视觉上显示的上下文相同。
- 公开实时区域（例如 `<output aria-live="polite">…</output>`），宣布复制结果并
  警告，与现在连接到 Swift/Android 示例中的 VoiceOver/TalkBack 行为相匹配。

该仪器通过证明操作员可以观察 Torii 摄取和
禁用本地选择器之前的客户端复制模式。

## 本地→全局迁移工具包

使用[本地→全局工具包](local-to-global-toolkit.md)来自动化
JSON 审核报告和操作员附加的转换后的首选 I105/第二最佳压缩 (`sora`) 列表
到准备票，而随附的操作手册链接 Grafana
控制严格模式切换的仪表板和 Alertmanager 规则。

## 二进制布局快速参考 (ADDR-1a)

当 SDK 提供高级地址工具（检查器、验证提示、
清单构建器），让开发人员了解在中捕获的规范传输格式
`docs/account_structure.md`。布局始终是
`header · selector · controller`，其中标头位为：

```
bit index:   7        5 4      3 2      1 0
             ┌─────────┬────────┬────────┬────┐
payload bit: │version  │ class  │  norm  │ext │
             └─────────┴────────┴────────┴────┘
```

- 今天 `addr_version = 0`（位 7-5）；非零值被保留并且必须
  提高 `AccountAddressError::InvalidHeaderVersion`。
- `addr_class` 区分单个 (`0`) 与多重签名 (`1`) 控制器。
- `norm_version = 1` 编码 Normv1 选择器规则。未来的规范将被重用
  相同的 2 位字段。
- `ext_flag` 始终为 `0` — 设置位指示不支持的有效负载扩展。

选择器紧跟在标题后面：

```
┌──────────┬──────────────────────────────────────────────┐
│ tag (u8) │ payload (depends on selector kind)           │
└──────────┴──────────────────────────────────────────────┘
```

UI 和 SDK 表面应该准备好显示选择器类型：

- `0x00` = 隐式默认域（无负载）。
- `0x01` = 本地摘要（12 字节 `blake2s_mac("SORA-LOCAL-K:v1", label)`）。
- `0x02` = 全局注册表项（大端 `registry_id:u32`）。

钱包工具可以链接或嵌入文档/测试的规范十六进制示例：

|选择器种类|规范六角 |
|----------------|---------------|
|隐式默认| `0x020001203b6a27bcceb6a42d62a3a8d02a6f0d73653215771de243a63ac048a18b59da29` |
|本地摘要 (`treasury`) | `0x0201b18fe9c1abbac45b3e38fc5d0001203b77a042f1de02f6d5f418f36a2a28ea` |
|全局注册表 (`android`) | `0x020200000059a6a47eb7c9aa415f77b18636a85a57837d5518ff5357ef63c35202` |

请参阅 `docs/source/references/address_norm_v1.md` 了解完整的选择器/状态
表和 `docs/account_structure.md` 为完整的字节图。

## 强制执行规范形式

字符串必须遵循 ADDR-5 下记录的 CLI 工作流程：

1. `iroha tools address inspect` 现在使用 I105 发出结构化 JSON 摘要，
   压缩的、规范的十六进制有效负载。摘要还包括 `domain`
   具有 `kind`/`warning` 字段的对象，并通过以下方式回显任何提供的域
   `input_domain` 字段。当 `kind` 为 `local12` 时，CLI 会打印一条警告
   stderr 和 JSON 摘要呼应相同的指导，因此 CI 管道和 SDK
   可以将其浮现出来。每当您需要转换时传递 `legacy  suffix`
   编码重播为 `<i105>@<domain>`。
2. SDK 可以通过 JavaScript 帮助程序显示相同的警告/摘要：

   ```js
   import { inspectAccountId } from "@iroha/iroha-js";

   const summary = inspectAccountId("sora...");
   if (summary.domain.warning) {
     console.warn(summary.domain.warning);
   }
   console.log(summary.i105.value, summary.i105Warning);
   ```
  帮助器保留从文字中检测到的 I105 前缀，除非您
  明确提供 `networkPrefix`，因此非默认网络的摘要不会
  不会使用默认前缀默默地重新渲染。

3. 通过重用 `i105.value` 或 `i105_default` 转换规范负载
   摘要中的字段（或通过 `--format` 请求其他编码）。这些
   字符串已经可以安全地与外部共享。
4. 使用以下内容更新清单、注册表和面向客户的文档
   规范形式并通知交易对手本地选择器将是
   切换完成后被拒绝。
5. 对于批量数据集，运行
   `iroha tools address audit --input addresses.txt --network-prefix 753`。命令
   读取换行符分隔的文字（以 `#` 开头的注释将被忽略，并且
   `--input -` 或无标志使用 STDIN），发出 JSON 报告
   每个条目的规范/首选 I105/第二最佳压缩 (`sora`) 摘要，并对两个解析进行计数
   包含垃圾行的转储以及带有 `strict CI post-check` 的门自动化
   一旦操作员准备好阻止 CI 中的本地选择器。
6. 当需要换行符到换行符重写时，使用
  对于本地选择器修复电子表格，请使用
  导出 `input,status,format,…` CSV，一次性突出显示规范编码、警告和解析失败。
   默认情况下，助手会跳过非本地行，转换每个剩余的条目
   到请求的编码（I105首选/压缩（`sora`）第二好/十六进制/JSON），并保留
   设置 `legacy  suffix` 时的原始域。与 `--allow-errors` 配对
   即使转储包含格式错误的文字也可以继续扫描。
7. CI/lint 自动化可以运行 `ci/check_address_normalize.sh`，它提取
   来自 `fixtures/account/address_vectors.json` 的本地选择器，转换
   通过 `iroha tools address normalize` 进行回放
   `iroha tools address audit` 证明版本不再发出
   本地摘要。`torii_address_local8_total{endpoint}`加
`torii_address_collision_total{endpoint,kind="local12_digest"}`，
`torii_address_collision_domain_total{endpoint,domain}`，以及
Grafana 板 `dashboards/grafana/address_ingest.json` 提供执行
信号：一旦生产仪表板显示零合法的本地提交和
连续 30 天零 Local-12 冲突，Torii 将翻转 Local-8
主网上出现硬故障，一旦全局域出现故障，则 Local-12 紧随其后
匹配的注册表项。将 CLI 输出视为面向操作员的通知
对于此冻结 - SDK 工具提示使用相同的警告字符串，并且
自动化以与路线图退出标准保持一致。 Torii 现在默认为
当诊断回归时。保持镜像 `torii_address_domain_total{domain_kind}`
进入 Grafana (`dashboards/grafana/address_ingest.json`) 所以 ADDR-7 证据包
可以证明 `domain_kind="local12"` 在之前所需的 30 天窗口内保持为零
(`dashboards/alerts/address_ingest_rules.yml`)增加了三个护栏：

- 每当上下文报告新的 Local-8 时，`AddressLocal8Resurgence` 页面
  增量。停止严格模式推出，在
  直到信号返回到零，然后恢复默认值 (`true`)。
- 当两个 Local-12 标签散列到相同的值时，`AddressLocal12Collision` 会触发
  消化。暂停清单促销，运行本地 → 全局工具包进行审核
  摘要映射，并在重新发布之前与 Nexus 治理进行协调
  注册表项或重新启用下游部署。
- `AddressInvalidRatioSlo` 当车队范围内的无效比率（不包括
  本地 8/严格模式拒绝）超过 0.1% SLO 十分钟。使用
  `torii_address_invalid_total` 查明负责任的背景/原因和
  在重新启用严格模式之前与所属 SDK 团队协调。

### 发行说明片段（钱包和浏览器）

发货时，请在钱包/浏览器发行说明中包含以下项目符号
切换：

> **地址：** 添加了 `iroha tools address normalize`
> helper 并将其连接到 CI (`ci/check_address_normalize.sh`) 所以钱包/资源管理器
> 在主网上阻止 Local-8/Local-12 之前。将任何自定义导出更新为
> 运行命令并将规范化列表附加到发布证据包中。