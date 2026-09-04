//! Immutable published section payload. Pages are sealed; no Storage pointers.

use super::SectionError;
use lumio_voxel_contracts::legacy_baseline;
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

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SectionPayload {
    schema_id: &'static str,
    pages: Arc<[SealedPage]>,
    _extent_family: IsolatedCubicExtentFamily,
}

impl SectionPayload {
    pub fn from_pages(pages: impl IntoIterator<Item = SectionPage>) -> Result<Self, SectionError> {
        let schema_id = SCHEMA_IDS
            .iter()
            .copied()
            .find(|id| *id == SECTION_PAGE_SCHEMA)
            .expect("the section page schema id must exist in the frozen mirror's SCHEMA_IDS");
        let mut sealed = Vec::new();
        for page in pages {
            sealed.push(DenseUncompressedAdapter::seal(page)?);
        }
        Ok(Self {
            schema_id,
            pages: Arc::from(sealed),
            _extent_family: IsolatedCubicExtentFamily,
        })
    }

    pub fn schema_id(&self) -> &'static str {
        self.schema_id
    }
}

fn intern_name(name: &str, generated: &[&'static str]) -> Result<&'static str, SectionError> {
    generated
        .iter()
        .copied()
        .find(|item| *item == name)
        .ok_or_else(SectionError::invalid_handle)
}
