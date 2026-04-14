---
lang: zh-hans
direction: ltr
source: docs/source/kotodama_grammar.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: ac9b1fa221c6de46c139ee3a3c280957adad4910b49015fbb746259a4af22659
source_last_modified: "2026-01-30T12:29:10.190473+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# Kotodama 语言语法和语义

本文档指定了 Kotodama 语言语法（词法分析、语法）、键入规则、确定性语义，以及程序如何使用 Norito 指针 ABI 约定降低为 IVM 字节码 (.to)。 Kotodama 源使用 .ko 扩展名。编译器发出 IVM 字节码 (.to)，并且可以选择返回清单。

内容
- 概述和目标
- 词汇结构
- 类型和文字
- 声明和模块
- 合约容器和元数据
- 功能及参数
- 声明
- 表达式
- 内置函数和指针 ABI 构造函数
- 收藏品和地图
- 确定性迭代和界限
- 错误和诊断
- Codegen 映射到 IVM
- ABI、标头和清单
- 路线图

## 概述和目标

- 确定性：程序必须在硬件上产生相同的结果；没有浮点或不确定源。所有主机交互都是通过带有 Norito 编码参数的系统调用进行的。
- 可移植：目标为 Iroha 虚拟机 (IVM) 字节码，而不是物理 ISA。存储库中可见的类似 RISC-V 的编码是 IVM 解码的实现细节，并且不得改变可观察到的行为。
- 可审计：小而明确的语义；语法到 IVM 操作码和主机系统调用的清晰映射。
- 有界性：无界数据上的循环必须带有显式边界。地图迭代有严格的规则来保证确定性。

## 词汇结构

空白和注释
- 空格分隔标记，否则无关紧要。
- 行注释以 `//` 开始，一直到行尾。
- 块注释 `/* ... */` 不嵌套。

标识符
- 开始：`[A-Za-z_]`，然后继续 `[A-Za-z0-9_]*`。
- 区分大小写; `_` 是一个有效的标识符，但不鼓励使用。

关键词（保留）
- `seiyaku`、`hajimari`、`kotoage`、`kaizen`、`state`、`struct`、`fn`、`let`、 `const`、`return`、`if`、`else`、`while`、`for`、`in`、`break`、 `continue`、`true`、`false`、`permission`、`kotoba`。

运算符和标点符号
- 算术：`+ - * / %`
- 按位：`& | ^ ~`，移位 `<< >>`
- 比较：`== != < <= > >=`
- 逻辑：`&& || !`
- 分配：`= += -= *= /= %= &= |= ^= <<= >>=`
- 其他：`: , ; . :: ->`
- 支架：`() [] {}`文字
- 整数：十进制 (`123`)、十六进制 (`0x2A`)、二进制 (`0b1010`)。所有整数在运行时都是有符号的 64 位；不带后缀的文字通过推理输入或默认输入为 `int`。
- 字符串：带转义的双引号（`\n`、`\r`、`\t`、`\0`、`\xNN`、`\u{...}`、`\"`、 `\\`); UTF-8。原始字符串 `r"..."` 或 `r#"..."#` 禁用转义并允许换行。
- 字节：带转义的 `b"..."`，或原始 `br"..."` / `rb"..."`；产生 `bytes` 文字。
- 布尔值：`true`、`false`。

## 类型和文字

标量类型
- `int`：64位二进制补码；算术对 add/sub/mul 进行模 2^64 换行；除法在 IVM 中定义了有符号/无符号变体；编译器根据语义选择适当的操作。
- `fixed_u128`、`Amount`、`Balance`：由 Norito `Numeric` 支持的数字别名（有符号十进制，最多 512 位尾数和小数位数）。 Kotodama 将这些别名视为非负数；检查算术，保留别名，并捕获溢出或除以零。从 `int` 创建的值使用小数位 0；与 `int` 之间的转换在运行时进行范围检查（非负、整数、适合 i64）。
- `bool`：逻辑真值；降低至 `0`/`1`。
- `string`：不可变的 UTF-8 字符串；传递给系统调用时表示为 Norito TLV；虚拟机内操作使用字节片和长度。
- `bytes`：原始 Norito 有效负载；为散列/加密/证明输入和持久覆盖的指针 ABI `Blob` 类型别名。

