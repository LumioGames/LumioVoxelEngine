# Known Gaps and Hard Stops

A gap below blocks only the affected implementation task. It does not authorize guessing or redesign.

- **Repository commit unavailable:** no implementation task may claim repository-grounded file placement.
- **ADR originals not accessible in cloned content:** reconcile ADR 0001–0006 before W0 exit.
- **Module README originals not accessible in cloned content:** reconcile module ownership before W0 exit.
- **`CRATE_CONTRACT` unresolved:** W0 must bind this alias to the exact frozen crate before any owned-file task starts.
- **`CRATE_DOMAIN` unresolved:** W0 must bind this alias to the exact frozen crate before any owned-file task starts.
- **`CRATE_PERSISTENCE` unresolved:** W0 must bind this alias to the exact frozen crate before any owned-file task starts.
- **`CRATE_STREAMING` unresolved:** W0 must bind this alias to the exact frozen crate before any owned-file task starts.
- **`CRATE_RUNTIME` unresolved:** W0 must bind this alias to the exact frozen crate before any owned-file task starts.
- **`CRATE_FFI` unresolved:** W0 must bind this alias to the exact frozen crate before any owned-file task starts.
- **`CRATE_TESTKIT` unresolved:** W0 must bind this alias to the exact frozen crate before any owned-file task starts.

- `VOX-D-001`–`VOX-D-008` remain unapproved by instruction; cards list their blocked behavior.
- This package deliberately does not choose queue depth, concurrency, timeout, retry/backoff, retention, pin/cache budget, eviction, or codec policy values.
- Generated public field/error/capability/fixture details must be consumed from the architecture output at implementation time; they are not restated here.
