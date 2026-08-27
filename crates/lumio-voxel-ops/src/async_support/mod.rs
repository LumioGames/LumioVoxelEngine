//! Origin token, bounded job port, and completion envelope (R-00068).
//!
//! Production code must not depend on `lumio-voxel-test-support`.

#![forbid(unsafe_code)]

pub mod bounded_port;
pub mod completion;
pub mod origin;

pub use bounded_port::{full_load_action, BoundedJobPort, SubmitError};
pub use completion::{validate_completion, CompletionDisposition, CompletionEnvelope};
pub use origin::{OriginEnvelope, OriginError, OriginToken, APPLY_PHASES};