复合类型
- `struct Name { field: Type, ... }` 用户定义的产品类型。构造函数在表达式中使用调用语法 `Name(a, b, ...)`。支持字段访问 `obj.field` 并在内部降低为元组样式位置字段。链上持久状态 ABI 采用 Norito 编码；编译器会发出反映结构顺序的覆盖层，并且最近的测试（`crates/iroha_core/tests/kotodama_struct_overlay.rs`）使布局在各个版本中保持锁定。
- `Map<K, V>`：确定性关联图；语义限制迭代和迭代过程中的突变（见下文）。
- `Tuple (T1, T2, ...)`：带有位置字段的匿名产品类型；用于多次返回。

特殊指针 ABI 类型（面向主机）
- `AccountId`、`AssetDefinitionId`、`Name`、`Json`、`NftId`、`Blob` 和类似的不是一流的运行时类型。它们是生成输入区域（Norito TLV 信封）的类型化、不可变指针的构造函数，并且只能用作系统调用参数或在变量之间移动而无需突变。

类型推断
- 本地 `let` 绑定从初始值设定项推断类型。函数参数必须显式键入。除非不明确，否则可以推断返回类型。

## 声明和模块顶级项目
- 合约：`seiyaku Name { ... }` 包含函数、状态、结构和元数据。
- 允许但不鼓励每个文件有多个合同；一个主 `seiyaku` 用作清单中的默认条目。
- `struct` 声明定义合约内的用户类型。

能见度
- `kotoage fn` 表示公共入口点；可见性影响调度程序权限，而不是代码生成。
- 可选访问提示：`#[access(read=..., write=...)]` 可以在 `fn`/`kotoage fn` 之前提供清单读/写密钥。编译器还会自动发出咨询提示；不透明的主机调用会回退到保守的通配符密钥 (`*`) 并显示诊断，除非提供显式访问提示，因此调度程序可以选择动态预传递以获得更细粒度的密钥。

## 合约容器和元数据

语法
```
seiyaku Name {
  meta {
    abi_version: 1,
    vector_length: 0,
    max_cycles: 0,
    features: ["zk", "simd"],
  }

  state int counter;

  hajimari() { counter = 0; }

  kotoage fn inc() { counter = counter + 1; }
}
```

语义学
- `meta { ... }` 字段覆盖已发出的 IVM 标头的编译器默认值：`abi_version`、`vector_length`（0 表示未设置）、`max_cycles`（0 表示编译器默认值）、`features` 切换标头功能位（ZK 跟踪、向量）宣布）。编译器将 `max_cycles: 0` 视为“使用默认值”，并发出配置的非零默认值以满足准入要求。不支持的功能将被忽略并发出警告。当省略 `meta {}` 时，编译器将发出 `abi_version = 1` 并使用其余标头字段的选项默认值。
- `features: ["zk", "simd"]`（别名：`"vector"`）显式请求相应的标头位。未知的特征字符串现在会产生解析器错误而不是被忽略。
- `state` 声明持久合约变量。编译器降低对 `STATE_GET/STATE_SET/STATE_DEL` 系统调用的访问，主机将它们暂存在每个事务覆盖中（检查点/恢复回滚、提交时刷新到 WSV）。针对文字状态路径发出访问提示；动态键回退到映射级冲突键。对于显式主机支持的读/写，请使用 `state_get/state_set/state_del` 帮助程序和 `map.ensure(...)` 映射帮助程序；这些通过 Norito TLV 进行路由并保持名称/字段顺序稳定。
- 保留状态标识符；参数中隐藏 `state` 名称或 `let` 绑定被拒绝 (`E_STATE_SHADOWED`)。
- 状态映射值不是一流的：直接使用状态标识符进行映射操作和迭代。将状态映射绑定或传递给用户定义的函数被拒绝 (`E_STATE_MAP_ALIAS`)。
- 持久状态映射当前仅支持 `int` 和指针 ABI 密钥类型；其他键类型在编译时被拒绝。
- 持久状态字段必须是 `int`、`bool`、`Json`、`Blob`/`bytes` 或指针 ABI 类型（包括由这些字段组成的结构体/元组）； `string` 不支持持久状态。

