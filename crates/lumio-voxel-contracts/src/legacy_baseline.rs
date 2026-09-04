//! 已废弃基线 `LGE-V1.4-2026-08-27` 的产物标识符,隔离在此。
//!
//! **这些名字已废弃,不得当作分层语义使用。** 它们来自 `generated/` 下那份只读镜像:
//! 生成源仓 `LumioGameEngineArchitecture` 已不存在,镜像永远不可能重新生成,而它用
//! `Chunk` 指代 16×16×16 的数据单元——按活契约 `lumio.voxel-world.v1`,那个单元叫
//! **Section**(见 [`crate::voxel_world`])。
//!
//! 活代码仍然要报出这些字符串,只因为它们是**冻结产物的 id**(schema 名、状态机名),
//! 不是可以本仓自行改写的语义。把它们收在这一个模块里,是为了让「16³ 数据单元叫 chunk」
//! 这个错误命名在整个工作区只剩这一处出现,并且带着为什么还在的解释。
//!
//! 上游重新发布体素产物时,这里应当整体消失,而不是被逐点修补。

/// 冻结基线 id。
pub const BASELINE: &str = "LGE-V1.4-2026-08-27";

/// Section 页在废弃基线里的 schema id(该基线把 Section 叫 chunk)。
pub const SECTION_PAGE_SCHEMA_ID: &str = "voxel-chunk-page";

/// Section 驻留状态机在废弃基线里的 machine id(同上)。
pub const SECTION_RESIDENCY_MACHINE_ID: &str = "VoxelChunkResidency";
