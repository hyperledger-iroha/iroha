<!-- Auto-generated stub for Chinese (Simplified) (zh-hans) translation. Replace this content with the full translation. -->

---
lang: zh-hans
direction: ltr
source: docs/source/soracloud/uploaded_private_models.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 97d6a421ce93a0e85be6cc99e828f965c9d8617d0ee27a772a2c9f2f646e77b7
source_last_modified: "2026-03-24T18:59:46.535846+00:00"
translation_last_reviewed: 2026-04-02
translator: machine-google-reviewed
---

# Soracloud 用户上传的模型和私有运行时

本说明定义了 So Ra 用户上传的模型流程应如何落地
现有的 Soracloud 模型平面，无需发明并行运行时。

## 设计目标

添加仅 Soracloud 上传的模型系统，让客户可以：

- 上传自己的模型存储库；
- 将固定模型版本绑定到特工公寓或封闭竞技场团队；
- 使用加密输入和加密模型/状态运行私有推理；和
- 接收公开承诺、收据、定价和审计跟踪。

这不是 `ram_lfe` 功能。 `ram_lfe` 保留通用隐藏功能
子系统记录在 `../universal_accounts_guide.md` 中。上传模型
私有推理应该扩展 Soracloud 的现有模型注册表，
工件、单元能力、FHE 和解密策略表面。

## 可重复使用的现有 Soracloud 表面

当前的 Soracloud 堆栈已经拥有正确的基础对象：- `SoraModelRegistryV1`
  - 权威的每服务模型名称和升级版本状态。
- `SoraModelWeightVersionRecordV1`
  - 版本沿袭、升级、回滚、出处和再现性
    哈希值。
- `SoraModelArtifactRecordV1`
  - 确定性工件元数据已经绑定到模型/权重管道。
- `SoraCapabilityPolicyV1.allow_model_inference`
  - 公寓/服务能力标志应成为强制性的
    公寓绑定到上传的模型。
- `SecretEnvelopeV1` 和 `CiphertextStateRecordV1`
  - 确定性加密字节和密文状态载体。
- `FheParamSetV1`、`FheExecutionPolicyV1`、`FheGovernanceBundleV1`、
  `DecryptionAuthorityPolicyV1` 和 `DecryptionRequestV1`
  - 用于加密执行和受控输出的策略/治理层
    释放。
- 当前Torii型号路线：
  - `/v1/soracloud/model/weight/{register,promote,rollback,status}`
  - `/v1/soracloud/model/artifact/{register,status}`
- 当前 Torii HF 共享租赁路由：
  - `/v1/soracloud/hf/{deploy,status,lease/leave,lease/renew}`

上传的模型路径应该扩展这些表面。它不应该超载
HF 共享租约，并且不应将 `ram_lfe` 重用为模型服务运行时。

## 规范上传合约

Soracloud 的私有上传模型合约应该只承认规范
拥抱脸式模型库：- 所需的基础文件：
  - `config.json`
  - 分词器文件
  - 家庭需要时的处理器/预处理器文件
  - `*.safetensors`
- 在这一里程碑中承认的家庭团体：
  - 具有 RoPE/RMSNorm/SwiGLU/GQA 语义的仅解码器因果 LM
  - LLaVA风格的文本+图像模型
  - Qwen2-VL风格的文本+图像模型
- 在此里程碑中被拒绝：
  - GGUF 作为上传的私有运行时合约
  -ONNX
  - 缺少标记器/处理器资产
  - 不支持的架构/形状
  - 音频/视频多模式套餐

为何签订此合同：

- 它与现有的已占主导地位的服务器端模型布局相匹配
  围绕 safetensors 和 Hugging Face 存储库的生态系统；
- 它让模型平面共享一个确定性标准化路径
  所以Ra，Torii，以及运行时编译；和
- 它避免将本地运行时导入格式（例如 GGUF）与
  Soracloud 私有运行时合同。

HF 共享租约对于共享/公共源导入工作流程仍然有用，但是
私有上传模型路径将加密的编译字节存储在链上，而不是
与从共享源池租赁模型字节相比。