### 言叶本地化
语法
```
kotoba {
  "E_UNBOUNDED_ITERATION": { en: "Loop over map lacks a bound." }
}
```语义学
- `kotoba` 条目将转换表附加到合同清单（`kotoba` 字段）。
- 消息 ID 和语言标签接受标识符或字符串文字；条目必须非空。
- 重复的 `msg_id` + 语言标记对在编译时被拒绝。

## 触发声明

触发器声明将调度元数据附加到入口点清单并自动注册
当合同实例被激活时（停用时被删除）。它们在一个内部被解析
`seiyaku` 块。

语法
```
register_trigger wake {
  call run;
  on time pre_commit;
  repeats 2;
  authority "sorauロ1Npテユヱヌq11pウリ2ア5ヌヲiCJKjRヤzキNMNニケユPCウルFvオE9LBLB";
  metadata { tag: "alpha"; count: 1; enabled: true; }
}
```

注释
- `call` 必须引用同一合约中的公共 `kotoage fn` 入口点；一个可选的
  清单中记录了 `namespace::entrypoint` 但跨合约回调被拒绝
  目前由运行时（仅限本地回调）。
- 支持的过滤器：`time pre_commit` 和 `time schedule(start_ms, period_ms?)`，以及
  `execute trigger <name>` 用于按调用触发器、`data any` 和管道过滤器
  （`pipeline transaction`、`pipeline block`、`pipeline merge`、`pipeline witness`）。
- `authority` 可选择覆盖触发权限（AccountId 字符串文字）。如果省略，
  运行时使用合约激活权限。
- 元数据值必须是 JSON 文本（`string`、`number`、`bool`、`null`）或 `json!(...)`。
- 运行时注入的触发器元数据键：`contract_namespace`、`contract_id`、
  `contract_entrypoint`、`contract_code_hash`、`contract_trigger_id`。

## 函数和参数

语法
- 声明：`fn name(param1: Type, param2: Type, ...) -> Ret { ... }`
- 公共：`kotoage fn name(...) { ... }`
- 初始化程序：`hajimari() { ... }`（在部署时由运行时调用，而不是由虚拟机本身调用）。
- 升级挂钩：`kaizen(args...) permission(Role) { ... }`。

参数和返回值
- 根据 ABI，参数作为值或 INPUT 指针 (Norito TLV) 在寄存器 `r10..r22` 中传递；额外的参数溢出到堆栈。
- 函数返回零或一个标量或元组。标量的主要返回值位于 `r10` 中；按照惯例，元组在堆栈/输出中具体化。

## 声明- 变量绑定：`let x = expr;`、`let mut x = expr;`（可变性是编译时检查；仅允许本地变量运行时突变）。
- 赋值：`x = expr;` 和复合形式 `x += 1;` 等。目标必须是变量或映射索引；元组/结构字段是不可变的。
- 数字别名（`fixed_u128`、`Amount`、`Balance`）是不同的 `Numeric` 支持类型；算术保留别名，混合别名需要通过 `int` 绑定进行转换。在运行时检查与 `int` 之间的转换（非负、整数、范围限制）。
- 控制：`if (cond) { ... } else { ... }`、`while (cond) { ... }`、C 型 `for (init; cond; step) { ... }`。
  - `for` 初始化器和步骤必须是简单的 `let name = expr` 或表达式语句；复杂的解构被拒绝（`E0005`、`E0006`）。
  - `for` 范围：来自 init 子句的绑定在循环中及其之后可见；在主体或步骤中创建的绑定不会逃脱循环。
