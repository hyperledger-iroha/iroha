---
lang: zh-hans
direction: ltr
source: docs/source/ivm_architecture_plan.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: da8a99adbbcf1d8b209a25da32e256c0dad2860633f373d7410a3a91d790c938
source_last_modified: "2026-01-21T19:17:13.236818+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# IVM 架构重构计划

该计划抓住了重塑 Iroha 虚拟机的短期里程碑
(IVM) 进入更清晰的层，同时保留安全性和性能特征。
它侧重于隔离职责，使主机集成更安全，以及
准备 Kotodama 语言堆栈以提取到独立的 crate 中。

## 目标

1. **分层运行时外观** – 引入显式运行时接口，以便 VM
   核心可以嵌入到狭窄的特征后面，并且替代的前端可以发展
   无需接触内部模块。
2. **主机/系统调用边界强化** – 通过路由系统调用调度
   专用适配器，在任何主机之前强制执行 ABI 策略和指针验证
   代码执行。
3. **语言/工具分离** – 将 Kotodama 特定代码移至新 crate 并
   仅将字节码执行面保留在 `ivm` 中。
4. **配置内聚** – 统一加速和功能切换，以便它们
   通过 `iroha_config` 驱动，消除生产中基于环境的旋钮
   路径。

## 阶段分解

### 第 1 阶段 – 运行时外观（正在进行中）
- 添加 `runtime` 模块，该模块定义描述生命周期的 `VmEngine` 特征
  操作（`load_program`、`execute`、主机管道）。
- 教 `IVM` 实现该特征。  这保留了现有的结构，但允许
  消费者（和未来的测试）依赖于接口而不是具体的
  类型。
- 开始放弃从 `lib.rs` 的直接模块重新导出，以便调用者通过
  尽可能使用外观。

**安全/性能影响**：外观限制对内部的直接访问
状态；仅暴露安全入口点。  这使得审核主机变得更容易
关于气体或 TLV 处理的相互作用和原因。

### 第 2 阶段 – 系统调用调度程序
- 引入一个 `SyscallDispatcher` 组件，该组件包装 `IVMHost` 并强制执行 ABI
  策略和指针在一个位置验证一次。
- 迁移默认主机和模拟主机以使用调度程序，删除
  重复的验证逻辑。
- 使调度程序可插拔，以便主机可以提供自定义仪器，而无需
  绕过安全检查。
- 提供 `SyscallDispatcher::shared(...)` 帮助程序，以便克隆的虚拟机可以转发
  通过共享 `Arc<Mutex<..>>` 主机进行系统调用，无需每个工作人员构建
  定制包装纸。

**安全/性能影响**：集中控制可防止主机
忘记调用 `is_syscall_allowed`，它允许将来缓存指针
重复系统调用的验证。

### 第 3 阶段 – Kotodama 提取
- Kotodama 编译器提取到 `crates/kotodama_lang`（来自 `crates/ivm/src/kotodama`）。
- 提供 VM 使用的最小字节码 API (`compile_to_ivm_bytecode`)。

**安全/性能影响**：解耦降低了虚拟机的攻击面
核心并允许语言创新，而不会有解释器回归的风险。### 第 4 阶段 – 配置整合
- 通过 `iroha_config` 预设的线程加速选项（例如，启用 GPU 后端），同时保留现有环境覆盖（`IVM_DISABLE_CUDA`、`IVM_DISABLE_METAL`）作为运行时终止开关。
- 通过新外观公开 `RuntimeConfig` 对象，以便主机选择
  明确的确定性加速政策。

**安全/性能影响**：消除基于环境的切换避免静默
配置漂移并确保跨部署的确定性行为。

## 接下来的步骤

- 通过添加外观特征并更新高级调用站点来完成第一阶段
  依赖它。
- 审核公共再导出，以确保仅使用外观和有意公开的 API
  从板条箱中泄漏出来。
- 在单独的模块中对系统调用调度程序 API 进行原型设计并迁移
  验证后默认主机。

一旦实施，每个阶段的进展将在 `status.md` 中跟踪
正在进行中。