## 分层模型平面设计

### 1. 来源和注册层

扩展当前模型注册表设计而不是替换它：- 添加 `SoraModelProvenanceKindV1::UserUpload`
  - 当前类型（`TrainingJob`、`HfImport`）不足以区分
    模型由 So Ra 等客户端直接上传和标准化。
- 保留 `SoraModelRegistryV1` 作为升级版本索引。
- 保留 `SoraModelWeightVersionRecordV1` 作为沿袭/升级/回滚记录。
- 使用可选的上传私有运行时扩展 `SoraModelArtifactRecordV1`
  参考文献：
  - `private_bundle_root`
  - `chunk_manifest_root`
  - `compile_profile_hash`
  - `privacy_mode`

文物记录仍然是联系出处的确定性锚点，
可再现性元数据和捆绑标识在一起。体重记录
仍然是升级版本的沿袭对象。

### 2. Bundle/Chunk 存储层

为加密的上传模型材料添加一流的 Soracloud 记录：

- `SoraUploadedModelBundleV1`
  - `model_id`
  - `weight_version`
  - `family`
  - `modalities`
  - `runtime_format`
  - `bundle_root`
  - `chunk_count`
  - `plaintext_bytes`
  - `ciphertext_bytes`
  - `compile_profile_hash`
  - `pricing_policy`
  - `decryption_policy_ref`
- `SoraUploadedModelChunkV1`
  - `model_id`
  - `bundle_root`
  - `ordinal`
  - `offset_bytes`
  - `plaintext_len`
  - `ciphertext_len`
  - `ciphertext_hash`
  - 加密的有效负载（`SecretEnvelopeV1`）

确定性规则：- 明文字节在加密前被分成固定的 4 MiB 块；
- 块排序是严格且按序数驱动的；
- 块/根摘要在重播过程中保持稳定；和
- 每个加密分片必须保持在当前 `SecretEnvelopeV1` 以下
  密文上限为 `33,554,432` 字节。

这个里程碑通过块将文字加密字节存储在链状态中
记录。它不会将私有上传模型字节卸载到 SoraFS。

由于链是公开的，上传机密性必须来自真实的
Soracloud 持有的接收者密钥，而不是来自公共派生的确定性密钥
元数据。桌面应该获取广告的上传加密收件人，
使用随机的每次上传捆绑密钥加密块，并仅发布
收件人元数据加上封装的捆绑密钥信封以及密文。

### 3.编译/运行时层

在 Soracloud 下添加专用的私有转换器编译器/运行时层：- 标准化 BFV 支持的确定性低精度编译推理
  现在，因为 CKKS 存在于模式讨论中，但不存在于实现中
  本地运行时；
- 将承认的模型编译成确定性的 Soracloud 私有 IR，涵盖
  嵌入、线性/投影层、注意力矩阵相乘、RoPE、RMSNorm /
  LayerNorm 近似、MLP 块、视觉块投影和
  图像到解码器投影仪路径；
- 使用确定性定点推理：
  - int8 权重
  - int16 激活
  - int32累加
  - 批准的非线性多项式近似

该编译器/运行时独立于 `ram_lfe`。它可以重用 BFV 原语
和Soracloud FHE治理对象，但不是同一个执行引擎
或路线家族。

### 4. 推理/会话层

添加私有运行的会话和检查点记录：- `SoraPrivateCompileProfileV1`
  - `family`
  - `quantization`
  - `opset_version`
  - `max_context`
  - `max_images`
  - `vision_patch_policy`
  - `fhe_param_set`
  - `execution_policy`
- `SoraPrivateInferenceSessionV1`
  - `session_id`
  - `apartment`
  - `model_id`
  - `weight_version`
  - `bundle_root`
  - `input_commitments`
  - `token_budget`
  - `image_budget`
  - `status`
  - `receipt_root`
  - `xor_cost_nanos`
