<!-- Auto-generated stub for Chinese (Simplified) (zh-hans) translation. Replace this content with the full translation. -->

---
lang: zh-hans
direction: ltr
source: docs/source/soracloud/cli_local_control_plane.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 567b63e9b61afaecfa5d85aa60f0348c856557e171559885ffaba45168ce61dc
source_last_modified: "2026-03-26T06:12:11.480025+00:00"
translation_last_reviewed: 2026-04-02
translator: machine-google-reviewed
---

# Soracloud CLI 和控制平面

Soracloud v1 是一个权威的、仅限 IVM 的运行时。

- `iroha app soracloud init` 是唯一的离线命令。它搭起脚手架
  `container_manifest.json`、`service_manifest.json` 和可选模板
  Soracloud 服务的工件。
- 所有其他 Soracloud CLI 命令仅支持网络并且需要
  `--torii-url`。
- CLI 不维护任何本地 Soracloud 控制平面镜像或状态
  文件。
- Torii 直接从公共 Soracloud 状态和突变路线提供服务
  权威的世界状态加上嵌入式 Soracloud 运行时管理器。

## 运行时范围- Soracloud v1 仅接受 `SoraContainerRuntimeV1::Ivm`。
- `NativeProcess` 仍然被拒绝。
- 有序邮箱执行直接运行承认的 IVM 处理程序。
- 水化和物化来自承诺的 SoraFS/DA 内容而不是
  比合成本地快照。
- `SoraContainerManifestV1` 现在携带 `required_config_names` 和
  `required_secret_names`，加上显式 `config_exports`。部署、升级、
  当有效的权威材料集出现时，回滚失败关闭
  不满足那些声明的绑定或者当配置导出的目标是
  非必需的配置或重复的环境/文件目标。
- 承诺的服务配置条目现在已具体化
  `services/<service>/<version>/configs/<config_name>` 作为规范 JSON
  有效负载文件。
- 显式配置环境导出被投影到
  `services/<service>/<version>/effective_env.json`，文件导出为
  物化下
  `services/<service>/<version>/config_exports/<relative_path>`。出口
  值使用引用的配置条目的规范 JSON 有效负载文本。
- Soracloud IVM 处理程序现在可以读取这些权威配置有效负载
  直接通过运行时主机`ReadConfig`面，如此普通
  `query`/`update` 处理程序不需要猜测节点本地文件路径
  使用已提交的服务配置。
- 承诺服务秘密信封现已具体化
  `services/<service>/<version>/secret_envelopes/<secret_name>` 为
  权威的信封文件。- 普通 Soracloud IVM 处理程序现在可以读取那些提交的秘密
  包络直接通过运行时主机 `ReadSecretEnvelope` 表面。
- 遗留的私有运行时后备树现在已从提交同步
  `secrets/<service>/<version>/<secret_name>` 下的部署状态，因此
  旧的原始秘密读取路径和权威控制平面点
  相同的字节。
- 私有运行时 `ReadSecret` 现在解决了权威部署问题
  `service_secrets` 首先且仅回退到旧节点本地
  `secrets/<service>/<version>/...` 未提交时的物化文件树
  所请求的密钥存在服务秘密条目。
- 秘密摄取仍然故意比配置摄取更窄：
  `ReadSecretEnvelope` 是公共安全的普通处理程序合约，而
  `ReadSecret` 仍然仅私有运行时，并且仍然返回已提交的
  信封密文字节而不是明文挂载合约。
- 运行时服务计划现在公开相应的摄取功能
  布尔值加上声明的 `config_exports` 和有效的预计
  环境，因此状态消费者可以判断是否进行了具体的修订
  支持主机配置读取、主机秘密信封读取、私有原始秘密
  读取和显式配置注入，无需从处理程序推断它
  独自上课。

## CLI 命令- `iroha app soracloud init`
  - 仅离线脚手架。
  - 支持 `baseline`、`site`、`webapp` 和 `pii-app` 模板。