- `int`、`bool`、`string`、指针 ABI 标量（例如 `AccountId`、 `Name`、`Blob`/`bytes`、`Json`)；元组、结构体和映射不具有可比性。
- 地图循环：`for (k, v) in map { ... }`（确定性；见下文）。
- 流量：`return expr;`、`break;`、`continue;`。
- 调用：`name(args...);` 或 `call name(args...);`（两者都接受；编译器规范化为调用语句）。
- 断言：`assert(cond);`、`assert_eq(a, b);` 映射到非 ZK 构建中的 IVM `ASSERT*` 或 ZK 模式中的 ZK 约束。

## 表达式

优先级（高→低）
1. 会员/索引：`a.b`、`a[b]`
2. 一元：`! ~ -`
3.乘法：`* / %`
4.添加剂：`+ -`
5.班次：`<< >>`
6. 关系：`< <= > >=`
7. 平等：`== !=`
8. 按位与/异或/或：`& ^ |`
9. 逻辑与/或：`&& ||`
10.三元：`cond ? a : b`

调用和元组
- 调用使用位置参数：`f(a, b, c)`。
- 元组文字：`(a, b, c)` 和解构：`let (x, y) = pair;`。
- 元组解构需要具有匹配数量的元组/结构类型；不匹配会被拒绝。

字符串和字节
- 字符串为 UTF-8；源代码中接受原始字符串和字节文字形式。
- 字节文字 (`b"..."`、`br"..."`、`rb"..."`) 低于 `bytes` (Blob) 指针；当系统调用需要 NoritoBytes TLV 有效负载时，用 `norito_bytes(...)` 包装。

## 内置函数和指针 ABI 构造函数

指针构造函数（将 Norito TLV 发出到 INPUT 并返回类型化指针）
- `account_id(string) -> AccountId*`
- `asset_definition(string) -> AssetDefinitionId*`
- `asset_id(string) -> AssetId*`
- `domain(string) | domain_id(string) -> DomainId*`
- `name(string) -> Name*`
- `json(string) -> Json*`
- `nft_id(string) -> NftId*`
- `blob(bytes|string) -> Blob*`
- `norito_bytes(bytes|string) -> NoritoBytes*`
- `dataspace_id(string|0xhex) -> DataSpaceId*`
- `axt_descriptor(string|0xhex) -> AxtDescriptor*`
- `asset_handle(string|0xhex) -> AssetHandle*`
- `proof_blob(string|0xhex) -> ProofBlob*`Prelude 宏为这些构造函数提供更短的别名和内联验证：
- `account!("<i105-account-id>")`, `account_id!("<i105-account-id>")`
- `asset_definition!("62Fk4FPcMuLvW5QjDGNF2a4jAmjM")`、`asset_id!("62Fk4FPcMuLvW5QjDGNF2a4jAmjM")`
- `domain!("wonderland")`, `domain_id!("wonderland")`
- `name!("example")`
- `json!("{\"hello\":\"world\"}")` 或结构化文字，例如 `json!{ hello: "world" }`
- `nft_id!("dragon$demo")`、`blob!("bytes")`、`norito_bytes!("...")`

这些宏扩展为上面的构造函数，并在编译时拒绝无效的文字。

实施情况
- 已实现：上面的构造函数接受字符串文字参数，下面的构造函数接受放置在 INPUT 区域中的类型化 Norito TLV 信封。它们返回可用作系统调用参数的不可变类型指针。非文字字符串表达式被拒绝；使用 `Blob`/`bytes` 进行动态输入。 `blob`/`norito_bytes` 在运行时也接受 `bytes` 类型的值，无需宏填充程序。
- 扩展形式：
  - `json(Blob[NoritoBytes]) -> Json*` 通过 `JSON_DECODE` 系统调用。
  - `name(Blob[NoritoBytes]) -> Name*` 通过 `NAME_DECODE` 系统调用。
  - 来自 Blob/NoritoBytes 的指针解码：任何指针构造函数（包括 AXT 类型）接受 `Blob`/`NoritoBytes` 有效负载，并降低为具有预期类型 id 的 `POINTER_FROM_NORITO`。
  - 指针形式的传递：`name(Name) -> Name*`、`blob(Blob) -> Blob*`、`norito_bytes(Blob) -> Blob*`。
  - 支持方法糖：`s.name()`、`s.json()`、`b.blob()`、`b.norito_bytes()`。

