use crate::{
    crypto::{self, KdfParams, KEY_LEN, SALT_LEN},
    error::{AppError, AppResult},
    models::{
        AppSettings, BackupPreview, CustomField, Entry, EntryInput, EntrySummary, EntryType,
        EntryTypeInput, ImportMode, ImportReport, ListOptions, PasswordOptions, SortOrder,
        VaultState,
    },
};
use base64::{engine::general_purpose::STANDARD as BASE64, Engine as _};
use chrono::Utc;
use rand::{rngs::OsRng, seq::SliceRandom, Rng};
use rusqlite::{params, Connection, OptionalExtension, Transaction};
use serde::{Deserialize, Serialize};
use std::{
    collections::HashSet,
    fs,
    path::{Path, PathBuf},
    time::{Duration, Instant},
};
use uuid::Uuid;
use zeroize::Zeroizing;

const VAULT_VERSION: i64 = 1;
const VERIFIER: &[u8] = b"keychains-vault-v1";
const VERIFIER_AAD: &[u8] = b"keychains:verifier:v1";
const BACKUP_MAGIC: &str = "KEYCHAINS-BACKUP";
const BACKUP_AAD: &[u8] = b"keychains:backup:v1";
const MAX_BACKUP_BYTES: u64 = 256 * 1024 * 1024;

type EncryptedRow = (String, Vec<u8>, Vec<u8>);