- `iroha app soracloud deploy`
  - 在本地验证 `SoraDeploymentBundleV1` 准入规则，签署
    请求，并调用 `POST /v1/soracloud/deploy`。
  - `--initial-configs <path>` 和 `--initial-secrets <path>` 现在可以附加
    权威的内联服务配置/秘密映射以原子方式
    首先部署，以便在首次准入时可以满足所需的绑定。
  - CLI 现在使用以下任一方式对 HTTP 请求进行规范签名
    `X-Iroha-Account`、`X-Iroha-Signature`、`X-Iroha-Timestamp-Ms` 和
    `X-Iroha-Nonce` 对于普通单签名帐户，或
    `X-Iroha-Account` 加 `X-Iroha-Witness` 时
    `soracloud.http_witness_file` 指向多重签名见证 JSON 负载；
    Torii 返回确定性草案交易指令集和
    CLI 然后通过正常的 Iroha 客户端提交真实交易
    车道。
  - Torii 还强制执行 SCR 主机准入上限和故障关闭功能
    在接受突变之前进行检查。
- `iroha app soracloud upgrade`
  - 验证并签署新的捆绑版本，然后调用
    `POST /v1/soracloud/upgrade`。
  - 相同的 `--initial-configs <path>` / `--initial-secrets <path>` 流量是
    可用于升级期间的原子材料更新。
  - 在升级之前，在服务器端运行相同的 SCR 主机准入检查
    承认了。
- `iroha app soracloud status`- 从`GET /v1/soracloud/status`查询权威服务状态。
- `iroha app soracloud config-*`
  - `config-set`、`config-delete` 和 `config-status` 仅支持 Torii。
  - CLI 签署规范的服务配置来源有效负载和调用
    `POST /v1/soracloud/service/config/set`，
    `POST /v1/soracloud/service/config/delete`，和
    `GET /v1/soracloud/service/config/status`。
  - 配置条目保留在权威部署状态并保留
    附加在部署/升级/回滚修订更改中。
  - 当活动修订版仍然声明时，`config-delete` 现在无法关闭
    `container.required_config_names` 中的命名配置。
- `iroha app soracloud secret-*`
  - `secret-set`、`secret-delete` 和 `secret-status` 仅支持 Torii。
  - CLI 签署规范的服务秘密来源有效负载和调用
    `POST /v1/soracloud/service/secret/set`，
    `POST /v1/soracloud/service/secret/delete`，和
    `GET /v1/soracloud/service/secret/status`。
  - 秘密条目作为权威 `SecretEnvelopeV1` 记录保存
    处于部署状态并能承受正常的服务修订更改。
  - 当活动修订版仍然声明时，`secret-delete` 现在无法关闭
    `container.required_secret_names` 中命名的秘密。
- `iroha app soracloud rollback`
  - 签署回滚元数据并调用 `POST /v1/soracloud/rollback`。
- `iroha app soracloud rollout`
  - 签署推出元数据并调用 `POST /v1/soracloud/rollout`。
- `iroha app soracloud agent-*`
  - 所有公寓生命周期、钱包、邮箱和自治命令
    仅支持 Torii。
