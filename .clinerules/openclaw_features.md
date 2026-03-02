# OpenClaw 特点总结

## 什么是 OpenClaw

OpenClaw 是一个**自托管网关**，将您喜爱的聊天应用（WhatsApp、Telegram、Discord、iMessage 等）连接到 AI 编码代理（如 Pi）。您在自己的机器（或服务器）上运行一个 Gateway 进程，它就成为您的消息应用和随时可用的 AI 助手之间的桥梁。

**适用对象**：开发者和高级用户，他们希望拥有一个可以从任何地方发送消息的个人 AI 助手——无需放弃对数据的控制或依赖托管服务。

## 核心特点

### 1. 自托管
- 在您的硬件上运行，遵循您的规则

### 2. 多渠道
- 一个 Gateway 可同时服务 WhatsApp、Telegram、Discord 等多个平台

### 3. 代理原生
- 为编码代理构建，支持工具使用、会话、记忆和多代理路由

### 4. 开源
- MIT 许可证，社区驱动

## 主要功能

### 多渠道网关
- 支持 WhatsApp、Telegram、Discord 和 iMessage，只需一个 Gateway 进程

### 插件渠道
- 通过扩展包添加 Mattermost 等更多渠道

### 多代理路由
- 按代理、工作区或发送者隔离会话

### 媒体支持
- 发送和接收图像、音频和文档

### Web 控制 UI
- 用于聊天、配置、会话和节点的浏览器仪表板

### 移动节点
- 配对 iOS 和 Android 节点，支持 Canvas、相机/屏幕和语音工作流

## 快速开始

### 前置要求
- Node 22+
- API 密钥（推荐 Anthropic）
- 5 分钟时间

### 安装步骤

1. 安装 OpenClaw
   ```bash
   npm install -g openclaw@latest
   ```

2. 初始化并安装服务
   ```bash
   openclaw onboard --install-daemon
   ```

3. 配对 WhatsApp 并启动 Gateway
   ```bash
   openclaw channels login
   openclaw gateway --port 18789
   ```

### 访问仪表板
- 本地默认地址：http://127.0.0.1:18789/

## 配置

配置文件位于：`~/.openclaw/openclaw.json`

示例配置：
```json
{
  channels: {
    whatsapp: {
      allowFrom: ["+15555550123"],
      groups: { "*": { requireMention: true } },
    },
  },
  messages: { groupChat: { mentionPatterns: ["@openclaw"] } },
}
```

## 总结

OpenClaw 提供了一个灵活、自托管的解决方案，让您可以通过常用的聊天应用与 AI 代理交互，同时保持对数据和基础设施的完全控制。