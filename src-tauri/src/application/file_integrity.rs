use crate::{
    domain::{FileHash, HashAlgorithm},
    error::AppResult,
};
use md5::Md5;
use sha1::Sha1;
use sha2::{Digest, Sha512};
use std::path::Path;
use tokio::io::AsyncReadExt;

pub(crate) enum HashState {
    Sha512(Sha512),
    Sha1(Sha1),
    Md5(Md5),
}

impl HashState {
    pub(crate) fn new(algorithm: HashAlgorithm) -> Self {
        match algorithm {
            HashAlgorithm::Sha512 => Self::Sha512(Sha512::new()),
            HashAlgorithm::Sha1 => Self::Sha1(Sha1::new()),
            HashAlgorithm::Md5 => Self::Md5(Md5::new()),
        }
    }

    pub(crate) fn update(&mut self, bytes: &[u8]) {
        match self {
            Self::Sha512(hash) => hash.update(bytes),
            Self::Sha1(hash) => hash.update(bytes),
            Self::Md5(hash) => hash.update(bytes),
        }
    }

    pub(crate) fn finish(self) -> String {
        match self {
            Self::Sha512(hash) => hex::encode(hash.finalize()),
            Self::Sha1(hash) => hex::encode(hash.finalize()),
            Self::Md5(hash) => hex::encode(hash.finalize()),
        }
    }
}

pub fn preferred_hash(hashes: &[FileHash]) -> Option<&FileHash> {
    hashes
        .iter()
        .find(|hash| matches!(hash.algorithm, HashAlgorithm::Sha512))
        .or_else(|| {
            hashes
                .iter()
                .find(|hash| matches!(hash.algorithm, HashAlgorithm::Sha1))
        })
        .or_else(|| hashes.first())
}

pub async fn hash_file(path: &Path, algorithm: HashAlgorithm) -> AppResult<String> {
    let mut file = tokio::fs::File::open(path).await?;
    let mut buffer = vec![0u8; 64 * 1024];
    let mut hash = HashState::new(algorithm);
    loop {
        let size = file.read(&mut buffer).await?;
        if size == 0 {
            break;
        }
        hash.update(&buffer[..size]);
    }
    Ok(hash.finish())
}

pub async fn hash_matches(path: &Path, expected: &FileHash) -> AppResult<bool> {
    Ok(hash_file(path, expected.algorithm)
        .await?
        .eq_ignore_ascii_case(&expected.value))
}

pub async fn curseforge_fingerprint_file(path: &Path) -> AppResult<u32> {
    let mut file = tokio::fs::File::open(path).await?;
    let mut buffer = vec![0u8; 64 * 1024];
    let mut bytes = Vec::new();
    loop {
        let size = file.read(&mut buffer).await?;
        if size == 0 {
            break;
        }
        bytes.extend(
            buffer[..size]
                .iter()
                .copied()
                .filter(|byte| !matches!(byte, b'\t' | b'\n' | b'\r' | b' ')),
        );
    }
    Ok(murmur2(&bytes, 1))
}

fn murmur2(data: &[u8], seed: u32) -> u32 {
    const M: u32 = 0x5bd1e995;
    const R: u32 = 24;

    let mut hash = seed ^ data.len() as u32;
    let mut chunks = data.chunks_exact(4);
    for chunk in &mut chunks {
        let mut value = u32::from_le_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]);
        value = value.wrapping_mul(M);
        value ^= value >> R;
        value = value.wrapping_mul(M);

        hash = hash.wrapping_mul(M);
        hash ^= value;
    }

    let remainder = chunks.remainder();
    match remainder.len() {
        3 => {
            hash ^= (remainder[2] as u32) << 16;
            hash ^= (remainder[1] as u32) << 8;
            hash ^= remainder[0] as u32;
            hash = hash.wrapping_mul(M);
        }
        2 => {
            hash ^= (remainder[1] as u32) << 8;
            hash ^= remainder[0] as u32;
            hash = hash.wrapping_mul(M);
        }
        1 => {
            hash ^= remainder[0] as u32;
            hash = hash.wrapping_mul(M);
        }
        _ => {}
    }

    hash ^= hash >> 13;
    hash = hash.wrapping_mul(M);
    hash ^ (hash >> 15)
}

#[cfg(test)]
mod tests {
    use super::murmur2;

    #[test]
    fn murmur2_matches_a_stable_vector() {
        assert_eq!(murmur2(b"mosaic", 1), 0x38fe2e60);
    }
}