- `iroha app soracloud training-*`
  - 所有训练作业命令仅受 Torii 支持。- `iroha app soracloud model-*`
  - 所有模型工件、权重、上传模型和私有运行时命令
    仅受 Torii 支持。
  - 上传模型/私有运行时表面现在位于同一个家族中：
    `model-upload-encryption-recipient`、`model-upload-init`、
    `model-upload-chunk`, `model-upload-finalize`, `model-upload-status`,
    `model-compile`, `model-compile-status`, `model-allow`,
    `model-run-private`、`model-run-status`、`model-decrypt-output` 和
    `model-publish-private`。
  - `model-run-private` 现在隐藏草稿然后最终确定运行时握手
    CLI 并返回权威的最终确定后会话状态。
  - `model-publish-private` 现在支持准备好的
    捆绑/分块/最终确定/编译/允许发布计划和更高级别
    文件草案。草案现在带有 `source: PrivateModelSourceV1`，
    它接受 `LocalDir { path }` 或
    `HuggingFaceSnapshot { repo, revision }`。
  - 当使用 `--draft-file` 调用时，CLI 会对声明的源进行规范化
    进入确定性临时树，验证 v1 HF safetensors 合约，
    根据活动确定性地序列化和加密捆绑包
    Torii 上传接收者，将其分成固定大小的加密块，
    可选择通过 `--emit-plan-file` 写入准备好的计划，然后
    执行上传/完成/编译/允许序列。
  - `HuggingFaceSnapshot` 修订是强制性的，必须固定提交
    SHA；类似分支的引用会被拒绝并失败关闭。- v1 中承认的源布局故意变窄：`config.json`，
    标记器资产、一个或多个 `*.safetensors` 分片以及可选
    支持图像的模型的处理器/预处理器元数据。 GGUF、ONNX、
    其他非安全张量权重和任意嵌套自定义布局是
    被拒绝了。
  - 当使用 `--plan-file` 调用时，CLI 仍然消耗已经
    准备好发布计划文档并在计划上传时失败关闭
    收件人不再与权威 Torii 收件人匹配。
  - 有关分层这些路由的设计，请参阅 `uploaded_private_models.md`
    到现有的模型注册表和工件/权重记录中。
- `model-host` 控制平面路线
  - Torii现已公开权威
    `POST /v1/soracloud/model-host/advertise`，
    `POST /v1/soracloud/model-host/heartbeat`，
    `POST /v1/soracloud/model-host/withdraw`，和
    `GET /v1/soracloud/model-host/status`。
  - 这些路由持续存在选择加入验证器主机功能广告
    权威的世界状态，让运营商检查哪些验证者是
    目前广告模特-主持人能力。
  - `iroha app soracloud model-host-advertise`，
    `model-host-heartbeat`、`model-host-withdraw` 和
    `model-host-status` 现在签署了与
    原始API并直接调用匹配的Torii路由。
