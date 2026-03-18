---
lang: zh-hans
direction: ltr
source: docs/source/crypto/sm_lock_refresh_plan.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 3065571b34a226a5871c4fb68063f9419e48074b20096de215f440bdf54a4e59
source_last_modified: "2025-12-29T18:16:35.943236+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

//！安排 SM 尖峰所需的 Cargo.lock 刷新的过程。

# SM 功能 `Cargo.lock` 刷新计划

`iroha_crypto` 的 `sm` 功能尖峰最初无法完成 `cargo check`，而强制执行 `--locked`。本说明记录了经批准的 `Cargo.lock` 更新的协调步骤，并跟踪该需求的当前状态。

> **2026-02-12 更新：** 最近的验证显示可选的 `sm` 功能现在使用现有锁定文件构建（`cargo check -p iroha_crypto --features sm --locked` 在 7.9 秒冷/0.23 秒热中成功）。依赖项集已包含 `base64ct`、`ghash`、`opaque-debug`、`pem-rfc7468`、`pkcs8`、`polyval`、`primeorder`、`sm2`、 `sm3`、`sm4` 和 `sm4-gcm`，因此不需要立即锁定刷新。请保留以下过程，以备将来出现依赖项或新的可选包时使用。

## 为什么需要刷新
- 尖峰的早期迭代需要添加锁定文件中缺少的可选包。当前的锁定快照已包含 RustCrypto 堆栈（`sm2`、`sm3`、`sm4`、支持编解码器和 AES 帮助程序）。
- 存储库策略仍然阻止机会主义的锁定文件编辑；如果将来需要升级依赖项，则以下过程仍然适用。
- 保留此计划，以便团队可以在引入新的 SM 相关依赖项或现有依赖项需要版本更新时执行受控刷新。

## 建议的协调步骤
1. **在 Crypto WG + Release Eng 同步中提出请求（所有者：@crypto-wg Lead）。**
   - 参考 `docs/source/crypto/sm_program.md` 并注意该功能的可选性质。
   - 确认没有并发的锁文件更改窗口（例如，依赖性冻结）。
2. **准备带锁差异的补丁（所有者：@release-eng）。**
   - 执行 `scripts/sm_lock_refresh.sh`（批准后）以仅更新所需的 crate。
   - 捕获 `cargo tree -p iroha_crypto --features sm` 输出（脚本发出 `target/sm_dep_tree.txt`）。
3. **安全审查（所有者：@security-reviews）。**
   - 验证新的包/版本是否符合审核注册和许可期望。
   - 在供应链跟踪器中记录哈希值。
4. **合并窗口执行。**
   - 提交仅包含锁定文件增量、依赖关系树快照（作为工件附加）和更新的审核注释的 PR。
   - 在合并之前确保 CI 使用 `cargo check -p iroha_crypto --features sm` 运行。
5. **后续任务。**
   - 更新 `docs/source/crypto/sm_program.md` 行动项目清单。
   - 通知 SDK 团队该功能可以使用 `--features sm` 进行本地编译。## 时间表和所有者
|步骤|目标|业主|状态 |
|------|--------|--------|--------|
|请求下次加密货币工作组电话会议的议程位置 | 2025-01-22 |加密货币工作组负责人 | ✅ 已完成（审查结论秒杀可以继续进行，无需刷新）|
|草稿选择性 `cargo update` 命令 + 健全性差异 | 2025-01-24 |发布工程| ⚪ 待机（如果出现新箱子则重新激活）|
|新货箱安全审查| 2025-01-27 |安全评论 | ⚪ 待机（刷新恢复时重新使用审核清单）|
|合并锁定文件更新 PR | 2025-01-29 |发布工程| ⚪ 待机 |
|更新 SM 程序文档清单 |合并后 |加密货币工作组负责人 | ✅ 通过 `docs/source/crypto/sm_program.md` 条目解决 (2026-02-12) |

## 注释
- 将任何未来的刷新限制在上面列出的 SM 相关 crate（以及支持 `rfc6979` 等帮助程序），避免工作区范围内的 `cargo update`。
- 如果任何传递依赖项引入 MSRV 漂移，请在合并之前将其显示出来。
- 合并后，启用临时 CI 作业来监控 `sm` 功能的构建时间。