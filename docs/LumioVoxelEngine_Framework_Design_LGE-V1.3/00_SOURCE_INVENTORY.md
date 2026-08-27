# Source Inventory and Precedence

- Architecture baseline detected/required: `LGE-V1.3-2026-08-27`.
- Repository inspected at commit: `UNAVAILABLE`.
- Precedence: architecture source of truth → effective ADRs → frozen module README boundaries → this implementation design → task cards.
- This package does not redefine public ABI fields, Manifest fields, ErrorCode values, capability identifiers, fixture identifiers, or numerical policy values.

## Physical crate aliases

Aliases below are resolved only from repository text. `SOURCE_CRATE_MAP_REQUIRED` is a hard implementation gate, not permission to invent a crate.

| Alias | Detected frozen crate |
|---|---|
| `CRATE_CONTRACT` | `SOURCE_CRATE_MAP_REQUIRED` |
| `CRATE_DOMAIN` | `SOURCE_CRATE_MAP_REQUIRED` |
| `CRATE_PERSISTENCE` | `SOURCE_CRATE_MAP_REQUIRED` |
| `CRATE_STREAMING` | `SOURCE_CRATE_MAP_REQUIRED` |
| `CRATE_RUNTIME` | `SOURCE_CRATE_MAP_REQUIRED` |
| `CRATE_FFI` | `SOURCE_CRATE_MAP_REQUIRED` |
| `CRATE_TESTKIT` | `SOURCE_CRATE_MAP_REQUIRED` |

## Detected crate evidence

| Candidate | Source locations |
|---|---|
| _none recovered_ | Repository/ADR crate-map source must be made available before crate scaffolding. |

## Inputs

| Path | SHA-256 | Bytes | Role |
|---|---|---:|---|
| `Pasted text(6).txt` | `9dd08ad3e3a1b5feb63c7ebfee62455b6022c29ed3d4c67467d0afe9620f2227` | 21311 | uploaded input |
| `LumioGameEngine_Architecture_v0.3.md` | `61fda90c78bd153154e17ea318d37b6be3fdb1e5c592e1b7ca270de798f2dfb1` | 428 | uploaded input |

## Repository ADR files observed

- No ADR file was accessible in the clone; treat this as a source-gap gate.

## Repository module README files observed

- No module README file was accessible in the clone; treat this as a source-gap gate.