- `iroha app soracloud hf-*`
  - `hf-deploy`、`hf-status`、`hf-lease-leave` 和 `hf-lease-renew` 是
    仅支持 Torii。- `hf-deploy` 和 `hf-lease-renew` 现在也自动承认确定性
    为请求的 `service_name` 生成 HF 推理服务，以及
    自动承认确定性生成的 HF 单元
    `apartment_name` 当请求时，在共享租约突变之前
    已提交。
  - 重用将失败关闭：如果指定的服务/公寓已存在，但已存在
    不是该规范源预期生成的高频部署，高频
    突变被拒绝，而不是默默地将租约绑定到不相关的
    Soracloud 对象。
  - 当附加嵌入式运行时管理器时，`hf-status` 现在也
    返回规范源的运行时投影，包括绑定
    服务/公寓、排队的下一窗口可见性和本地捆绑/
    工件缓存未命中； `importer_pending` 遵循运行时投影
    而不是仅仅依赖权威的源枚举。
  - 当 `hf-deploy` 或 `hf-lease-renew` 承认生成的 HF 服务时
    与共享租赁突变相同的交易，权威的 HF
    源现在立即翻转到 `Ready` 并且 `importer_pending` 保持不变
    响应中的 `false`。
  - HF 租赁状态和突变响应现在也暴露任何权威
    放置快照已附加到活动租用窗口，包括分配的主机、合格主机数、热主机数和单独的主机数
    存储与计算费用字段。
  - `hf-deploy` 和 `hf-lease-renew` 现在派生规范的 HF 资源
    在提交之前，从已解析的 Hugging Face 存储库元数据中获取配置文件
    突变：
    - Torii 检查存储库 `siblings`，更喜欢 `.gguf`
      `.safetensors` 通过 PyTorch 权重布局，将所选文件引导至
      派生 `required_model_bytes`，并将其映射到第一个版本
      后端/格式加上 RAM/磁盘层；
    - 当没有实时验证器主机广告时，租赁准入失败关闭
      满足该配置文件；和
    - 当主机集可用时，活动窗口现在会记录
      确定性权益加权放置和单独的计算预留
      费用与现有的存储租赁会计一起。
  - 后来加入活跃 HF 窗口的会员现在按比例支付存储费用，
    仅计算剩余窗口的份额，而较早的成员
    获得相同的确定性存储退款和计算退款会计
    从那晚加入。
  - 嵌入式运行时管理器可以综合生成的 HF 存根包
    本地，因此这些生成的服务无需等待即可实现
    仅为占位符推理包提交了 SoraFS 有效负载。- 嵌入式运行时管理器现在还导入列入白名单的 Hugging Face 存储库
    文件到 `soracloud_runtime.state_dir/hf_sources/<source_id>/files/` 和
    保留本地 `import_manifest.json` 以及已解析的提交，导入
    文件、跳过的文件以及任何导入器错误。
  - 生成的 HF `metadata` 本地读取现在返回本地导入清单，
    包括导入的文件清单以及是否本地执行和
    为该节点启用了网桥回退。
  - 生成的 HF `infer` 本地读取现在更喜欢在节点上执行
    导入的共享字节：
    - `irohad` 在本地实现嵌入式Python适配器脚本
      Soracloud运行时状态目录并通过以下方式调用它
      `soracloud_runtime.hf.local_runner_program`；
    - 嵌入式运行器首先检查确定性的固定装置节
      `config.json`（用于测试），然后加载导入的源
      目录通过`transformers.pipeline(..., local_files_only=True)`所以
      该模型针对共享本地导入执行，而不是拉取
      新的集线器字节；和
    - 如果 `soracloud_runtime.hf.allow_inference_bridge_fallback = true` 和
      配置了`soracloud_runtime.hf.inference_token`，运行时间下降
      仅在本地执行时返回配置的 HF 推理基本 URL
      不可用或失败，并且调用者明确选择加入
      `x-soracloud-hf-allow-bridge-fallback: 1`、`true` 或 `yes`。
  - 运行时投影现在将 HF 源保留在 `PendingImport` 中，直到存在成功的本地导入清单，导入器失败表现为
    运行时 `Failed` 加 `last_error`，而不是默默报告 `Ready`。
  - 生成的 HF 公寓现在消耗经过批准的自治运行
    节点本地运行时路径：
    - `agent-autonomy-run` 现在遵循两步流程：首先签名
      突变记录权威批准并返回确定性
      草案交易，然后第二个签名的最终确定请求询问
      嵌入式运行时管理器来执行已批准的边界运行
      生成HF `/infer`服务并返回任何权威后续
      作为另一个确定性草案的指示；
    - 批准的运行记录现在也保持规范
      `request_commitment`，所以后面生成的服务回执即可
      绑定到确切的权威自主批准；
    - 批准现在可以保留可选的规范 `workflow_input_json`
      身体；如果存在，嵌入式运行时会转发确切的 JSON 负载
      到生成的 HF `/infer` 处理程序，当不存在时，它会回退到
      旧的 `run_label`-as-`inputs` 信封带有权威
      `artifact_hash` / `provenance_hash` / `budget_units` / `run_id` 携带
      作为结构化参数；
    - `workflow_input_json` 现在也可以选择确定性顺序多步执行
      `{ "workflow_version": 1, "steps": [...] }`，其中每个步骤运行一个
      生成的 HF `/infer` 请求和后续步骤可以参考之前的输出
      通过 `${run.*}`、`${previous.text|json|result_commitment}` 和
      `${steps.<step_id>.text|json|result_commitment}` 占位符；和
    - 突变反应和 `agent-autonomy-status` 现在都浮出水面
      节点本地执行摘要（如果可用），包括成功/失败，
      绑定服务修订、确定性结果承诺、检查点
      / 日志工件哈希值、生成的服务 `AuditReceipt` 以及
      解析的 JSON 响应正文。
    - 当生成的服务收据存在时，Torii 将其记录到
      权威的 `soracloud_runtime_receipts` 并公开了结果
      关于最近运行状态的权威运行时收据以及
      节点本地执行摘要。
    - 生成的 HF 自主路径现在还记录了专门的权威
      公寓 `AutonomyRunExecuted` 审核事件和最近运行状态
      返回执行审核以及权威的运行时收据。
  - `hf-lease-renew` 现在有两种模式：
    - 如果当前窗口已过期或耗尽，它会立即打开一个新窗口
      窗户；
    - 如果当前窗口仍然处于活动状态，它将调用者作为调用者排队
      下一窗口赞助商，收取全部下一窗口存储和计算费用预先预订费用，坚持确定性的下一窗口
      安置计划，并通过 `hf-status` 公开排队的赞助
      直到后来的突变使池向前滚动。
  - 生成的 HF 公共 `/infer` 入口现在解析权威
    放置，并且当接收节点不是热主节点时，代理
    通过 Soracloud P2P 控制消息向指定的主主机请求；
    嵌入式运行时在直接副本/未分配的本地上仍然无法关闭
    执行和生成的 HF 运行时收据带有 `placement_id`，
    验证者，以及来自权威放置记录的同行归因。
  - 当代理到主路径超时、在响应之前关闭时，或者
    返回权威机构的非客户端运行时故障
    主节点，入口节点现在报告 `AssignedHeartbeatMiss`
    Primary 和 Enqueues `ReconcileSoracloudModelHosts` 通过相同的
    内部突变泳道。
  - 权威的过期主机协调记录现在保留
    模型主机违规证据，重用公共通道验证器斜线路径，
    并应用默认的 HF 共享租赁惩罚政策：
    `warmup_no_show_slash_bps=500`，
    `assigned_heartbeat_miss_slash_bps=250`，
    `assigned_heartbeat_miss_strike_threshold=3`，和
    `advert_contradiction_slash_bps=1000`。
  - 本地分配的 HF 运行时健康状况现在也提供相同的证据路径：本地 `Warming` 主机上的协调时间导入/预热失败
    `WarmupNoShow`，以及本地暖主节点上的常驻工作人员故障
    通过正常方式发出节流 `AssignedHeartbeatMiss` 报告
    交易队列。
  - 现在协调还可以预启动并调查当地的常驻高频工人
    分配的热主机/热主机，包括副本，因此副本可能会失败
    在任何之前关闭权威 `AssignedHeartbeatMiss` 路径
    公共 `/infer` 请求曾经到达主数据库。
  - 当本地探测成功时，运行时现在也会发出一个
    本地验证器的权威 `model-host-heartbeat` 突变
    分配的主机仍然是 `Warming` 或者活动主机广告需要 TTL
    刷新，因此成功的本地准备可以促进同样的权威
    手动心跳将更新的放置/广告状态。
  - 当运行时发出本地 `WarmupNoShow` 或
    `AssignedHeartbeatMiss`，它现在也入队
    `ReconcileSoracloudModelHosts` 通过相同的内部突变泳道所以
    权威的故障转移/回填立即开始，而不是等待
    稍后定期进行主机到期扫描。
  - 当公共生成的 HF 入口甚至更早失败时，因为已提交
    放置没有可代理的热主节点，Torii 现在询问运行时
    处理排队相同的权威`ReconcileSoracloudModelHosts` 立即指令而不是等待
    用于稍后到期或工作失败信号。
  - 当公共生成的 HF 入口确实收到代理成功响应时，
    Torii 现在验证包含的运行时收据仍然证明执行
    积极安置的承诺热情初选；缺失或不匹配
    展示位置归因现在无法关闭，并暗示相同的权威
    `ReconcileSoracloudModelHosts` 路径而不是返回
    非权威回应。 Torii 现在也拒绝代理成功
    当运行时接收承诺或认证策略执行时的响应
    与即将返回的响应不匹配，以及相同的错误收据
    路径还提供远程主 `AssignedHeartbeatMiss` 报告
    钩子。
  - 代理生成的 HF 执行失败现在请求相同的
    上报后权威`ReconcileSoracloudModelHosts`路径
    远程主要健康故障，而不是等待稍后到期
    扫。
  - Torii 现在将每个挂起的生成的 HF 代理请求绑定到
    它所针对的权威主要同行。来自错误的代理响应
    对等点现在被忽略，而不是毒害待处理的请求，因此仅
    权威主节点可以完成或失败该请求。代理
    来自具有不受支持的代理响应模式的预期对等方的响应版本仍然失败关闭而不是被接受只是因为它
    `request_id` 匹配待处理的请求。如果回答错误的同行
    本身仍然是该放置的分配的 generated-HF 主机，
    运行时现在通过现有的报告主机
    `WarmupNoShow` / `AssignedHeartbeatMiss` 证据路径基于其
    权威分配状态，也暗示权威
    `ReconcileSoracloudModelHosts`，所以过时的主/副本权限漂移
    为控制循环提供信号，而不是仅在入口处被忽略。
  - 传入的 Soracloud 代理执行现在也仅限于预期的
    已提交的热主数据库上生成的 HF `infer` 查询案例。非高频
    公共本地读取路由和生成的 HF 请求传递到节点
    这不再是权威的暖主节点，现在改为失败关闭
    通过 P2P 代理路径执行。权威初选现在也
    在执行前重新计算规范生成的 HF 请求承诺，
    因此伪造或不匹配的代理信封无法关闭。
  - 当指定的副本或过时的前主节点拒绝传入的副本时
    generated-HF代理执行，因为它不再是权威的
    温暖的主要，接收端运行时现在也提示
    `ReconcileSoracloudModelHosts` 而不是仅依赖调用方
    路由视图。- 当相同的传入生成的 HF 代理权限失败发生时
    本地权威主节点本身，运行时现在将其视为
    一流的宿主健康信号：温暖的初选自我报告
    `AssignedHeartbeatMiss`，变暖初选自我报告 `WarmupNoShow`，
    并且两条路径立即重用相同的权威
    `ReconcileSoracloudModelHosts` 控制回路。
  - 当同一个非主要接收者仍然是权威接收者之一时
    分配的主机并可以从提交的链状态解析热主节点，
    它现在将生成的 HF 请求重新代理到该主节点
    立即失败。未分配的验证器失败关闭而不是执行
    作为通用中间 HF 代理跃点，原始入口节点仍然
    根据权威放置验证返回的运行时收据
    状态。如果指定副本到主节点的前向跳跃在
    请求实际上被调度，接收方运行时报告
    远程主健康故障及权威提示
    `ReconcileSoracloudModelHosts`；如果本地分配的副本甚至不能
    尝试向前跳跃，因为它自己的代理传输/运行时丢失，
    该故障现在被视为本地分配主机故障，而不是
    归咎于初级。
  - 协调现在也会在以下情况下自动发出 `AdvertContradiction`本地验证器配置的运行时对等 ID 与
    该验证器的权威 `model-host-advertise` 对等 ID。
  - 有效的模型主机重新通告突变现在也同步权威
    分配主机 `peer_id` / `host_class` 元数据并重新计算当前
    主机类别变更时的安置预订费用。
  - 矛盾的模型宿主重新通告突变现在会立即发出
    `AdvertContradiction` 证据，应用现有验证器斜线/驱逐
    路径，并刷新受影响的展示位置，而不仅仅是验证失败。
  - 剩余的 HF 托管工作现在是：
    - 超出本地范围的更广泛的跨节点/运行时集群健康信号
      验证者的直接工作/预热观察加上分配的主机
      当远程对等内部健康状况应该时，接收方权限失败
      还提供权威的重新平衡/斜线路径。
  - 生成的 HF 本地执行现在保留一个常驻的每个源 Python 工作线程
    在 `irohad` 下存活，在重复的 `/infer` 中重用加载的模型
    调用，并在本地导入时确定性地重新启动该工作程序
    明显变化或进程退出。
  - 这些路由不是私有上传模型路径。 HF 共享租赁保留
    专注于共享源/导入成员资格而不是加密的链上
    私有模型字节。- 生成的 HF 自主批准现在支持确定性顺序
    多步骤请求范围，但更广泛的非线性/工具使用
    编排和工件图执行仍然是后续工作
    超越链接的 `/infer` 步骤。

