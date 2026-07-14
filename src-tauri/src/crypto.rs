use crate::error::{AppError, AppResult};
use argon2::{Algorithm, Argon2, Params, Version};
use chacha20poly1305::{
    aead::{Aead, Payload},
    KeyInit, XChaCha20Poly1305, XNonce,
};
use rand::{rngs::OsRng, RngCore};
use serde::{Deserialize, Serialize};
use zeroize::Zeroizing;

pub const KEY_LEN: usize = 32;
pub const SALT_LEN: usize = 16;
pub const NONCE_LEN: usize = 24;

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct KdfParams {
    pub memory_kib: u32,
    pub iterations: u32,
    pub parallelism: u32,
}

impl Default for KdfParams {
    fn default() -> Self {
        Self {
            memory_kib: 65_536,
            iterations: 3,
            parallelism: 1,
        }
    }
}

pub fn random_bytes<const N: usize>() -> [u8; N] {
    let mut output = [0u8; N];
    OsRng.fill_bytes(&mut output);
    output
}

pub fn derive_key(
    password: &str,
    salt: &[u8],
    config: KdfParams,
) -> AppResult<Zeroizing<[u8; KEY_LEN]>> {
    if !(32_768..=262_144).contains(&config.memory_kib)
        || !(2..=10).contains(&config.iterations)
        || !(1..=4).contains(&config.parallelism)
    {
        return Err(AppError::new("CRYPTO", "密钥派生参数超出安全范围"));
    }
    let params = Params::new(
        config.memory_kib,
        config.iterations,
        config.parallelism,
        Some(KEY_LEN),
    )
    .map_err(|_| AppError::new("CRYPTO", "密钥派生参数无效"))?;
    let argon = Argon2::new(Algorithm::Argon2id, Version::V0x13, params);
    let mut key = Zeroizing::new([0u8; KEY_LEN]);
    argon
        .hash_password_into(password.as_bytes(), salt, key.as_mut())
        .map_err(|_| AppError::new("CRYPTO", "无法派生加密密钥"))?;
    Ok(key)
}

pub fn encrypt(key: &[u8; KEY_LEN], plaintext: &[u8], aad: &[u8]) -> AppResult<(Vec<u8>, Vec<u8>)> {
    let cipher = XChaCha20Poly1305::new(key.into());
    let nonce = random_bytes::<NONCE_LEN>();
    let ciphertext = cipher
        .encrypt(
            XNonce::from_slice(&nonce),
            Payload {
                msg: plaintext,
                aad,
            },
        )
        .map_err(|_| AppError::new("CRYPTO", "数据加密失败"))?;
    Ok((nonce.to_vec(), ciphertext))
}

pub fn decrypt(
    key: &[u8; KEY_LEN],
    nonce: &[u8],
    ciphertext: &[u8],
    aad: &[u8],
) -> AppResult<Zeroizing<Vec<u8>>> {
    if nonce.len() != NONCE_LEN {
        return Err(AppError::new("DATA_CORRUPTED", "加密数据的 nonce 无效"));
    }
    let cipher = XChaCha20Poly1305::new(key.into());
    let plaintext = cipher
        .decrypt(
            XNonce::from_slice(nonce),
            Payload {
                msg: ciphertext,
                aad,
            },
        )
        .map_err(|_| AppError::new("DECRYPT_FAILED", "密码错误或加密数据已损坏"))?;
    Ok(Zeroizing::new(plaintext))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trip_and_tamper_detection() {
        let key = random_bytes::<KEY_LEN>();
        let (nonce, mut ciphertext) = encrypt(&key, b"secret", b"entry:1").unwrap();
        assert_eq!(
            &*decrypt(&key, &nonce, &ciphertext, b"entry:1").unwrap(),
            b"secret"
        );
        ciphertext[0] ^= 1;
        assert!(decrypt(&key, &nonce, &ciphertext, b"entry:1").is_err());
    }

    #[test]
    fn aad_prevents_record_swapping() {
        let key = random_bytes::<KEY_LEN>();
        let (nonce, ciphertext) = encrypt(&key, b"secret", b"entry:1").unwrap();
        assert!(decrypt(&key, &nonce, &ciphertext, b"entry:2").is_err());
    }

    #[test]
    fn encryption_uses_unique_nonces() {
        let key = random_bytes::<KEY_LEN>();
        let (first, _) = encrypt(&key, b"same", b"aad").unwrap();
        let (second, _) = encrypt(&key, b"same", b"aad").unwrap();
        assert_ne!(first, second);
    }
}
