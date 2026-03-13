---
lang: zh-hans
direction: ltr
source: docs/source/crypto/sm_audit_success_checklist.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 624ef9305dc14d477a616923c80445094c692bc6a38d69465f679b54ccd52e92
source_last_modified: "2025-12-29T18:16:35.940844+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

% SM2/SM3/SM4 审核成功标准
% Iroha 加密工作组
% 2026-01-30

# 目的

该清单列出了成功所需的具体标准
完成SM2/SM3/SM4外部审核。应在期间进行审查
启动，在每个状态检查点重新访问，并用于确认退出
为生产验证器启用 SM 签名之前的条件。

# 参与前准备

- [ ] 已签署合同，包括范围、可交付成果、保密性和
      修复支持语言。
- [ ] 审核团队接收存储库镜像访问权限、CI 工件存储桶以及
      `docs/source/crypto/sm_audit_brief.md` 中列出的文档包。
- [ ] 通过每个角色的备份确认联系点
      （加密货币、IVM、平台操作、安全性、文档）。
- [ ] 内部利益相关者就目标发布日期进行协调并冻结窗口。
- [ ] SBOM 导出（`cargo auditable` + CycloneDX）生成并共享。
- [ ] OpenSSL/Tongsuo 构建出处包已准备好
      （源 tarball 哈希、构建脚本、可重复性说明）。
- [ ] 捕获的最新确定性测试输出：
      `scripts/sm_openssl_smoke.sh`、`cargo test -p iroha_crypto sm` 和
      Norito 往返夹具。
- [ ] Torii `/v2/node/capabilities` 广告（通过 `iroha runtime capabilities`）记录，验证 `crypto.sm` 清单字段和加速策略快照。

# 参与执行

- [ ] 启动研讨会在对目标的共同理解下完成，
      时间表和沟通节奏。
- [ ] 收到并分类的每周状态报告；风险登记册已更新。
- [ ] 在发现严重性时在一个工作日内通报调查结果
      为高或严重。
- [ ] 审计团队验证 ≥2 个 CPU 架构（x86_64、
      aarch64）具有匹配的输出。
- [ ] 旁道审查包括恒定时间证明或经验测试
      Rust 和 FFI 路径的证据。
- [ ] 合规性和文件审查确认操作员指南相符
      监管义务。
- [ ] 针对参考实现的差异测试（RustCrypto、
      OpenSSL/Tongsuo）在审计员监督下执行。
- [ ] 毛绒线束评估；在存在差距的地方提供新的种子语料库。

# 修复和退出

- [ ] 所有发现均按严重性、影响、可利用性和
      建议的修复步骤。
- [ ] 高/关键问题得到审计员批准的补丁或缓解措施
      验证；记录残余风险。
- [ ] 审核员提供重新测试验证，证明已解决的问题（差异、测试
      运行，或签名证明）。
- [ ] 交付的最终报告：执行摘要、详细调查结果、方法、
      决定论判决、顺从判决。
- [ ] 内部签署会议总结后续步骤、发布调整、
      和文档更新。
- [ ] `status.md` 更新了审核结果和未完成的补救措施
      后续行动。
- [ ] `docs/source/crypto/sm_program.md` 中捕获的事后分析（教训
      学习了，未来的强化任务）。