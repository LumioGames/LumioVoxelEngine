# Verification Matrix

| Property | Unit/model | Integration | Architecture fixture/evidence |
|---|---|---|---|
| Revision monotonicity | reservation/finalization property tests | concurrent publish/read | source revision fixture where defined |
| Atomic cut | published-root race model | mutation/query and snapshot/query | deterministic schedule trace |
| Prepare purity | state snapshot differential | failed batch request | no-visible-side-effect evidence |
| TxnId replay | ledger model | retry across runtime adapter | original generated receipt equivalence |
| Four chunk states | exhaustive slot model | query + streaming flow | generated status mapping fixture |
| Dirty ack coverage | frontier property test | snapshot, later mutation, old ack | persisted cut evidence |
| Snapshot barrier | lease instrumentation | encode/store worker handoff | no-I/O-inside-barrier trace |
| Restore atomicity | shadow builder fault matrix | restore/query and restore/stream races | old-or-new root trace |
| Streaming staleness | ticket generation model | mutation/restore/close races | stale completion evidence |
| World fault containment | lifecycle/fault model | two-World runtime | unaffected-World progress trace |
| LocalEmbedded isolation | object graph no-alias audit | local vs remote topology | generated message/outcome equivalence |
| Contract integrity | adapter exhaustive tests | end-to-end boundary calls | architecture-generated fixture hashes |
| Dependency direction | graph checker | workspace build | L0–L5 report |
| Decision compliance | config validation | startup with unresolved gate | no compiled defaults scan |

## Evidence record schema (internal test artifact)

Each run records baseline `LGE-V1.3-2026-08-27`, architecture input hash, repo/implementation commit, fixture registry revision, deterministic scheduler seed/trace, approved decision-source revisions, and pass/fail diagnostics. This evidence schema is test tooling only; it is not a public engine protocol.
