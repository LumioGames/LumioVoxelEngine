//! Directory lookup mapped onto generated `SECTION_PRESENCE`. No payload leak.

#![forbid(unsafe_code)]

use super::QueryError;
use lumio_voxel_contracts::voxel_world::SECTION_PRESENCE;
use lumio_voxel_domain::publication::PublishedReadView;
use lumio_voxel_domain::section::SectionError;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SectionAccessResult {
    section_id: String,
    presence: &'static str,
    schema_id: Option<&'static str>,
}

impl SectionAccessResult {
    pub fn section_id(&self) -> &str {
        &self.section_id
    }

    pub fn presence(&self) -> &'static str {
        self.presence
    }

    pub fn schema_id(&self) -> Option<&'static str> {
        self.schema_id
    }
}

pub(super) fn intern_presence(name: &str) -> Result<&'static str, QueryError> {
    SECTION_PRESENCE
        .iter()
        .copied()
        .find(|item| *item == name)
        .ok_or_else(QueryError::invalid_handle)
}

pub(super) fn access(
    view: &PublishedReadView,
    section_id: &str,
) -> Result<SectionAccessResult, QueryError> {
    match view.directory().lookup(section_id) {
        Ok(Some(slot)) => {
            let presence = intern_presence(slot.presence())?;
            let schema_id = if presence == "Ready" {
                Some(
                    slot.payload()
                        .map(|payload| payload.schema_id())
                        .ok_or_else(QueryError::invalid_handle)?,
                )
            } else {
                None
            };
            Ok(SectionAccessResult {
                section_id: section_id.to_string(),
                presence,
                schema_id,
            })
        }
        Ok(None) => Ok(SectionAccessResult {
            section_id: section_id.to_string(),
            presence: intern_presence("Unchanged")?,
            schema_id: None,
        }),
        Err(err) => Err(map_section(err)),
    }
}

fn map_section(err: SectionError) -> QueryError {
    match err {
        // 键错误已经带着契约错误码,原样透传比重新分类更诚实。
        SectionError::Key(key) => QueryError::from_key(key),
        _ => QueryError::invalid_handle(),
    }
}