- `SoraPrivateInferenceCheckpointV1`
  - `session_id`
  - `step`
  - `ciphertext_state_root`
  - `receipt_hash`
  - `decrypt_request_id`
  - `released_token`
  - `compute_units`
  - `updated_at_ms`

私人执行意味着：

- 加密的提示/图像输入；
- 加密模型权重和激活；
- 输出的显式解密策略发布；
- 公共运行时间收据和成本核算。

这并不意味着没有承诺或可审计性的隐藏执行。

## 客户的责任

因此 Ra 或其他客户端应该在之前执行确定性本地预处理
上传到 Soracloud：

- 分词器应用程序；
- 将图像预处理为承认的文本+图像族的补丁张量；
- 确定性束归一化；
- 令牌 ID 和图像补丁张量的客户端加密。

Torii 应接收加密输入加上公开承诺，而不是原始提示
文本或原始图像，用于私有路径。

## API 和 ISI 计划保留现有模型注册表路由作为规范注册表层并添加
顶部新的上传/运行时路由：

- `POST /v1/soracloud/model/upload/init`
- `POST /v1/soracloud/model/upload/chunk`
- `POST /v1/soracloud/model/upload/finalize`
- `GET /v1/soracloud/model/upload/encryption-recipient`
- `POST /v1/soracloud/model/compile`
- `POST /v1/soracloud/model/allow`
- `POST /v1/soracloud/model/run-private`
- `GET /v1/soracloud/model/run-status`
- `POST /v1/soracloud/model/decrypt-output`

使用匹配的 Soracloud ISI 支持它们：

- 捆绑注册
- 块追加/完成
- 编制录取通知书
- 私人运行开始
- 检查点记录
- 输出释放

流程应该是：

1. upload/init 建立确定性捆绑会话和预期根；
2. upload/chunk 按顺序附加加密分片；
3.上传/完成密封捆绑根和清单；
4.compile 生成一个绑定到的确定性私有编译配置文件
   承认的捆绑；
5.模型/工件+模型/权重注册表记录引用上传的包
   而不仅仅是培训工作；
6.allow-model将上传的模型绑定到已接纳的公寓
   `allow_model_inference`；
7. run-private 记录会话并发出检查点/收据；
8. 解密输出发布受管制的输出材料。

## 定价和控制平面策略

扩展当前 Soracloud 充电/控制平面行为：- 运行上传模型的公寓需要`allow_model_inference`；
- XOR 中的价格存储、编译、运行时步骤和解密发布；
- 在此里程碑中，对上传模型运行禁用叙述传播；
- 将上传的模型保留在 So Ra 的封闭竞技场和出口门控流程中。

## 授权和绑定语义

上传、编译和运行是独立的功能，应该保持独立
模型飞机。

- 上传模型包不得隐式授权公寓运行它；
- 编译成功不得隐式将模型版本升级为当前版本；
- 公寓绑定应该通过 `allow-model` 风格突变来明确
  记录：
  - 公寓，
  - 型号 ID，
  - 重量版本，
  - 束根，
  - 隐私模式，
  - 签名者/审核顺序；
- 绑定到上传模型的公寓必须已经承认
  `allow_model_inference`；
- 突变路线应继续需要相同的 Soracloud 签名请求
  现有模型/工件/训练路线使用的规则，应该是
  由 `CanManageSoracloud` 或同等明确的授权机构保护
  模型。

这可以防止“我上传了它，因此每个私人公寓都可以运行它”
漂移并保持公寓执行政策明确。

## 状态和审计模型

新记录需要权威的读取和审计表面，而不仅仅是突变
路线。

推荐补充：- 上传状态
  - 通过 `service_name + model_name + weight_version` 或通过
    `model_id + bundle_root`；
- 编译状态
  - 通过`model_id + bundle_root + compile_profile_hash`查询；
- 私人运行状态
  - 通过 `session_id` 查询，其中包含公寓/型号/版本上下文
    回应；
- 解密输出状态
  - 通过 `decrypt_request_id` 查询。

