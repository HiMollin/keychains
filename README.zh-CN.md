# Keychains

[English](README.md)

Keychains 是一款本地优先的桌面密码管理器。密码库保存在你的设备上，敏感数据在写入磁盘前就会完成加密，让你无需依赖云服务，也能简单、安心地管理各类账号。

项目使用 Rust、Tauri 2、Vue 3 和 TypeScript 构建，以原生安全核心承载加密与存储，并提供简洁的桌面操作界面。

## 主要功能

- 管理网站、应用及其他账号，支持新增、查看、编辑和删除
- 按名称、用户名、网址、标签、备注和非敏感自定义字段搜索
- 按收藏、条目类型或标签筛选，并按名称、账号和时间排序
- 添加自定义字段，并单独标记需要保护的敏感字段
- 使用操作系统提供的安全随机数生成密码
- 自动锁定密码库、自动隐藏已显示的密码，并定时清空剪贴板
- 更换主密码时，以事务方式重新加密整个密码库
- 创建由独立密码保护的 `.kcbak` 备份，支持合并导入或完整恢复
- 跟随系统主题，也可手动选择浅色或深色模式

## 安全设计

Keychains 的核心原则是避免让明文密码进入持久化存储：

- 主密码通过 Argon2id 派生为 256 位密钥，默认参数为 64 MiB 内存和 3 次迭代。
- 每个条目都使用 XChaCha20-Poly1305 和独立随机 nonce 加密。条目 ID 与格式版本会作为附加认证数据绑定，防止密文被替换到其他记录。
- SQLite 只保存加密后的条目、密钥派生元数据和非敏感应用设置；WebView 无法直接访问数据库。
- 解锁后的密钥只存在于 Rust 进程内存中，并会在锁定密码库或退出应用时清除。
- 备份使用独立的 Argon2id 派生密钥和 XChaCha20-Poly1305 加密；Keychains 不提供明文导出。
- 应用不会加载远程脚本、字体或网站图标，并通过受限的内容安全策略和 Tauri capabilities 控制权限。

主密码和备份密码都无法找回。请把加密备份保存在可靠的位置，并确保自己能够取回对应密码。

## 本地开发

开发环境需要 Node.js、npm、稳定版 Rust 工具链，以及 Tauri 在对应平台所需的系统依赖。

安装依赖并启动开发版本：

```powershell
npm install
npm run tauri dev
```

运行检查：

```powershell
npm run build
npm test
cargo test --manifest-path src-tauri/Cargo.toml
cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets -- -D warnings
```

构建 Windows NSIS 安装包：

```powershell
npm run tauri build
```

安装包会生成在 `src-tauri/target/release/bundle/nsis/`。

## 密码库位置

Keychains 会把密码库保存在各平台的应用数据目录中：

- Windows：`%APPDATA%\io.github.himollin.keychains\keychains.db`
- macOS：`~/Library/Application Support/io.github.himollin.keychains/keychains.db`
- Linux：`~/.local/share/io.github.himollin.keychains/keychains.db`

请不要在 Keychains 运行时手动编辑或同步数据库文件。需要在设备之间迁移密码库时，请使用应用内的加密备份功能。

## 特别鸣谢

特别鸣谢 Codex。
