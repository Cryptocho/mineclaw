# RFC: API Key 本地加密存储方案

## 1. 背景与目标
目前 `mineclaw.toml` 中直接明文存储 API Key，虽然 API 层已做脱敏，但只要能够访问服务器文件系统的人（或不慎提交到 Git），都能直接获取 Key。
本方案旨在通过**本地加密存储**（Encryption at Rest）来缓解此风险。

## 2. 核心设计
采用类似 Ruby on Rails 的 `master.key` 机制 + AES-GCM 对称加密。

### 2.1 密钥管理 (Master Key)
- 在 `config/` 目录下生成一个随机的 32 字节密钥文件 `master.key`。
- 该文件**必须**被添加到 `.gitignore`，严禁提交到版本控制系统。
- 仅持有该密钥文件的实例才能解密配置文件中的敏感信息。

### 2.2 加密流程 (Write/Save)
当程序保存配置 (`Config::save`) 时：
1. 检查 `llm.api_key` 是否以 `ENC:` 前缀开头。
2. 如果不是（即明文），则使用 `master.key` 对其进行 AES-256-GCM 加密。
3. 将密文编码为 Base64，并添加前缀：`ENC:<base64_ciphertext>`。
4. 将加密后的字符串写入 `mineclaw.toml`。

**示例：**
```toml
[llm]
# 存储状态
api_key = "ENC:U2FsdGVkX1+..." 
```

### 2.3 解密流程 (Read/Load)
当程序加载配置 (`Config::load`) 时：
1. 读取 `mineclaw.toml`。
2. 检查 `llm.api_key` 是否以 `ENC:` 开头。
3. 如果是，读取 `config/master.key` 进行解密。
4. 解密失败（如密钥不匹配）则报错并拒绝启动。
5. 解密成功后，内存中持有明文 Key，供 LLM Client 使用。

## 3. 实现细节

### 3.1 依赖库
- `aes-gcm`: 高性能且安全的对称加密算法。
- `rand`: 生成随机密钥和 Nonce。
- `base64`: 密文编码。

### 3.2 兼容性与迁移
- **平滑升级**：程序启动时若发现 `api_key` 是明文且存在 `master.key`，可自动将其加密并重写配置文件（Auto-Migration）。
- **首次启动**：若无 `master.key`，自动生成一个。

### 3.3 API 交互变化
- **GET /api/config**: 依然返回 `sk-****`（脱敏），不返回密文 `ENC:...`，也不返回明文。
- **PUT /api/config**: 
    - 用户输入明文 Key（如 `sk-new-key`）。
    - 内存中更新为明文。
    - 保存时自动触发加密逻辑。
    - 用户输入 `sk-****`（掩码）或空值，则保留内存中的原值（无论是明文还是解密后的明文）。

## 4. 优缺点评估

**优点**：
- **防泄漏**：即使 `mineclaw.toml` 被误传到 GitHub，没有 `master.key` 也无法解密。
- **透明性**：对上层业务逻辑透明，API 使用者无感知。
- **运维友好**：只需管理一个 `master.key` 文件。

**缺点**：
- **文件依赖**：如果丢失 `master.key`，所有加密配置将不可用（需重新配置）。
- **本地攻击**：如果攻击者拿到了服务器的文件系统权限（同时能读 toml 和 key），依然能解密（这是所有本地加密的通病，除非引入硬件加密机/HSM）。

## 5. 替代方案对比
- **系统 Keyring** (Windows DPAPI/macOS Keychain): 安全性更高，但跨平台依赖复杂，且 Headless 环境（无桌面）下可能无法使用。
- **环境变量**: 最安全，但对用户配置体验（尤其是 Windows 用户）不够友好。

**结论**：推荐采用 **Master Key + AES 文件加密** 方案，兼顾安全性与易用性。
