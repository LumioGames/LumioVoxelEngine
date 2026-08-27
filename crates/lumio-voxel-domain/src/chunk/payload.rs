//! Immutable published chunk payload. Pages are sealed; no Storage pointers.

use super::ChunkError;
use lumio_voxel_contracts::{SCHEMA_IDS, sha256};
use std::sync::Arc;

const CHUNK_PAGE_SCHEMA: &str = "voxel-chunk-page";

/// Generated page-encoding identities. Not Schema columns.
const PAGE_ENCODINGS: &[&str] = &["Dense", "Sparse"];
/// Generated compression identities. Not a selected public default.
const COMPRESSION_CODECS: &[&str] = &["None", "Zstd", "Lz4"];

const V1_ENCODING: &str = "Dense";
const V1_CODEC: &str = "None";

/// Unpublished page envelope. Validation and sealing happen in `from_pages`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ChunkPage {
    encoding: String,
    compression: String,
    bytes: Vec<u8>,
    declared_digest: [u8; 32],
}

impl ChunkPage {
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
    fn seal(page: ChunkPage) -> Result<SealedPage, ChunkError> {
        let encoding = intern_name(&page.encoding, PAGE_ENCODINGS)?;
        let compression = intern_name(&page.compression, COMPRESSION_CODECS)?;
        if encoding != V1_ENCODING || compression != V1_CODEC {
            return Err(ChunkError::invalid_handle());
        }
        let digest = sha256(&page.bytes);
        if digest != page.declared_digest {
            return Err(ChunkError::evidence_digest_mismatch());
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
pub struct ChunkPayload {
    schema_id: &'static str,
    pages: Arc<[SealedPage]>,
    _extent_family: IsolatedCubicExtentFamily,
}

impl ChunkPayload {
    pub fn from_pages(pages: impl IntoIterator<Item = ChunkPage>) -> Result<Self, ChunkError> {
        let schema_id = SCHEMA_IDS
            .iter()
            .copied()
            .find(|id| *id == CHUNK_PAGE_SCHEMA)
            .expect("voxel-chunk-page must exist in generated SCHEMA_IDS");
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

fn intern_name(name: &str, generated: &[&'static str]) -> Result<&'static str, ChunkError> {
    generated
        .iter()
        .copied()
        .find(|item| *item == name)
        .ok_or_else(ChunkError::invalid_handle)
}