#[derive(Debug)]
struct VaultMetadata {
    salt: Vec<u8>,
    kdf: KdfParams,
    verifier_nonce: Vec<u8>,
    verifier_ciphertext: Vec<u8>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct BackupEnvelope {
    magic: String,
    version: u32,
    kdf: KdfParams,
    salt: String,
    nonce: String,
    ciphertext: String,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct BackupPayload {
    entries: Vec<Entry>,
    entry_types: Vec<EntryType>,
    settings: AppSettings,
    exported_at: i64,
}

pub struct VaultService {
    db_path: PathBuf,
    key: Option<Zeroizing<[u8; KEY_LEN]>>,
    last_activity: Option<Instant>,
}

impl VaultService {
    pub fn new(db_path: PathBuf) -> AppResult<Self> {
        if let Some(parent) = db_path.parent() {
            fs::create_dir_all(parent)?;
        }
        let service = Self {
            db_path,
            key: None,
            last_activity: None,
        };
        service.ensure_schema()?;
        Ok(service)
    }

    fn connection(&self) -> AppResult<Connection> {
        let connection = Connection::open(&self.db_path)?;
        connection.pragma_update(None, "foreign_keys", "ON")?;
        connection.pragma_update(None, "journal_mode", "WAL")?;
        connection.pragma_update(None, "synchronous", "FULL")?;
        Ok(connection)
    }

    fn ensure_schema(&self) -> AppResult<()> {
        let connection = self.connection()?;
        connection.execute_batch(
            "
            CREATE TABLE IF NOT EXISTS vault_meta (
                id INTEGER PRIMARY KEY CHECK (id = 1),
                version INTEGER NOT NULL,
                salt BLOB NOT NULL,
                argon_memory_kib INTEGER NOT NULL,
                argon_iterations INTEGER NOT NULL,
                argon_parallelism INTEGER NOT NULL,
                verifier_nonce BLOB NOT NULL,
                verifier_ciphertext BLOB NOT NULL
            );
            CREATE TABLE IF NOT EXISTS entries (
                id TEXT PRIMARY KEY,
                nonce BLOB NOT NULL,
                ciphertext BLOB NOT NULL
            );
            CREATE TABLE IF NOT EXISTS entry_types (
                id TEXT PRIMARY KEY,
                nonce BLOB NOT NULL,
                ciphertext BLOB NOT NULL
            );
            CREATE TABLE IF NOT EXISTS app_settings (
                id INTEGER PRIMARY KEY CHECK (id = 1),
                auto_lock_minutes INTEGER NOT NULL,
                clipboard_clear_seconds INTEGER NOT NULL,
                theme TEXT NOT NULL,
                language TEXT NOT NULL DEFAULT 'system'
            );
            ",
        )?;
        let has_language = {
            let mut statement = connection.prepare("PRAGMA table_info(app_settings)")?;
            let columns = statement.query_map([], |row| row.get::<_, String>(1))?;
            columns
                .collect::<Result<Vec<_>, _>>()?
                .iter()
                .any(|column| column == "language")
        };
        if !has_language {
            connection.execute(
                "ALTER TABLE app_settings ADD COLUMN language TEXT NOT NULL DEFAULT 'system'",
                [],
            )?;
        }
        connection.execute(
            "INSERT OR IGNORE INTO app_settings
             (id, auto_lock_minutes, clipboard_clear_seconds, theme, language)
             VALUES (1, 5, 30, 'system', 'system')",
            [],
        )?;
        Ok(())
    }

    fn metadata(&self, connection: &Connection) -> AppResult<Option<VaultMetadata>> {
        connection
            .query_row(
                "SELECT salt, argon_memory_kib, argon_iterations, argon_parallelism,
                        verifier_nonce, verifier_ciphertext
                 FROM vault_meta WHERE id = 1",
                [],
                |row| {
                    Ok(VaultMetadata {
                        salt: row.get(0)?,
                        kdf: KdfParams {
                            memory_kib: row.get(1)?,
                            iterations: row.get(2)?,
                            parallelism: row.get(3)?,
                        },
                        verifier_nonce: row.get(4)?,
                        verifier_ciphertext: row.get(5)?,
                    })
                },
            )
            .optional()
            .map_err(Into::into)
    }

    pub fn state(&mut self) -> AppResult<VaultState> {
        let connection = self.connection()?;
        let exists = self.metadata(&connection)?.is_some();
        if exists {
            self.expire_if_idle()?;
        }
        Ok(VaultState {
            exists,
            locked: self.key.is_none(),
        })
    }

    pub fn initialize(&mut self, master_password: String) -> AppResult<()> {
        let master_password = Zeroizing::new(master_password);
        if master_password.is_empty() {
            return Err(AppError::validation("主密码不能为空"));
        }
        let mut connection = self.connection()?;
        if self.metadata(&connection)?.is_some() {
            return Err(AppError::new("VAULT_EXISTS", "密码库已经存在"));
        }

        let salt = crypto::random_bytes::<SALT_LEN>();
        let kdf = KdfParams::default();
        let key = crypto::derive_key(master_password.as_str(), &salt, kdf)?;
        let (nonce, verifier_ciphertext) = crypto::encrypt(&key, VERIFIER, VERIFIER_AAD)?;

        let transaction = connection.transaction()?;
        Self::write_metadata(&transaction, &salt, kdf, &nonce, &verifier_ciphertext)?;
        transaction.commit()?;
        self.key = Some(key);
        self.last_activity = Some(Instant::now());
        Ok(())
    }

    pub fn unlock(&mut self, master_password: String) -> AppResult<()> {
        let master_password = Zeroizing::new(master_password);
        if master_password.is_empty() {
            return Err(AppError::validation("请输入主密码"));
        }
        let connection = self.connection()?;
        let metadata = self
            .metadata(&connection)?
            .ok_or_else(|| AppError::new("VAULT_NOT_INITIALIZED", "密码库尚未创建"))?;
        let key = crypto::derive_key(master_password.as_str(), &metadata.salt, metadata.kdf)?;
        let verifier = crypto::decrypt(
            &key,
            &metadata.verifier_nonce,
            &metadata.verifier_ciphertext,
            VERIFIER_AAD,
        )
        .map_err(|_| AppError::new("INVALID_MASTER_PASSWORD", "主密码不正确"))?;
        if verifier.as_slice() != VERIFIER {
            return Err(AppError::new("INVALID_MASTER_PASSWORD", "主密码不正确"));
        }
        self.key = Some(key);
        self.last_activity = Some(Instant::now());
        Ok(())
    }

    pub fn lock(&mut self) {
        self.key = None;
        self.last_activity = None;
    }

    fn expire_if_idle(&mut self) -> AppResult<()> {
        if self.key.is_none() {
            return Ok(());
        }
        let settings = self.get_settings()?;
        if self.last_activity.is_some_and(|last| {
            last.elapsed() >= Duration::from_secs(settings.auto_lock_minutes as u64 * 60)
        }) {
            self.lock();
        }
        Ok(())
    }

    fn active_key(&mut self) -> AppResult<Zeroizing<[u8; KEY_LEN]>> {
        self.expire_if_idle()?;
        let key = self.key.as_ref().ok_or_else(AppError::locked)?;
        self.last_activity = Some(Instant::now());
        Ok(Zeroizing::new(**key))
    }

    fn write_metadata(
        transaction: &Transaction<'_>,
        salt: &[u8],
        kdf: KdfParams,
        nonce: &[u8],
        ciphertext: &[u8],
    ) -> AppResult<()> {
        transaction.execute(
            "INSERT OR REPLACE INTO vault_meta
             (id, version, salt, argon_memory_kib, argon_iterations, argon_parallelism,
              verifier_nonce, verifier_ciphertext)
             VALUES (1, ?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            params![
                VAULT_VERSION,
                salt,
                kdf.memory_kib,
                kdf.iterations,
                kdf.parallelism,
                nonce,
                ciphertext
            ],
        )?;
        Ok(())
    }

    pub fn get_settings(&self) -> AppResult<AppSettings> {
        let connection = self.connection()?;
        connection
            .query_row(
                "SELECT auto_lock_minutes, clipboard_clear_seconds, theme, language
                 FROM app_settings WHERE id = 1",
                [],
                |row| {
                    Ok(AppSettings {
                        auto_lock_minutes: row.get(0)?,
                        clipboard_clear_seconds: row.get(1)?,
                        theme: row.get(2)?,
                        language: row.get(3)?,
                    })
                },
            )
            .map_err(Into::into)
    }

    pub fn update_settings(&mut self, settings: AppSettings) -> AppResult<AppSettings> {
        Self::validate_settings(&settings)?;
        let connection = self.connection()?;
        Self::write_settings(&connection, &settings)?;
        Ok(settings)
    }

    fn write_settings(connection: &Connection, settings: &AppSettings) -> AppResult<()> {
        connection.execute(
            "UPDATE app_settings SET auto_lock_minutes = ?1,
             clipboard_clear_seconds = ?2, theme = ?3, language = ?4 WHERE id = 1",
            params![
                settings.auto_lock_minutes,
                settings.clipboard_clear_seconds,
                settings.theme,
                settings.language
            ],
        )?;
        Ok(())
    }

    fn validate_settings(settings: &AppSettings) -> AppResult<()> {
        if !(1..=120).contains(&settings.auto_lock_minutes) {
            return Err(AppError::validation("自动锁定时间必须在 1 到 120 分钟之间"));
        }
        if !(5..=300).contains(&settings.clipboard_clear_seconds) {
            return Err(AppError::validation("剪贴板清除时间必须在 5 到 300 秒之间"));
        }
        if !matches!(settings.theme.as_str(), "system" | "light" | "dark") {
            return Err(AppError::validation("主题设置无效"));
        }
        if !matches!(settings.language.as_str(), "system" | "zh-CN" | "en") {
            return Err(AppError::validation("语言设置无效"));
        }
        Ok(())
    }

    fn normalize_input(input: EntryInput) -> AppResult<EntryInput> {
        if input.type_id.trim().is_empty() {
            return Err(AppError::validation("请先选择类型"));
        }
        let name = input.name.trim().to_string();
        if name.is_empty() {
            return Err(AppError::validation("名称不能为空"));
        }

        let mut seen_tags = HashSet::new();
        let tags = input
            .tags
            .into_iter()
            .map(|tag| tag.trim().to_string())
            .filter(|tag| !tag.is_empty())
            .filter(|tag| seen_tags.insert(tag.to_lowercase()))
            .collect();
        let custom_fields = input
            .custom_fields
            .into_iter()
            .filter_map(|field| {
                let label = field.label.trim().to_string();
                if label.is_empty() {
                    return None;
                }
                Some(CustomField {
                    id: if field.id.is_empty() {
                        Uuid::new_v4().to_string()
                    } else {
                        field.id
                    },
                    label,
                    value: field.value,
                    secret: field.secret,
                })
            })
            .collect();

        Ok(EntryInput {
            type_id: input.type_id,
            name,
            url: input.url.trim().to_string(),
            username: input.username.trim().to_string(),
            password: input.password,
            notes: input.notes.trim().to_string(),
            tags,
            favorite: input.favorite,
            custom_fields,
        })
    }

    fn normalize_type_input(input: EntryTypeInput) -> AppResult<EntryTypeInput> {
        let name = input.name.trim().to_string();
        if name.is_empty() {
            return Err(AppError::validation("类型名称不能为空"));
        }
        if name.chars().count() > 30 {
            return Err(AppError::validation("类型名称不能超过 30 个字符"));
        }
        Ok(EntryTypeInput { name })
    }

    fn encrypt_entry(key: &[u8; KEY_LEN], entry: &Entry) -> AppResult<(Vec<u8>, Vec<u8>)> {
        let serialized = Zeroizing::new(serde_json::to_vec(entry)?);
        crypto::encrypt(key, &serialized, Self::entry_aad(&entry.id).as_bytes())
    }

    fn decrypt_entry(
        key: &[u8; KEY_LEN],
        id: &str,
        nonce: &[u8],
        ciphertext: &[u8],
    ) -> AppResult<Entry> {
        let plaintext = crypto::decrypt(key, nonce, ciphertext, Self::entry_aad(id).as_bytes())?;
        let entry: Entry = serde_json::from_slice(&plaintext)?;
        if entry.id != id {
            return Err(AppError::new("DATA_CORRUPTED", "条目身份校验失败"));
        }
        Ok(entry)
    }

    fn entry_aad(id: &str) -> String {
        format!("keychains:entry:v1:{id}")
    }

    fn type_aad(id: &str) -> String {
        format!("keychains:type:v1:{id}")
    }

    fn encrypt_type(key: &[u8; KEY_LEN], entry_type: &EntryType) -> AppResult<(Vec<u8>, Vec<u8>)> {
        let serialized = Zeroizing::new(serde_json::to_vec(entry_type)?);
        crypto::encrypt(key, &serialized, Self::type_aad(&entry_type.id).as_bytes())
    }

    fn decrypt_type(
        key: &[u8; KEY_LEN],
        id: &str,
        nonce: &[u8],
        ciphertext: &[u8],
    ) -> AppResult<EntryType> {
        let plaintext = crypto::decrypt(key, nonce, ciphertext, Self::type_aad(id).as_bytes())?;
        let entry_type: EntryType = serde_json::from_slice(&plaintext)?;
        if entry_type.id != id {
            return Err(AppError::new("DATA_CORRUPTED", "类型身份校验失败"));
        }
        Ok(entry_type)
    }

    fn encrypted_rows(connection: &Connection) -> AppResult<Vec<EncryptedRow>> {
        let mut statement = connection.prepare("SELECT id, nonce, ciphertext FROM entries")?;
        let rows = statement.query_map([], |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)))?;
        rows.collect::<Result<Vec<_>, _>>().map_err(Into::into)
    }

    fn all_entries(connection: &Connection, key: &[u8; KEY_LEN]) -> AppResult<Vec<Entry>> {
        Self::encrypted_rows(connection)?
            .into_iter()
            .map(|(id, nonce, ciphertext)| Self::decrypt_entry(key, &id, &nonce, &ciphertext))
            .collect()
    }

    fn all_types(connection: &Connection, key: &[u8; KEY_LEN]) -> AppResult<Vec<EntryType>> {
        let mut statement = connection.prepare("SELECT id, nonce, ciphertext FROM entry_types")?;
        let rows = statement.query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, Vec<u8>>(1)?,
                row.get::<_, Vec<u8>>(2)?,
            ))
        })?;
        rows.map(|row| {
            let (id, nonce, ciphertext) = row?;
            Self::decrypt_type(key, &id, &nonce, &ciphertext)
        })
        .collect()
    }

    fn save_type(
        transaction: &Transaction<'_>,
        key: &[u8; KEY_LEN],
        entry_type: &EntryType,
    ) -> AppResult<()> {
        let (nonce, ciphertext) = Self::encrypt_type(key, entry_type)?;
        transaction.execute(
            "INSERT OR REPLACE INTO entry_types (id, nonce, ciphertext) VALUES (?1, ?2, ?3)",
            params![entry_type.id, nonce, ciphertext],
        )?;
        Ok(())
    }

    fn ensure_type_exists(
        connection: &Connection,
        key: &[u8; KEY_LEN],
        type_id: &str,
    ) -> AppResult<()> {
        let encrypted: Option<(Vec<u8>, Vec<u8>)> = connection
            .query_row(
                "SELECT nonce, ciphertext FROM entry_types WHERE id = ?1",
                [type_id],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .optional()?;
        let (nonce, ciphertext) =
            encrypted.ok_or_else(|| AppError::validation("所选类型不存在，请重新选择"))?;
        Self::decrypt_type(key, type_id, &nonce, &ciphertext)?;
        Ok(())
    }

    pub fn list_entry_types(&mut self) -> AppResult<Vec<EntryType>> {
        let key = self.active_key()?;
        let connection = self.connection()?;
        let mut types = Self::all_types(&connection, &key)?;
        types.sort_by_key(|entry_type| entry_type.created_at);
        Ok(types)
    }

    pub fn create_entry_type(&mut self, input: EntryTypeInput) -> AppResult<EntryType> {
        let key = self.active_key()?;
        let input = Self::normalize_type_input(input)?;
        let mut connection = self.connection()?;
        if Self::all_types(&connection, &key)?
            .iter()
            .any(|item| item.name.eq_ignore_ascii_case(&input.name))
        {
            return Err(AppError::validation("已存在同名类型"));
        }
        let entry_type = EntryType {
            id: Uuid::new_v4().to_string(),
            name: input.name,
            created_at: Utc::now().timestamp_millis(),
        };
        let transaction = connection.transaction()?;
        Self::save_type(&transaction, &key, &entry_type)?;
        transaction.commit()?;
        Ok(entry_type)
    }

    pub fn delete_entry_type(&mut self, id: &str) -> AppResult<()> {
        let key = self.active_key()?;
        let connection = self.connection()?;
        Self::ensure_type_exists(&connection, &key, id)?;
        if Self::all_entries(&connection, &key)?
            .iter()
            .any(|entry| entry.type_id == id)
        {
            return Err(AppError::validation("该类型下还有条目，无法删除"));
        }
        connection.execute("DELETE FROM entry_types WHERE id = ?1", [id])?;
        Ok(())
    }

    fn entry_by_id(connection: &Connection, key: &[u8; KEY_LEN], id: &str) -> AppResult<Entry> {
        let encrypted: Option<(Vec<u8>, Vec<u8>)> = connection
            .query_row(
                "SELECT nonce, ciphertext FROM entries WHERE id = ?1",
                [id],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .optional()?;
        let (nonce, ciphertext) =
            encrypted.ok_or_else(|| AppError::new("NOT_FOUND", "找不到该条目"))?;
        Self::decrypt_entry(key, id, &nonce, &ciphertext)
    }

    fn save_entry(
        transaction: &Transaction<'_>,
        key: &[u8; KEY_LEN],
        entry: &Entry,
    ) -> AppResult<()> {
        let (nonce, ciphertext) = Self::encrypt_entry(key, entry)?;
        transaction.execute(
            "INSERT OR REPLACE INTO entries (id, nonce, ciphertext) VALUES (?1, ?2, ?3)",
            params![entry.id, nonce, ciphertext],
        )?;
        Ok(())
    }

    fn entry_from_input(id: String, created_at: i64, updated_at: i64, input: EntryInput) -> Entry {
        Entry {
            id,
            type_id: input.type_id,
            name: input.name,
            url: input.url,
            username: input.username,
            password: input.password,
            notes: input.notes,
            tags: input.tags,
            favorite: input.favorite,
            custom_fields: input.custom_fields,
            created_at,
            updated_at,
        }
    }

    pub fn create_entry(&mut self, input: EntryInput) -> AppResult<Entry> {
        let key = self.active_key()?;
        let input = Self::normalize_input(input)?;
        let connection = self.connection()?;
        Self::ensure_type_exists(&connection, &key, &input.type_id)?;
        let now = Utc::now().timestamp_millis();
        let entry = Self::entry_from_input(Uuid::new_v4().to_string(), now, now, input);
        let mut connection = connection;
        let transaction = connection.transaction()?;
        Self::save_entry(&transaction, &key, &entry)?;
        transaction.commit()?;
        Ok(entry)
    }

    pub fn update_entry(&mut self, id: &str, input: EntryInput) -> AppResult<Entry> {
        let key = self.active_key()?;
        let input = Self::normalize_input(input)?;
        let mut connection = self.connection()?;
        Self::ensure_type_exists(&connection, &key, &input.type_id)?;
        let existing = Self::entry_by_id(&connection, &key, id)?;
        let entry = Self::entry_from_input(
            existing.id,
            existing.created_at,
            Utc::now().timestamp_millis(),
            input,
        );
        let transaction = connection.transaction()?;
        Self::save_entry(&transaction, &key, &entry)?;
        transaction.commit()?;
        Ok(entry)
    }

    pub fn delete_entry(&mut self, id: &str) -> AppResult<()> {
        let _key = self.active_key()?;
        let connection = self.connection()?;
        if connection.execute("DELETE FROM entries WHERE id = ?1", [id])? == 0 {
            return Err(AppError::new("NOT_FOUND", "找不到该条目"));
        }
        Ok(())
    }

    pub fn get_entry(&mut self, id: &str) -> AppResult<Entry> {
        let key = self.active_key()?;
        let connection = self.connection()?;
        Self::entry_by_id(&connection, &key, id)
    }

    pub fn list_entries(&mut self, options: ListOptions) -> AppResult<Vec<EntrySummary>> {
        let key = self.active_key()?;
        let connection = self.connection()?;
        let query = options.query.trim().to_lowercase();
        let mut entries: Vec<Entry> = Self::all_entries(&connection, &key)?
            .into_iter()
            .filter(|entry| !options.favorite_only || entry.favorite)
            .filter(|entry| {
                options
                    .type_id
                    .as_ref()
                    .is_none_or(|type_id| &entry.type_id == type_id)
            })
            .filter(|entry| {
                options.tag.as_ref().is_none_or(|tag| {
                    entry
                        .tags
                        .iter()
                        .any(|value| value.eq_ignore_ascii_case(tag))
                })
            })
            .filter(|entry| query.is_empty() || Self::entry_matches(entry, &query))
            .collect();

        entries.sort_by(|left, right| match options.sort {
            SortOrder::NameAsc => left.name.to_lowercase().cmp(&right.name.to_lowercase()),
            SortOrder::NameDesc => right.name.to_lowercase().cmp(&left.name.to_lowercase()),
            SortOrder::UsernameAsc => left
                .username
                .to_lowercase()
                .cmp(&right.username.to_lowercase()),
            SortOrder::CreatedDesc => right.created_at.cmp(&left.created_at),
            SortOrder::UpdatedDesc => right.updated_at.cmp(&left.updated_at),
        });
        Ok(entries.iter().map(EntrySummary::from).collect())
    }

    fn entry_matches(entry: &Entry, query: &str) -> bool {
        entry.name.to_lowercase().contains(query)
            || entry.url.to_lowercase().contains(query)
            || entry.username.to_lowercase().contains(query)
            || entry.notes.to_lowercase().contains(query)
            || entry
                .tags
                .iter()
                .any(|tag| tag.to_lowercase().contains(query))
            || entry.custom_fields.iter().any(|field| {
                !field.secret
                    && (field.label.to_lowercase().contains(query)
                        || field.value.to_lowercase().contains(query))
            })
    }

    pub fn change_master_password(
        &mut self,
        current_password: String,
        new_password: String,
    ) -> AppResult<()> {
        let current_password = Zeroizing::new(current_password);
        let new_password = Zeroizing::new(new_password);
        if new_password.is_empty() {
            return Err(AppError::validation("新主密码不能为空"));
        }
        let connection = self.connection()?;
        let metadata = self
            .metadata(&connection)?
            .ok_or_else(|| AppError::new("VAULT_NOT_INITIALIZED", "密码库尚未创建"))?;
        let current_key =
            crypto::derive_key(current_password.as_str(), &metadata.salt, metadata.kdf)?;
        crypto::decrypt(
            &current_key,
            &metadata.verifier_nonce,
            &metadata.verifier_ciphertext,
            VERIFIER_AAD,
        )
        .map_err(|_| AppError::new("INVALID_MASTER_PASSWORD", "当前主密码不正确"))?;
        let entries = Self::all_entries(&connection, &current_key)?;
        let entry_types = Self::all_types(&connection, &current_key)?;

        let salt = crypto::random_bytes::<SALT_LEN>();
        let kdf = KdfParams::default();
        let new_key = crypto::derive_key(new_password.as_str(), &salt, kdf)?;
        let (verifier_nonce, verifier_ciphertext) =
            crypto::encrypt(&new_key, VERIFIER, VERIFIER_AAD)?;

        let mut connection = connection;
        let transaction = connection.transaction()?;
        for entry in &entries {
            Self::save_entry(&transaction, &new_key, entry)?;
        }
        for entry_type in &entry_types {
            Self::save_type(&transaction, &new_key, entry_type)?;
        }
        Self::write_metadata(
            &transaction,
            &salt,
            kdf,
            &verifier_nonce,
            &verifier_ciphertext,
        )?;
        transaction.commit()?;
        self.key = Some(new_key);
        self.last_activity = Some(Instant::now());
        Ok(())
    }

    pub fn generate_password(options: PasswordOptions) -> AppResult<String> {
        if !(12..=64).contains(&options.length) {
            return Err(AppError::validation("密码长度必须在 12 到 64 之间"));
        }
        let groups: Vec<&[u8]> = [
            options
                .lowercase
                .then_some(b"abcdefghijklmnopqrstuvwxyz".as_slice()),
            options
                .uppercase
                .then_some(b"ABCDEFGHIJKLMNOPQRSTUVWXYZ".as_slice()),
            options.digits.then_some(b"0123456789".as_slice()),
            options
                .symbols
                .then_some(b"!@#$%^&*()-_=+[]{};:,.?".as_slice()),
        ]
        .into_iter()
        .flatten()
        .collect();
        if groups.is_empty() {
            return Err(AppError::validation("至少启用一种字符类型"));
        }

        let all: Vec<u8> = groups
            .iter()
            .flat_map(|group| group.iter().copied())
            .collect();
        let mut rng = OsRng;
        let mut result: Vec<u8> = groups
            .iter()
            .map(|group| group[rng.gen_range(0..group.len())])
            .collect();
        while result.len() < options.length {
            result.push(all[rng.gen_range(0..all.len())]);
        }
        result.shuffle(&mut rng);
        String::from_utf8(result).map_err(|_| AppError::new("CRYPTO", "密码生成失败"))
    }

    pub fn secret_value(&mut self, id: &str, field: &str) -> AppResult<String> {
        let entry = self.get_entry(id)?;
        match field {
            "password" => Ok(entry.password),
            "username" => Ok(entry.username),
            _ if field.starts_with("custom:") => {
                let custom_id = &field[7..];
                entry
                    .custom_fields
                    .into_iter()
                    .find(|item| item.id == custom_id)
                    .map(|item| item.value)
                    .ok_or_else(|| AppError::new("NOT_FOUND", "找不到该自定义字段"))
            }
            _ => Err(AppError::validation("不支持复制该字段")),
        }
    }

    fn decrypt_backup(path: &Path, password: String) -> AppResult<BackupPayload> {
        let password = Zeroizing::new(password);
        if password.is_empty() {
            return Err(AppError::validation("备份密码不能为空"));
        }
        if fs::metadata(path)?.len() > MAX_BACKUP_BYTES {
            return Err(AppError::new("BACKUP_INVALID", "备份文件过大"));
        }
        let bytes = fs::read(path)?;
        let envelope: BackupEnvelope = serde_json::from_slice(&bytes)
            .map_err(|_| AppError::new("BACKUP_INVALID", "备份文件格式无效"))?;
        if envelope.magic != BACKUP_MAGIC || envelope.version != 1 {
            return Err(AppError::new("BACKUP_INVALID", "不支持该备份文件"));
        }
        let salt = BASE64
            .decode(envelope.salt)
            .map_err(|_| AppError::new("BACKUP_INVALID", "备份盐值无效"))?;
        let nonce = BASE64
            .decode(envelope.nonce)
            .map_err(|_| AppError::new("BACKUP_INVALID", "备份 nonce 无效"))?;
        let ciphertext = BASE64
            .decode(envelope.ciphertext)
            .map_err(|_| AppError::new("BACKUP_INVALID", "备份密文无效"))?;
        let key = crypto::derive_key(password.as_str(), &salt, envelope.kdf)?;
        let plaintext = crypto::decrypt(&key, &nonce, &ciphertext, BACKUP_AAD)
            .map_err(|_| AppError::new("BACKUP_PASSWORD_INVALID", "备份密码错误或文件已损坏"))?;
        serde_json::from_slice(&plaintext)
            .map_err(|_| AppError::new("BACKUP_INVALID", "备份内容无效"))
    }

    pub fn export_backup(&mut self, path: &Path, password: String) -> AppResult<BackupPreview> {
        let password = Zeroizing::new(password);
        if password.is_empty() {
            return Err(AppError::validation("备份密码不能为空"));
        }
        let key = self.active_key()?;
        let connection = self.connection()?;
        let payload = BackupPayload {
            entries: Self::all_entries(&connection, &key)?,
            entry_types: Self::all_types(&connection, &key)?,
            settings: self.get_settings()?,
            exported_at: Utc::now().timestamp_millis(),
        };
        let serialized = Zeroizing::new(serde_json::to_vec(&payload)?);
        let salt = crypto::random_bytes::<SALT_LEN>();
        let kdf = KdfParams::default();
        let backup_key = crypto::derive_key(password.as_str(), &salt, kdf)?;
        let (nonce, ciphertext) = crypto::encrypt(&backup_key, &serialized, BACKUP_AAD)?;
        let envelope = BackupEnvelope {
            magic: BACKUP_MAGIC.into(),
            version: 1,
            kdf,
            salt: BASE64.encode(salt),
            nonce: BASE64.encode(nonce),
            ciphertext: BASE64.encode(ciphertext),
        };
        fs::write(path, serde_json::to_vec_pretty(&envelope)?)?;
        Ok(BackupPreview {
            entry_count: payload.entries.len(),
            exported_at: payload.exported_at,
        })
    }

    pub fn inspect_backup(path: &Path, password: String) -> AppResult<BackupPreview> {
        let payload = Self::decrypt_backup(path, password)?;
        Self::validate_settings(&payload.settings)
            .map_err(|_| AppError::new("BACKUP_INVALID", "备份设置无效"))?;
        Ok(BackupPreview {
            entry_count: payload.entries.len(),
            exported_at: payload.exported_at,
        })
    }

    pub fn import_backup(
        &mut self,
        path: &Path,
        password: String,
        mode: ImportMode,
    ) -> AppResult<ImportReport> {
        let payload = Self::decrypt_backup(path, password)?;
        Self::validate_settings(&payload.settings)
            .map_err(|_| AppError::new("BACKUP_INVALID", "备份设置无效"))?;
        let key = self.active_key()?;
        let mut connection = self.connection()?;
        let transaction = connection.transaction()?;
        let mut imported = 0;
        let mut skipped = 0;

        if matches!(mode, ImportMode::Replace) {
            transaction.execute("DELETE FROM entries", [])?;
            transaction.execute("DELETE FROM entry_types", [])?;
        }
        for entry_type in &payload.entry_types {
            let normalized = Self::normalize_type_input(EntryTypeInput {
                name: entry_type.name.clone(),
            })
            .map_err(|_| AppError::new("BACKUP_INVALID", "备份中存在无效类型"))?;
            if entry_type.id.is_empty() || normalized.name != entry_type.name {
                return Err(AppError::new("BACKUP_INVALID", "备份中存在无效类型"));
            }
            Self::save_type(&transaction, &key, entry_type)?;
        }
        for entry in &payload.entries {
            if entry.id.is_empty() || entry.name.trim().is_empty() {
                return Err(AppError::new("BACKUP_INVALID", "备份中存在无效条目"));
            }
            Self::ensure_type_exists(&transaction, &key, &entry.type_id)
                .map_err(|_| AppError::new("BACKUP_INVALID", "条目引用了不存在的类型"))?;
            let existing: Option<(Vec<u8>, Vec<u8>)> = transaction
                .query_row(
                    "SELECT nonce, ciphertext FROM entries WHERE id = ?1",
                    [&entry.id],
                    |row| Ok((row.get(0)?, row.get(1)?)),
                )
                .optional()?;
            let should_import = existing
                .map(|(nonce, ciphertext)| {
                    Self::decrypt_entry(&key, &entry.id, &nonce, &ciphertext)
                        .map(|current| entry.updated_at > current.updated_at)
                })
                .transpose()?
                .unwrap_or(true);
            if should_import {
                Self::save_entry(&transaction, &key, entry)?;
                imported += 1;
            } else {
                skipped += 1;
            }
        }
        if matches!(mode, ImportMode::Replace) {
            Self::write_settings(&transaction, &payload.settings)?;
        }
        transaction.commit()?;
        Ok(ImportReport { imported, skipped })
    }

    pub fn restore_new_vault(
        &mut self,
        path: &Path,
        backup_password: String,
        new_master_password: String,
    ) -> AppResult<()> {
        let new_master_password = Zeroizing::new(new_master_password);
        if new_master_password.is_empty() {
            return Err(AppError::validation("新主密码不能为空"));
        }
        let payload = Self::decrypt_backup(path, backup_password)?;
        Self::validate_settings(&payload.settings)
            .map_err(|_| AppError::new("BACKUP_INVALID", "备份设置无效"))?;
        if payload
            .entries
            .iter()
            .any(|entry| entry.id.is_empty() || entry.name.trim().is_empty())
            || payload.entry_types.iter().any(|entry_type| {
                entry_type.id.is_empty()
                    || Self::normalize_type_input(EntryTypeInput {
                        name: entry_type.name.clone(),
                    })
                    .is_err()
            })
        {
            return Err(AppError::new("BACKUP_INVALID", "备份中存在无效条目或类型"));
        }
        let mut connection = self.connection()?;
        if self.metadata(&connection)?.is_some() {
            return Err(AppError::new("VAULT_EXISTS", "密码库已经存在"));
        }
        let salt = crypto::random_bytes::<SALT_LEN>();
        let kdf = KdfParams::default();
        let key = crypto::derive_key(new_master_password.as_str(), &salt, kdf)?;
        let (verifier_nonce, verifier_ciphertext) = crypto::encrypt(&key, VERIFIER, VERIFIER_AAD)?;
        let transaction = connection.transaction()?;
        Self::write_metadata(
            &transaction,
            &salt,
            kdf,
            &verifier_nonce,
            &verifier_ciphertext,
        )?;
        for entry_type in &payload.entry_types {
            Self::save_type(&transaction, &key, entry_type)?;
        }
        for entry in &payload.entries {
            Self::ensure_type_exists(&transaction, &key, &entry.type_id)
                .map_err(|_| AppError::new("BACKUP_INVALID", "条目引用了不存在的类型"))?;
            Self::save_entry(&transaction, &key, entry)?;
        }
        Self::write_settings(&transaction, &payload.settings)?;
        transaction.commit()?;
        self.key = Some(key);
        self.last_activity = Some(Instant::now());
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    fn service() -> VaultService {
        let directory = tempdir().unwrap().keep();
        VaultService::new(directory.join("vault.db")).unwrap()
    }

    fn create_type(vault: &mut VaultService) -> String {
        vault
            .create_entry_type(EntryTypeInput {
                name: "账号".into(),
            })
            .unwrap()
            .id
    }

    fn sample(type_id: &str, name: &str, username: &str) -> EntryInput {
        EntryInput {
            type_id: type_id.into(),
            name: name.into(),
            url: "https://example.com".into(),
            username: username.into(),
            password: "secret".into(),
            notes: "中文备注".into(),
            tags: vec!["工作".into()],
            favorite: false,
            custom_fields: vec![],
        }
    }

    #[test]
    fn initialize_unlock_and_crud() {
        let mut vault = service();
        vault.initialize("weak".into()).unwrap();
        assert_eq!(vault.get_settings().unwrap().language, "system");
        let type_id = create_type(&mut vault);
        let created = vault
            .create_entry(sample(&type_id, "示例网站", "mollin"))
            .unwrap();
        assert_eq!(vault.get_entry(&created.id).unwrap().password, "secret");
        vault.lock();
        assert!(vault.get_entry(&created.id).is_err());
        assert!(vault.unlock("wrong".into()).is_err());
        vault.unlock("weak".into()).unwrap();
        assert_eq!(vault.list_entries(ListOptions::default()).unwrap().len(), 1);
        assert!(vault.delete_entry_type(&type_id).is_err());
        vault.delete_entry(&created.id).unwrap();
        assert!(vault.get_entry(&created.id).is_err());
        vault.delete_entry_type(&type_id).unwrap();
        assert!(vault.list_entry_types().unwrap().is_empty());
    }

    #[test]
    fn search_finds_chinese_and_username() {
        let mut vault = service();
        vault.initialize("master".into()).unwrap();
        let type_id = create_type(&mut vault);
        vault
            .create_entry(sample(&type_id, "公司邮箱", "hello@example.com"))
            .unwrap();
        let options = ListOptions {
            query: "公司".into(),
            ..ListOptions::default()
        };
        assert_eq!(vault.list_entries(options).unwrap().len(), 1);
        let options = ListOptions {
            query: "HELLO@".into(),
            ..ListOptions::default()
        };
        assert_eq!(vault.list_entries(options).unwrap().len(), 1);
    }

    #[test]
    fn changing_master_password_reencrypts_entries() {
        let mut vault = service();
        vault.initialize("old".into()).unwrap();
        let type_id = create_type(&mut vault);
        vault
            .create_entry(sample(&type_id, "Example", "user"))
            .unwrap();
        vault
            .change_master_password("old".into(), "new".into())
            .unwrap();
        vault.lock();
        assert!(vault.unlock("old".into()).is_err());
        vault.unlock("new".into()).unwrap();
        assert_eq!(vault.list_entries(ListOptions::default()).unwrap().len(), 1);
    }

    #[test]
    fn generated_password_satisfies_enabled_groups() {
        let password = VaultService::generate_password(PasswordOptions {
            length: 24,
            uppercase: true,
            lowercase: true,
            digits: true,
            symbols: true,
        })
        .unwrap();
        assert_eq!(password.len(), 24);
        assert!(password
            .chars()
            .any(|character| character.is_ascii_uppercase()));
        assert!(password
            .chars()
            .any(|character| character.is_ascii_lowercase()));
        assert!(password.chars().any(|character| character.is_ascii_digit()));
        assert!(password
            .chars()
            .any(|character| !character.is_ascii_alphanumeric()));
    }

    #[test]
    fn encrypted_backup_can_restore_a_new_vault() {
        let directory = tempdir().unwrap();
        let backup_path = directory.path().join("vault.kcbak");
        let mut source = VaultService::new(directory.path().join("source.db")).unwrap();
        source.initialize("source-master".into()).unwrap();
        let type_id = create_type(&mut source);
        source
            .create_entry(sample(&type_id, "GitHub", "mollin"))
            .unwrap();
        let preview = source
            .export_backup(&backup_path, "backup-secret".into())
            .unwrap();
        assert_eq!(preview.entry_count, 1);
        assert!(VaultService::inspect_backup(&backup_path, "wrong".into()).is_err());

        let mut restored = VaultService::new(directory.path().join("restored.db")).unwrap();
        restored
            .restore_new_vault(&backup_path, "backup-secret".into(), "new-master".into())
            .unwrap();
        assert_eq!(
            restored.list_entries(ListOptions::default()).unwrap().len(),
            1
        );
        restored.lock();
        assert!(restored.unlock("source-master".into()).is_err());
        restored.unlock("new-master".into()).unwrap();
    }
}
