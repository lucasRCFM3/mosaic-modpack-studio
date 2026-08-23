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