主机/系统调用内置函数（映射到 SCALL；精确数字在 ivm.md 中）
- `mint_asset(AccountId*, AssetDefinitionId*, numeric)`
- `burn_asset(AccountId*, AssetDefinitionId*, numeric)`
- `transfer_asset(AccountId*, AccountId*, AssetDefinitionId*, numeric)`
- `set_account_detail(AccountId*, Name*, Json*)`
- `execute_instruction(Blob[NoritoBytes])`
- `execute_query(Blob[NoritoBytes]) -> Blob`
- `subscription_bill()`
- `subscription_record_usage()`
- `nft_mint_asset(NftId*, AccountId*)`
- `nft_transfer_asset(AccountId*, NftId*, AccountId*)`
- `nft_set_metadata(NftId*, Json*)`
- `nft_burn_asset(NftId*)`
- `authority() -> AccountId*`
- `register_domain(DomainId*)`
- `unregister_domain(DomainId*)`
- `transfer_domain(AccountId*, DomainId*, AccountId*)`
- `vrf_verify(Blob, Blob, Blob, int variant) -> Blob`
- `vrf_verify_batch(Blob) -> Blob`
- `axt_begin(AxtDescriptor*)`
- `axt_touch(DataSpaceId*, Blob[NoritoBytes]? manifest)`
- `verify_ds_proof(DataSpaceId*, ProofBlob?)`
- `use_asset_handle(AssetHandle*, Blob[NoritoBytes], ProofBlob?)`
- `axt_commit()`
- `contains(Map<K,V>, K) -> bool`

内置实用程序
- `info(string|int)`：通过 OUTPUT 发出结构化事件/消息。
- `hash(blob) -> Blob*`：将 Norito 编码的哈希值返回为 Blob。
- `build_submit_ballot_inline(election_id, ciphertext, nullifier32, backend, proof, vk) -> Blob*` 和 `build_unshield_inline(asset, to, amount, inputs32, backend, proof, vk) -> Blob*`：内联 ISI 构建器；所有参数必须是编译时文字（字符串文字或文字的指针构造函数）。 `nullifier32` 和 `inputs32` 必须恰好为 32 个字节（原始字符串或 `0x` 十六进制），并且 `amount` 必须为非负数。
- `schema_info(Name*) -> Json* { "id": "<hex>", "version": N }`
- `encode_schema(Name*, Json*) -> Blob`：使用主机架构注册表对 JSON 进行编码（除了订单/交易示例之外，DefaultRegistry 还支持 `QueryRequest` 和 `QueryResponse`）。
- `decode_schema(Name*, Blob|bytes) -> Json*`：使用主机架构注册表解码 Norito 字节。
- `pointer_to_norito(ptr) -> NoritoBytes*`：将现有指针 ABI TLV 包装为 NoritoBytes 以进行存储或传输。
- `isqrt(int) -> int`：作为 IVM 操作码实现的整数平方根 (`floor(sqrt(x))`)。
- `min(int, int) -> int`、`max(int, int) -> int`、`abs(int) -> int`、`div_ceil(int, int) -> int`、`gcd(int, int) -> int`、`mean(int, int) -> int` — 由本机 IVM 操作码支持的融合算术助手（ceil 除法陷阱除以零）。注释
- 内置是薄垫片；编译器将它们降低为寄存器移动和 `SCALL`。
- 指针构造函数是纯的：VM 确保 INPUT 中的 Norito TLV 在调用期间不可变。
 - 具有指针 ABI 字段的结构（例如，`DomainId`、`AccountId`）可用于按人体工程学对系统调用参数进行分组。编译器将 `obj.field` 映射到正确的寄存器/值，无需额外分配。