## 状态语义

`/v1/soracloud/status` 和相关代理/训练/模型状态端点现在
反映权威的运行时状态：

- 承认来自承诺的世界状态的服务修订；
- 来自嵌入式运行时管理器的运行时水合/物化状态；
- 真实邮箱执行回执和失败状态；
- 出版的期刊/检查点工件；
- 缓存和运行时运行状况而不是占位符状态垫片。

如果权威运行时材料已过时或不可用，则读取将失败关闭
而不是退回到本地状态镜像。

`/v1/soracloud/status` 是 v1 中唯一记录的 Soracloud 状态端点。
没有单独的 `/v1/soracloud/registry` 路由。

## 移除本地脚手架

这些旧的本地模拟概念在 v1 中不再存在：

- CLI 本地注册表/状态文件或注册表路径选项
- Torii-本地文件支持的控制平面镜像

## 示例

```bash
iroha app soracloud deploy \
  --container container_manifest.json \
  --service service_manifest.json \
  --torii-url http://127.0.0.1:8080 \
  --api-token <token-if-required> \
  --timeout-secs 10
```

## 注释- 在签名和提交请求之前，本地验证仍会运行。
- 标准 Soracloud 突变端点不再接受原始 `authority` /
  `private_key` 用于部署、升级、回滚、推出、代理的 JSON 字段
  生命周期、训练、模型宿主和模型权重路径； Torii 验证
  这些请求来自规范的 HTTP 签名标头。
