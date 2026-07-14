# Keychains

[简体中文](README.zh-CN.md)

Keychains is a local-first password manager for desktop. It keeps your vault on your device and encrypts sensitive data before it is written to disk, giving you a straightforward way to manage credentials without relying on a cloud service.

Built with Rust, Tauri 2, Vue 3, and TypeScript, Keychains combines a native security core with a clean desktop interface.

## Highlights

- Create, view, edit, and delete credentials for websites, apps, and other accounts
- Search names, usernames, URLs, tags, notes, and non-sensitive custom fields
- Filter by favorites, entry type, or tags, with sorting by name, account, and time
- Add custom fields and mark individual fields as sensitive
- Generate passwords with the operating system's secure random number generator
- Automatically lock the vault, hide revealed passwords, and clear the clipboard after a delay
- Change the master password with transactional re-encryption of the vault
- Create password-protected `.kcbak` backups, merge them into an existing vault, or restore a complete vault
- Choose between system, light, and dark themes

## Security

Keychains is designed so that plaintext secrets stay out of persistent storage:

- The master password is processed with Argon2id to derive a 256-bit key. The default parameters use 64 MiB of memory and three iterations.
- Every entry is encrypted with XChaCha20-Poly1305 and a unique random nonce. The entry ID and format version are bound as additional authenticated data to prevent ciphertext from being moved between records.
- SQLite stores encrypted entries, key-derivation metadata, and non-sensitive application settings. The WebView cannot access the database directly.
- The unlocked key exists only in the Rust process and is cleared when the vault is locked or the application exits.
- Backups use a separate Argon2id-derived key and XChaCha20-Poly1305 encryption. Keychains does not provide plaintext exports.
- The application does not load remote scripts, fonts, or website icons, and runs with a restricted Content Security Policy and Tauri capability set.

Master passwords and backup passwords cannot be recovered. Keep an encrypted backup in a safe place and make sure you can access its password.

## Development

You will need Node.js, npm, a stable Rust toolchain, and the platform-specific dependencies required by Tauri.

Install the dependencies and start the development build:

```powershell
npm install
npm run tauri dev
```

Run the checks:

```powershell
npm run build
npm test
cargo test --manifest-path src-tauri/Cargo.toml
cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets -- -D warnings
```

Build the Windows NSIS installer:

```powershell
npm run tauri build
```

The installer will be written to `src-tauri/target/release/bundle/nsis/`.

## Vault location

Keychains stores the vault in the platform's application data directory:

- Windows: `%APPDATA%\io.github.himollin.keychains\keychains.db`
- macOS: `~/Library/Application Support/io.github.himollin.keychains/keychains.db`
- Linux: `~/.local/share/io.github.himollin.keychains/keychains.db`

Do not edit or synchronize the database file while Keychains is running. Use an encrypted backup when moving a vault between devices.
