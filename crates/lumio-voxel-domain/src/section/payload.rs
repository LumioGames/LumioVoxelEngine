//! Immutable published section payload. Pages are sealed; no Storage pointers.

use super::SectionError;
use lumio_voxel_contracts::legacy_baseline;
use lumio_voxel_contracts::voxel_world as vw;
use lumio_voxel_contracts::{SCHEMA_IDS, sha256};
use std::sync::Arc;

/// 页 schema 在废弃基线里的 id。名字里的 `chunk` 是那份冻结产物的拼写,不是分层语义
/// (见 `lumio_voxel_contracts::legacy_baseline`);Section 的页语义取自活契约。
const SECTION_PAGE_SCHEMA: &str = legacy_baseline::SECTION_PAGE_SCHEMA_ID;

/// Generated page-encoding identities. Not Schema columns.
const PAGE_ENCODINGS: &[&str] = &["Dense", "Sparse"];
/// Generated compression identities. Not a selected public default.
const COMPRESSION_CODECS: &[&str] = &["None", "Zstd", "Lz4"];

const V1_ENCODING: &str = "Dense";
const V1_CODEC: &str = "None";

/// Unpublished page envelope. Validation and sealing happen in `from_pages`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SectionPage {
    encoding: String,
    compression: String,
    bytes: Vec<u8>,
    declared_digest: [u8; 32],
}

impl SectionPage {
    pub fn new(
        encoding: impl Into<String>,
        compression: impl Into<String>,
        bytes: impl Into<Vec<u8>>,
        declared_digest: [u8; 32],
    ) -> Self {
        Self {
            encoding: encoding.into(),
            compression: compression.into(),
            bytes: bytes.into(),
            declared_digest,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct SealedPage {
    encoding: &'static str,
    compression: &'static str,
    bytes: Arc<[u8]>,
    digest: [u8; 32],
}

/// V1 adapter-internal backend. Codec identity is generated `None`.
struct DenseUncompressedAdapter;

impl DenseUncompressedAdapter {
    fn seal(page: SectionPage) -> Result<SealedPage, SectionError> {
        let encoding = intern_name(&page.encoding, PAGE_ENCODINGS)?;
        let compression = intern_name(&page.compression, COMPRESSION_CODECS)?;
        if encoding != V1_ENCODING || compression != V1_CODEC {
            return Err(SectionError::invalid_handle());
        }
        let digest = sha256(&page.bytes);
        if digest != page.declared_digest {
            return Err(SectionError::section_digest_mismatch());
        }
        Ok(SealedPage {
            encoding,
            compression,
            bytes: Arc::from(page.bytes),
            digest,
        })
    }
}

/// IsolatedCubicExtentFamily is adapter-internal. No public extent field.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct IsolatedCubicExtentFamily;

#[derive(Clone, PartialEq, Eq)]
pub struct SectionPayload {
    schema_id: &'static str,
    pages: Arc<[SealedPage]>,
    // Block storage is an adapter-local baseline used by structured mutation.
    // Generic callers may omit it when their page bytes are not voxel storage.
    storage: Option<super::SectionStorage>,
    _extent_family: IsolatedCubicExtentFamily,
}

impl std::fmt::Debug for SectionPayload {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("SectionPayload")
            .field("schema_id", &self.schema_id)
            .field("pages", &self.pages)
            .field("_extent_family", &self._extent_family)
            .finish()
    }
}

impl SectionPayload {
    /// Build a structured voxel payload whose immutable page and storage sidecar
    /// are derived from the same canonical bytes.
    pub fn from_storage(storage: super::SectionStorage) -> Result<Self, SectionError> {
        let bytes = storage.encoded_payload();
        let digest = lumio_voxel_contracts::sha256(&bytes);
        Self::from_pages_with_storage(
            [SectionPage::new("Dense", "None", bytes, digest)],
            Some(storage),
        )
    }

    pub fn from_pages(pages: impl IntoIterator<Item = SectionPage>) -> Result<Self, SectionError> {
        Self::from_pages_with_storage(pages, None)
    }

    pub fn from_pages_with_storage(
        pages: impl IntoIterator<Item = SectionPage>,
        storage: Option<super::SectionStorage>,
    ) -> Result<Self, SectionError> {
        let schema_id = SCHEMA_IDS
            .iter()
            .copied()
            .find(|id| *id == SECTION_PAGE_SCHEMA)
            .expect("the section page schema id must exist in the frozen mirror's SCHEMA_IDS");
        let mut sealed = Vec::new();
        for page in pages {
            sealed.push(DenseUncompressedAdapter::seal(page)?);
        }
        if let Some(storage) = &storage {
            let encoded = storage.encoded_payload();
            if sealed.len() != 1 || sealed[0].bytes.as_ref() != encoded.as_slice() {
                return Err(SectionError::contract_violation(
                    vw::SECTION_ENCODING_MISMATCH,
                ));
            }
        }
        Ok(Self {
            schema_id,
            pages: Arc::from(sealed),
            storage,
            _extent_family: IsolatedCubicExtentFamily,
        })
    }

    pub fn schema_id(&self) -> &'static str {
        self.schema_id
    }

    pub fn storage(&self) -> Option<&super::SectionStorage> {
        self.storage.as_ref()
    }

    pub(crate) fn identity_digest(&self) -> [u8; 32] {
        let mut bytes = Vec::new();
        bytes.extend_from_slice(self.schema_id.as_bytes());
        bytes.extend_from_slice(&(self.pages.len() as u64).to_le_bytes());
        for page in self.pages.iter() {
            bytes.extend_from_slice(page.encoding.as_bytes());
            bytes.push(0);
            bytes.extend_from_slice(page.compression.as_bytes());
            bytes.push(0);
            bytes.extend_from_slice(&(page.bytes.len() as u64).to_le_bytes());
            bytes.extend_from_slice(&page.digest);
        }
        match &self.storage {
            Some(storage) => {
                bytes.push(1);
                bytes.extend_from_slice(&storage.identity_digest());
            }
            None => bytes.push(0),
        }
        lumio_voxel_contracts::sha256(&bytes)
    }

    pub(crate) fn replacement_identity_digest(&self) -> [u8; 32] {
        if self.storage.is_none() {
            return lumio_voxel_contracts::sha256(format!("{self:?}").as_bytes());
        }
        self.identity_digest()
    }

    pub(crate) fn storage_identity_digest(&self) -> Option<[u8; 32]> {
        self.storage
            .as_ref()
            .map(|storage| storage.identity_digest())
    }
}

fn intern_name(name: &str, generated: &[&'static str]) -> Result<&'static str, SectionError> {
    generated
        .iter()
        .copied()
        .find(|item| *item == name)
        .ok_or_else(SectionError::invalid_handle)
}
