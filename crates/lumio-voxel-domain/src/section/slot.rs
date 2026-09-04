//! 四态 presence 槽位,与契约 `diffDispatch.presence` 一一对应。
//!
//! `Unchanged` 是「该 Section 相对原始地图没有改动」,以零字节短票表达;它与 `Pending`
//! (已请求未送达)、`Unavailable`(当前不可提供)在语法上必须可区分,三者都不得被物化
//! 成空气。

use super::SectionError;
use super::payload::SectionPayload;
use lumio_voxel_contracts::voxel_world::{self as vw, SECTION_PRESENCE};
use std::sync::Arc;

#[derive(Clone, Debug, PartialEq, Eq)]
enum Inner {
    Ready(Arc<SectionPayload>),
    Unchanged,
    Pending,
    Unavailable,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SectionSlot {
    inner: Inner,
}

impl SectionSlot {
    pub fn ready(payload: SectionPayload) -> Self {
        Self {
            inner: Inner::Ready(Arc::new(payload)),
        }
    }

    pub fn unchanged() -> Self {
        Self {
            inner: Inner::Unchanged,
        }
    }

    pub fn pending() -> Self {
        Self {
            inner: Inner::Pending,
        }
    }

    pub fn unavailable() -> Self {
        Self {
            inner: Inner::Unavailable,
        }
    }

    pub fn presence(&self) -> &'static str {
        let name = match self.inner {
            Inner::Ready(_) => "Ready",
            Inner::Unchanged => "Unchanged",
            Inner::Pending => "Pending",
            Inner::Unavailable => "Unavailable",
        };
        vw::intern_presence(name).expect("constructor presence is a contract SECTION_PRESENCE name")
    }

    pub fn payload(&self) -> Option<&SectionPayload> {
        match &self.inner {
            Inner::Ready(payload) => Some(payload),
            Inner::Unchanged | Inner::Pending | Inner::Unavailable => None,
        }
    }

    /// Illegal transitions fail before any caller-visible mutation.
    pub fn try_convert(
        &self,
        presence: &str,
        payload: Option<SectionPayload>,
    ) -> Result<Self, SectionError> {
        let to = intern_presence(presence)?;
        let from = self.presence();

        if from == to {
            return match (to, payload) {
                ("Ready", Some(page)) => Ok(Self::ready(page)),
                ("Ready", None) => Ok(self.clone()),
                (_, None) => Ok(self.clone()),
                (_, Some(_)) => Err(SectionError::invalid_handle()),
            };
        }

        if to == "Ready" && payload.is_none() {
            return Err(SectionError::section_unavailable());
        }

        match (from, to, payload) {
            ("Unchanged", "Pending", None) => Ok(Self::pending()),
            ("Pending", "Ready", Some(page)) => Ok(Self::ready(page)),
            ("Pending", "Unavailable", None) => Ok(Self::unavailable()),
            ("Pending", "Unchanged", None) => Ok(Self::unchanged()),
            ("Ready", "Unchanged", None) => Ok(Self::unchanged()),
            ("Unavailable", "Pending", None) => Ok(Self::pending()),
            ("Unavailable", "Unchanged", None) => Ok(Self::unchanged()),
            ("Unavailable", "Ready", _) | ("Unchanged", "Ready", _) => {
                Err(SectionError::section_unavailable())
            }
            _ => Err(SectionError::invalid_handle()),
        }
    }
}

fn intern_presence(name: &str) -> Result<&'static str, SectionError> {
    debug_assert_eq!(SECTION_PRESENCE.len(), 4);
    vw::intern_presence(name).ok_or_else(SectionError::invalid_handle)
}
