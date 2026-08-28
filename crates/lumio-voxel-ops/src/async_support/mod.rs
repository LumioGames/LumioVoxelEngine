//! Origin token, bounded job port, and completion envelope (R-00068).
//!
//! Production code must not depend on `lumio-voxel-test-support`.

#![forbid(unsafe_code)]

pub mod bounded_port;
pub mod completion;
pub mod origin;

pub use bounded_port::{BoundedJobPort, SubmitError, full_load_action};
pub use completion::{CompletionDisposition, CompletionEnvelope, validate_completion};
pub use origin::{APPLY_PHASES, OriginEnvelope, OriginError, OriginToken};
