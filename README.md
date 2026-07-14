# Keychains

Keychains 是一个使用 Rust、Tauri 2、Vue 3 和 TypeScript 构建的本地密码管理器。账号数据不会上传到网络，数据库中的密码条目始终以密文保存。

## 功能

- 网站、应用及其他账号的新增、查看、编辑和删除
- 按名称、用户名、网址、标签、备注及非敏感自定义字段检索
- 收藏、类型和标签筛选，以及名称、账号和时间排序
- 可标记为敏感的自定义字段
- 使用系统安全随机数的密码生成器
- 自动锁定、密码自动隐藏和剪贴板定时清除
- 主密码更换与事务式重新加密
- 使用独立密码保护的 `.kcbak` 备份、合并导入及整体恢复
- 跟随系统、浅色和深色三种主题

## 安全设计

- 主密码通过 Argon2id 派生 256 位密钥，默认使用 64 MiB 内存、3 次迭代。
- 每个条目使用 XChaCha20-Poly1305 和独立随机 nonce 加密；条目 ID 与格式版本通过 AAD 绑定，防止密文被替换到其他记录。
- SQLite 只保存密文、KDF 元数据和非敏感应用设置，WebView 没有直接访问数据库的能力。
- 解锁密钥只存在于 Rust 进程内存，锁定或退出后清除。
- 备份使用独立的 Argon2id 密钥与 XChaCha20-Poly1305 加密；不提供明文导出。
- 应用不加载远程脚本、字体或网站图标，并使用受限 Tauri capabilities 和 CSP。

主密码和备份密码都无法找回。请保留可靠的加密备份，并妥善保存对应密码。

## 本地开发

要求：Node.js、npm、Rust stable，以及 Tauri 对应平台的系统依赖。

```powershell
npm install
npm run tauri dev
```

检查与测试：

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

构建产物位于 `src-tauri/target/release/bundle/nsis/`。

## 数据位置

密码库使用 Tauri 的应用数据目录：

- Windows：`%APPDATA%\io.github.himollin.keychains\keychains.db`
- macOS：`~/Library/Application Support/io.github.himollin.keychains/keychains.db`
- Linux：`~/.local/share/io.github.himollin.keychains/keychains.db`

请不要在应用运行时手动编辑或同步数据库文件；跨设备迁移应使用应用内的加密备份功能。

## 当前边界

首版不包含云同步、浏览器扩展、网页自动填充、多用户共享、系统凭据免密解锁、明文 CSV 导出或在线更新。Windows 为首个完整验证平台，代码和数据格式保持跨平台。