## 收藏和地图

类型：`Map<K, V>`
- 内存中映射（通过 `Map::new()` 堆分配或作为参数传递）存储单个键/值对；键和值必须是字大小的类型：`int`、`bool`、`string`、`Blob`、`bytes`、`Json` 或指针类型（例如，`AccountId`、 `Name`）。
- 持久状态映射 (`state Map<...>`) 使用 Norito 编码的键/值。支持的按键：`int` 或指针类型。支持的值：`int`、`bool`、`Json`、`Blob`/`bytes` 或指针类型。
- `Map::new()` 分配并清零初始化单个内存条目（键/值 = 0）；对于非 `Map<int,int>` 映射，提供显式类型注释或返回类型。
- 状态地图不是一流的值：您无法重新分配它们（例如，`M = Map::new()`）；通过索引更新条目 (`M[key] = value`)。
- 操作：
  - 索引：`map[key]` 获取/设置值（通过主机系统调用执行设置；请参阅运行时 API 映射）。
  - 存在：`contains(map, key) -> bool`（降低的助手；可能是内部系统调用）。
  - 迭代：`for (k, v) in map { ... }`，具有确定性顺序和变异规则。

确定性迭代规则
- 迭代集是循环入口处键的快照。
- 顺序是 Norito 编码密钥的严格字节字典顺序升序。
- 循环期间对迭代映射的结构修改（插入/删除/清除）会导致确定性 `E_ITER_MUTATION` 陷阱。
- 需要有界性：地图上声明的最大值 (`@max_len`)、显式属性 `#[bounded(n)]` 或使用 `.take(n)`/`.range(..)` 的显式边界；否则编译器会发出 `E_UNBOUNDED_ITERATION`。

界限助手
- `#[bounded(n)]`：地图表达式上的可选属性，例如`for (k, v) in my_map #[bounded(2)] { ... }`。
- `.take(n)`：从头开始迭代第一个 `n` 条目。
- `.range(start, end)`：迭代半开区间 `[start, end)` 中的条目。语义相当于 `start` 和 `n = end - start`。关于动态边界的注释
- 文字边界：完全支持 `n`、`start` 和 `end` 作为整数文字，并编译为固定的迭代次数。
- 非文字边界：当在 `ivm` 包中启用 `kotodama_dynamic_bounds` 功能时，编译器接受动态 `n`、`start` 和 `end` 表达式，并插入运行时断言以确保安全（非负、 `end >= start`）。降低会通过 `if (i < n)` 检查发出最多 K 个受保护的迭代，以避免额外的主体执行（默认 K=2）。您可以通过 `CompilerOptions { dynamic_iter_cap, .. }` 以编程方式调整 K。
- 在编译前运行 `koto_lint` 检查 Kotodama lint 警告；主编译器总是在解析和类型检查后继续进行降低。
- 错误代码记录在 [Kotodama 编译器错误代码](./kotodama_error_codes.md) 中；使用 `koto_compile --explain <code>` 进行快速解释。

## 错误和诊断

编译时诊断（示例）
- `E_UNBOUNDED_ITERATION`：地图循环缺少界限。
- `E_MUT_DURING_ITER`：循环体中迭代映射的结构突变。
- `E_STATE_SHADOWED`：本地绑定不能隐藏 `state` 声明。
- `E_BREAK_OUTSIDE_LOOP`：`break` 在循环外部使用。
- `E_CONTINUE_OUTSIDE_LOOP`：`continue` 在循环外部使用。
- `E0005`：for 循环初始值设定项比支持的更复杂。
- `E0006`：for 循环步骤子句比支持的更复杂。
- `E_BAD_POINTER_USE`：在需要第一类类型的情况下使用指针 ABI 构造函数结果。
- `E_UNRESOLVED_NAME`、`E_TYPE_MISMATCH`、`E_ARITY_MISMATCH`、`E_DUP_SYMBOL`。
- 工具：`koto_compile` 在发出字节码之前运行 lint pass；使用 `--no-lint` 跳过或使用 `--deny-lint-warnings` 使 lint 输出的构建失败。

