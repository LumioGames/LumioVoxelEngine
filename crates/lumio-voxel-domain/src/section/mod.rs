//! Section:16×16×16 = 4096 格的数据单元——最小同步单位、驻留单位、版本锚点。
//!
//! 这里是 Section 的载荷、四态 presence 槽位与 COW 目录根。键与 Chunk 派生在
//! [`crate::key`];Chunk 是 16 个 Section 摞成的列容器,不携带数据。
//!
//! Presence 与契约 `diffDispatch.presence` 一一对应(`voxel_world::SECTION_PRESENCE`),
//! 不是驻留状态机。IsolatedCubicExtentFamily 是适配器内部的,不暴露 `section_size` /
//! `page_size`。

#![forbid(unsafe_code)]

mod delta;
mod directory;
mod dirty;
mod payload;
mod replacement;
mod slot;

pub use delta::{SectionDeltaBuilder, StagedEdit};
pub use directory::{SectionDirectoryBuilder, SectionDirectoryRoot};
pub use dirty::{
    CoveredSectionAck, DirtyCoverage, DirtyError, DirtyFrontier, DurabilityAckContext,
    DurabilityAckEvidence,
};
pub use payload::{SectionPage, SectionPayload};
pub use replacement::{ReplacementSet, SectionReplacement};
pub use slot::SectionSlot;

pub use crate::key::{KeyError, SectionId};

use lumio_voxel_contracts::STABLE_ERROR_IDS;
use lumio_voxel_contracts::voxel_world as vw;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SectionError {
    /// 键语法 / 定义域。错误码由 [`KeyError`] 按契约规则给出。
    Key(KeyError),
    /// 页载荷摘要在解释之前就对不上(契约 `page.digest-before-interpretation`)。
    PageDigestMismatch { error_id: &'static str },
    /// 引擎通用的句柄 / 状态非法。活契约没有对应错误码,沿用废弃镜像的稳定 id。
    InvalidHandle { error_id: &'static str },
    /// Section 当前不可提供。缺块永不等于空气。
    SectionUnavailable { error_id: &'static str },
}

impl SectionError {
    pub fn error_id(&self) -> &'static str {
        match self {
            Self::Key(err) => err.error_id(),
            Self::PageDigestMismatch { error_id }
            | Self::InvalidHandle { error_id }
            | Self::SectionUnavailable { error_id } => error_id,
        }
    }

    fn page_digest_mismatch() -> Self {
        Self::PageDigestMismatch {
            error_id: contract_error(vw::PAGE_DIGEST_MISMATCH),
        }
    }

    fn invalid_handle() -> Self {
        Self::InvalidHandle {
            error_id: stable_error("InvalidHandle"),
        }
    }

    fn section_unavailable() -> Self {
        Self::SectionUnavailable {
            error_id: contract_error(vw::SECTION_UNAVAILABLE),
        }
    }
}

impl From<KeyError> for SectionError {
    fn from(err: KeyError) -> Self {
        Self::Key(err)
    }
}

impl std::fmt::Display for SectionError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.error_id())
    }
}

impl std::error::Error for SectionError {}

fn stable_error(id: &'static str) -> &'static str {
    STABLE_ERROR_IDS
        .iter()
        .copied()
        .find(|candidate| *candidate == id)
        .expect("mapped error id must exist in the frozen mirror's STABLE_ERROR_IDS")
}

fn contract_error(id: &'static str) -> &'static str {
    vw::intern_error_code(id).expect("mapped error id must exist in the contract errorCodes")
}
