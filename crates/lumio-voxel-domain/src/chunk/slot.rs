//! Four-state availability slot mapped 1:1 onto generated `CHUNK_PRESENCE`.

use super::ChunkError;
use super::payload::ChunkPayload;
use lumio_voxel_contracts::CHUNK_PRESENCE;
use std::sync::Arc;

#[derive(Clone, Debug, PartialEq, Eq)]
enum Inner {
    Ready(Arc<ChunkPayload>),
    NotLoaded,
    Pending,
    Unavailable,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ChunkSlot {
    inner: Inner,
}

impl ChunkSlot {
    pub fn ready(payload: ChunkPayload) -> Self {
        Self {
            inner: Inner::Ready(Arc::new(payload)),
        }
    }

    pub fn not_loaded() -> Self {
        Self {
            inner: Inner::NotLoaded,
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
            Inner::NotLoaded => "NotLoaded",
            Inner::Pending => "Pending",
            Inner::Unavailable => "Unavailable",
        };
        intern_presence(name).expect("constructor presence is a generated CHUNK_PRESENCE name")
    }

    pub fn payload(&self) -> Option<&ChunkPayload> {
        match &self.inner {
            Inner::Ready(payload) => Some(payload),
            Inner::NotLoaded | Inner::Pending | Inner::Unavailable => None,
        }
    }

    /// Illegal transitions fail before any caller-visible mutation.
    pub fn try_convert(
        &self,
        presence: &str,
        payload: Option<ChunkPayload>,
    ) -> Result<Self, ChunkError> {
        let to = intern_presence(presence)?;
        let from = self.presence();

        if from == to {
            return match (to, payload) {
                ("Ready", Some(page)) => Ok(Self::ready(page)),
                ("Ready", None) => Ok(self.clone()),
                (_, None) => Ok(self.clone()),
                (_, Some(_)) => Err(ChunkError::invalid_handle()),
            };
        }

        if to == "Ready" && payload.is_none() {
            return Err(ChunkError::chunk_unavailable());
        }

        match (from, to, payload) {
            ("NotLoaded", "Pending", None) => Ok(Self::pending()),
            ("Pending", "Ready", Some(page)) => Ok(Self::ready(page)),
            ("Pending", "Unavailable", None) => Ok(Self::unavailable()),
            ("Pending", "NotLoaded", None) => Ok(Self::not_loaded()),
            ("Ready", "NotLoaded", None) => Ok(Self::not_loaded()),
            ("Unavailable", "Pending", None) => Ok(Self::pending()),
            ("Unavailable", "NotLoaded", None) => Ok(Self::not_loaded()),
            ("Unavailable", "Ready", _) | ("NotLoaded", "Ready", _) => {
                Err(ChunkError::chunk_unavailable())
            }
            _ => Err(ChunkError::invalid_handle()),
        }
    }
}

fn intern_presence(name: &str) -> Result<&'static str, ChunkError> {
    CHUNK_PRESENCE
        .iter()
        .copied()
        .find(|item| *item == name)
        .ok_or_else(ChunkError::invalid_handle)
}