运行时 VM 错误（已选择；完整列表位于 ivm.md 中）
- `E_NORITO_INVALID`、`E_OOB`、`E_UNALIGNED`、`E_SCALL_UNKNOWN`、`E_ASSERT`、`E_ASSERT_EQ`、`E_ITER_MUTATION`。

错误信息
- 诊断携带稳定的 `msg_id`，映射到 `kotoba {}` 转换表（如果可用）中的条目。

## Codegen 映射到 IVM

管道
1. Lexer/Parser 生成 AST。
2. 语义分析解析名称、检查类型并填充符号表。
3. IR 降低为简单的类似 SSA 的形式。
4. IVM GPR 的寄存器分配（根据调用约定，args/ret 为 `r10+`）；溢出到堆栈。
5. 字节码发射：允许混合 IVM 原生编码和 RV 兼容编码；使用 `abi_version`、特征、向量长度和 `max_cycles` 发出的元数据标头。映射亮点
- 算术和逻辑映射到 IVM ALU 操作。
- 分支和控制映射到条件分支和跳转；编译器在有利可图的情况下使用压缩形式。
- 本地内存溢出到VM堆栈；强制对齐。
- 内置寄存器移动和 `SCALL` 的 8 位数字。
- 指针构造函数将 Norito TLV 放入 INPUT 区域并生成它们的地址。
- 断言映射到 `ASSERT`/`ASSERT_EQ`，它捕获非 ZK 执行并在 ZK 构建中发出约束。

决定论约束
- 无FP；没有不确定的系统调用。
- SIMD/GPU加速对字节码不可见，并且必须位相同；编译器不会发出特定于硬件的操作。

## ABI、标头和清单

IVM 编译器设置的头字段
- `version`：IVM 字节码格式版本（主要.次要）。
- `abi_version`：系统调用表和指针 ABI 架构版本。
- `feature_bits`：功能标志（例如，`ZK`、`VECTOR`）。
- `vector_len`：逻辑向量长度（0 → 未设置）。
- `max_cycles`：准入限制和 ZK 填充提示。

清单（可选 sidecar）
- `code_hash`、`abi_hash`、`meta {}` 块的元数据、编译器版本以及可重现性的构建提示。

## 路线图

- **KD-231（2026 年 4 月）：** 添加迭代边界的编译时范围分析，以便循环向调度程序公开有界访问集。
- **KD-235（2026 年 5 月）：** 引入与 `string` 不同的一流 `bytes` 标量，以实现指针构造函数和 ABI 清晰度。
- **KD-242（2026 年 6 月）：** 使用确定性回退扩展功能标志后面的内置操作码集（哈希/签名验证）。
- **KD-247 (Jun2026)：** 稳定错误 `msg_id` 并维护 `kotoba {}` 表中的映射以进行本地化诊断。
### 清单排放

- Kotodama 编译器 API 可以通过 `ivm::kotodama::compiler::Compiler::compile_source_with_manifest` 返回 `ContractManifest` 以及已编译的 `.to`。
- 领域：
  - `code_hash`：编译器计算的用于绑定工件的代码字节的哈希值（不包括 IVM 标头和文字）。
  - `abi_hash`：程序的 `abi_version` 允许的系统调用表面的稳定摘要（请参阅 `ivm.md` 和 `ivm::syscalls::compute_abi_hash`）。
- 可选的 `compiler_fingerprint` 和 `features_bitmap` 保留用于工具链。
- `entrypoints`：导出入口点的有序列表（公共、`hajimari`、`kaizen`），包括其所需的 `permission(...)` 字符串和编译器的尽力读/写关键提示，以便准入逻辑和调度程序可以推断预期的 WSV 访问。
- 清单用于入场时检查和登记；有关生命周期，请参阅 `docs/source/new_pipeline.md`。