- 多重签名控制的 Soracloud 所有者现在使用 `X-Iroha-Witness`；点
  `soracloud.http_witness_file` 位于您希望 CLI 执行的确切见证 JSON
  重放下一个突变请求，如果以下情况，Torii 将失败关闭：
  见证主体帐户或规范请求哈希不匹配。
- `hf-deploy` 和 `hf-lease-renew` 现在包括客户端签名的辅助
  确定性生成的 HF 服务/公寓工件的来源，
  因此 Torii 不再需要调用者私钥来承认那些后续操作
  对象。
- `agent-autonomy-run` 和 `model/run-private` 现在使用草稿然后定稿
  流程：第一个签名的突变记录了权威批准/开始，
  第二个签名的完成请求执行运行时路径并返回
  任何权威的后续指示作为确定性草案
  交易。
- `model/decrypt-output` 现在返回权威的私有推理
  检查点作为确定性草案交易，仅由外部签名交易，而不是通过嵌入的 Torii 持有的私钥。
- ZK 附件 CRUD 现在对已签名的 Iroha 帐户进行密钥租用，并且仍然有效
  启用后，将 API 令牌视为额外的访问门。
- 公共 Soracloud 本地读取入口现在应用明确的每 IP 速率，并且
  并发限制并在本地或之前重新检查公共路由可见性
  代理执行。
- 私有运行时功能实施发生在 Soracloud 主机 ABI 内部，
  不在 CLI 或 Torii 本地脚手架内。
- `ram_lfe` 仍然是一个单独的隐藏功能子系统。用户上传的私有
  变压器执行应重用 Soracloud FHE/解密治理和
  模型注册表，而不是 `ram_lfe` 请求路径。
- 运行时健康状况、水合作用和执行情况来自
  `[soracloud_runtime]` 配置和提交状态，而不是环境
  切换。