审计应该停留在现有的 Soracloud 全局序列上，而不是
创建第二个每个功能计数器。添加一流的审核事件：

- 上传初始化/完成
- 块附加/密封
- 编译被接受/编译被拒绝
- 公寓模型允许/撤销
- 私有运行开始/检查点/完成/失败
- 输出释放/拒绝

这使得上传的模型活动在相同的权威重播中可见，并且
运营故事作为当前服务、训练、模型权重、模型工件、
HF 共享租赁和公寓审计流。

## 入学配额和州增长限制

仅当准入有界时，链上的文字加密模型字节才可行
积极地。

实现应至少定义确定性限制：

- 每个上传包的最大明文字节数；
- 每个包的最大加密字节数；
- 每个包的最大块数；
- 每个机构/服务的最大并发在线上传会话数；
- 每个服务/公寓窗口的最大编译作业数；
- 每个私人会话的最大保留检查点数量；
- 每个会话的最大输出释放请求。Torii 和核心应拒绝超过之前声明的限制的上传
发生状态放大。限制应该是配置驱动的，其中
适当的，但验证结果必须在同行之间保持确定性。

## 重放和编译器决定论

私有编译器/运行时路径比普通编译器/运行时路径具有更高的确定性负担
服务部署。

所需的不变量：

- 族检测和标准化必须产生稳定的规范束
  在发出任何编译哈希之前；
- 编译配置文件哈希必须绑定：
  - 标准化束根，
  - 家庭，
  - 量化配方，
  - opset版本，
  - FHE参数集，
  - 执行政策；
- 运行时必须避免不确定的内核、浮点漂移和
  硬件特定的减少可能会改变产出或收入
  同行。

在扩展承认的家庭设置之前，先为
每个系列类和锁定编译输出加上运行时收据与黄金
测试。

## 代码之前的剩余设计差距

最大的未解决的实施问题现已缩小为具体的
后端决策：- 精确的上传/分块/请求 DTO 形状和 Norito 模式；
- 用于捆绑/块/会话/检查点查找的世界状态索引键；
- `iroha_config` 中的配额/默认配置放置；
- 模型/工件状态是否应该变得面向版本而不是
  当 `UserUpload` 存在时，以培训工作为导向；
- 公寓丢失时的精确撤销行为
  `allow_model_inference` 或固定模型版本已回滚。

这些是下一个设计到代码的桥梁项目。建筑布局
该功能现在应该稳定了。

## 测试矩阵- 上传验证：
  - 接受规范的 HF safetensors 回购协议
  - 拒绝 GGUF、ONNX、缺少标记器/处理器资产、不受支持
    架构和音频/视频多模式包
- 分块：
  - 确定性丛根
  - 稳定的块排序
  - 精确重建
  - 信封天花板强制执行
- 注册表一致性：
  - 重播下的捆绑/块/工件/权重提升正确性
- 编译器：
  - 仅解码器、LLaVA 型和 Qwen2-VL 型各一个小型夹具
  - 拒绝不支持的操作和形状
- 私人运行时：
  - 加密的微型装置端到端烟雾测试，具有稳定的收据和
    阈值输出释放
- 定价：
  - 上传、编译、运行时步骤和解密的异或费用
- So Ra 整合：
  - 上传、编译、发布、绑定团队、运行封闭竞技场、检查收据、
    保存项目，重新打开，确定性地重新运行
- 安全：
  - 没有出口门旁路
  - 没有叙事自动传播
  - 公寓绑定失败，没有 `allow_model_inference`

## 实现切片1. 添加缺少的数据模型字段和新记录类型。
2. 添加新的 Torii 请求/响应类型和路由处理程序。
3. 添加匹配的 Soracloud ISI 和世界状态存储。
4. 添加确定性捆绑/块验证和链上加密字节
   存储。
5. 添加一个微型 BFV 支持的专用变压器固定装置/运行时路径。
6. 扩展 CLI 模型命令以涵盖上传/编译/私有运行流程。
7. 一旦后端路径具有权威性，即可进行 Land So Ra 集成。