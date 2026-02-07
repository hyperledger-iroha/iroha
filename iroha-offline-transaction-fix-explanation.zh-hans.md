---
lang: zh-hans
direction: ltr
source: iroha-offline-transaction-fix-explanation.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: af87986c9860f978003331715b6b947a1a02f710a5a35e0e5673198c4c3f4db9
source_last_modified: "2026-02-04T17:27:55.753652+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# Iroha 离线交易修复

**来源分支**：`i23`
**精选状态**：正在进行中

## 概述

该补丁包含离线交易系统的两个相关更改：

1. **错误修复**：致盲因子的标量解码（提交 `b28473cd5`）
2. **新功能**：`OfflineSpendReceiptPayloadEncoder` 适用于 Android SDK

---

## 更改 1：标量解码修复

### 总结

修复了一个严重错误，其中约 87% 的随机生成的致盲因素将被拒绝，导致离线余额证明间歇性失败。

### 问题

离线余额证明系统使用具有随机致盲因子的 Pedersen 承诺：

```
Commitment = value * G + blinding * H
```

原始代码使用 `Scalar::from_canonical_bytes()` ，要求输入小于曲线阶数 `L ≈ 2^252` 。当客户端生成随机 32 字节值时：
- 最大可能值：`2^256 - 1`
- 曲线顺序：`~2^252`
- **只有 ~6.25% 的随机值有效**

### 修复

```rust
// BEFORE (broken - rejects ~87% of random blindings):
let scalar = Scalar::from_canonical_bytes(array);
Option::from(scalar).ok_or(BridgeError::OfflineBlinding)

// AFTER (accepts all random blindings):
let scalar = Scalar::from_bytes_mod_order(array);
Ok(scalar)
```

`from_bytes_mod_order()` 接受任何 32 字节输入并以曲线阶数为模进行减少。这是加密安全的，是 libsodium 和 ed25519-dalek 使用的标准方法。

### 文件已更改

|文件|改变 |
|------|--------|
| `crates/connect_norito_bridge/src/lib.rs` |第 ~679 行：标量解码 |

---

## 更改 2：OfflineSpendReceiptPayloadEncoder

### 总结

添加新的 JNI 绑定来编码 `OfflineSpendReceiptPayload` 以登录 Android SDK。这使得移动应用程序能够在签名之前正确序列化离线支出收据数据。

### 目的

当发件人创建离线支出收据时，他们必须：
1. 构建包含所有交易详细信息的 `OfflineSpendReceiptPayload`
2.序列化为Norito字节
3. 使用支出密钥对字节进行签名
4. 在收据上签名

以前，Android SDK 无法生成正确的签名字节，因为 Norito 序列化很复杂，并且必须与 Iroha 节点所期望的完全匹配。

### API

**Java 类**：`org.hyperledger.iroha.android.offline.OfflineSpendReceiptPayloadEncoder`

```java
public static byte[] encode(
    String txIdHex,           // 32-byte transaction ID as hex (64 chars)
    String fromAccountId,     // Sender AccountId
    String toAccountId,       // Receiver AccountId
    String assetId,           // Full asset ID
    String amount,            // Decimal amount string
    long issuedAtMs,          // Timestamp in milliseconds
    String invoiceId,         // Invoice identifier
    String platformProofJson, // JSON-serialized OfflinePlatformProof
    String certificateJson    // JSON-serialized OfflineWalletCertificate
);
```

**返回**：Norito编码字节准备签名

### 文件已更改

|文件|描述 |
|------|-------------|
| `crates/connect_norito_bridge/src/lib.rs` |新的 `encode_offline_spend_receipt_payload` 函数 + JNI 绑定 |
| `java/iroha_android/.../OfflineSpendReceiptPayloadEncoder.java` | **新** - Java 包装类 |
| `java/iroha_android/.../OfflineSpendReceiptPayloadEncoderTest.java` | **新** - 测试类 |
| `java/iroha_android/core/build.gradle.kts` |添加了 `IROHA_NATIVE_LIBRARY_PATH` 对测试的支持 |
| `java/iroha_android/.../GradleHarnessTests.java` |注册新测试 |

### 实施细节

**生锈面** (`connect_norito_bridge/src/lib.rs`)：
- 解析所有字符串输入（账户 ID、资产 ID、金额）
- 反序列化 JSON 以获取平台证明和证书
- 构造 `OfflineSpendReceiptPayload` 结构体
- 使用 Norito 编解码器 (`to_bytes`) 进行序列化
- 将原始字节返回给 JNI 调用者

**Java 端** (`OfflineSpendReceiptPayloadEncoder.java`)：
- 在调用本机之前验证所有输入
- 加载 `connect_norito_bridge` 本机库
- 提供 `isNativeAvailable()` 检查以进行正常降级
- 对于无效输入抛出 `IllegalArgumentException`
- 如果本机库不可用，则抛出 `IllegalStateException`

### 测试覆盖率**防锈测试**：`encode_offline_spend_receipt_payload_matches_native`
- 使用原生 Rust 类型创建完整的 `OfflineSpendReceipt`
- 调用 `receipt.signing_bytes()` 获取预期字节
- 使用等效的 JSON 输入调用 `encode_offline_spend_receipt_payload()`
- 断言都产生相同的字节

**Java 测试**：`OfflineSpendReceiptPayloadEncoderTest`
- 使用来自 Rust 测试的测试数据调用 `encode()`
- 验证返回的非空字节
- 验证 Norito 标头 (`NRT0`) 是否存在
- 如果本机库不可用，则优雅地跳过
- 在 CI 中将 `IROHA_NATIVE_REQUIRED=1` 设置为失败而不是跳过

---

## 安全考虑

### 标量修复
- **安全**：模块化缩减是标准的加密实践
- **无安全影响**：保留致盲因素随机性
- **与**相同的方法：libsodium、ed25519-dalek、大多数 Ristretto 实现

### 有效负载编码器
- **没有新的加密**：仅序列化，无密钥处理
- **输入验证**：在本机调用之前验证所有参数
- **确定性**：相同的输入总是产生相同的字节

---

## 测试说明

### 生锈测试
```bash
cd /Users/sdi/work/iroha
cargo test encode_offline_spend_receipt_payload_matches_native
```

### Java 测试（需要本机库）
```bash
export IROHA_NATIVE_LIBRARY_PATH=/path/to/libconnect_norito_bridge.so
export IROHA_NATIVE_REQUIRED=1
cd java/iroha_android
./gradlew test --tests "*OfflineSpendReceiptPayloadEncoderTest*"
```

---

## 影响

|组件|之前 |之后 |
|------------|--------|--------|
|离线余额证明 | ~87% 随机失败 | 100% 成功 |
| Android 支出收据签名 |不可能（错误字节）|正确的 Norito 编码 |
| iOS/Swift |已在提交 b28473cd5 中修复 |不适用 |