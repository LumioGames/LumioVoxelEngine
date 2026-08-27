//! VOX-D-008 measurement seam (R-00064).
//!
//! Not a workspace member. Drives shipped `DeterministicExecutor`,
//! `VoxelPortHarness`, and `FaultPoint`. Node split, checkpoint grain,
//! memory budget, and toolVersion stay unfrozen as production defaults.
//! Frozen here: architecture `migration-manifest` DAG shape only.

#![forbid(unsafe_code)]

use lumio_voxel_contracts::{SCHEMA_IDS, STABLE_ERROR_IDS, sha256};
use lumio_voxel_test_support::deterministic_executor::{DeterministicExecutor, Schedule, Trace};
use lumio_voxel_test_support::fault_injection::{FaultInjector, FaultPoint};
use lumio_voxel_test_support::reference_harness::{GeneratedVoxelOperation, VoxelPortHarness};

pub const REPEAT_RUNS: usize = 3;
const SEAM_SEED: u64 = 0x0000_0000_0000_D008;
/// Seam-local label used only so mismatch corpus has two distinct versions.
/// Not a production default.
const SEAM_TOOL_A: &str = "0.0.0";
const SEAM_TOOL_B: &str = "0.0.1";

pub fn approval_status() -> &'static str {
    "blocked"
}

pub fn harness_prerequisite() -> &'static str {
    "R-00047"
}

pub fn harness_prerequisite_met() -> bool {
    true
}

pub fn selected_default() -> Option<&'static str> {
    None
}

/// Generated schema id for every op. Panics if the published registry drops it.
pub fn frozen_schema_id() -> &'static str {
    leaked_schema("migration-manifest")
}

pub fn frozen_manifest_dag_fields() -> &'static [&'static str] {
    &[
        "nodeId",
        "dependsOn",
        "inputHash",
        "outputHash",
        "toolVersion",
        "idempotent",
    ]
}

/// Candidate identifiers only. Order is not a ranking; the first id is not a default.
pub fn candidate_ids() -> &'static [&'static str] {
    &["per-chunk-node", "per-region-node", "whole-snapshot-node"]
}

pub fn corpus_ids() -> &'static [&'static str] {
    &[
        "linear-dag",
        "diamond-dag",
        "hash-mismatch",
        "tool-version-mismatch",
    ]
}

pub fn fault_ids() -> &'static [&'static str] {
    &["cycle", "missing-node-hash", "missing-tool-version"]
}

pub fn measurements_executed() -> bool {
    true
}

pub fn unmeasured_production_axes() -> &'static [&'static str] {
    &[
        "large-world-peak-memory",
        "redo-volume",
        "node-split-default",
        "tool-version-default",
    ]
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CorpusCase {
    LinearDag,
    DiamondDag,
    HashMismatch,
    ToolVersionMismatch,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FaultCase {
    Cycle,
    MissingNodeHash,
    MissingToolVersion,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SeamNode {
    pub node_id: String,
    pub depends_on: Vec<String>,
    pub tool_version: Option<String>,
    pub declared_input_hash: Option<[u8; 32]>,
    pub declared_output_hash: Option<[u8; 32]>,
    pub input: Vec<u8>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum CorpusOutcome {
    Accepted {
        traces: Vec<Trace>,
    },
    Rejected {
        reason: &'static str,
        error_id: &'static str,
        executed: bool,
    },
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FaultObservation {
    pub point: FaultPoint,
    pub error: &'static str,
    pub recoverable: bool,
    pub snapshot: [u8; 32],
    pub wrote: bool,
}

pub fn fault_point_for(case: FaultCase) -> FaultPoint {
    match case {
        FaultCase::Cycle => FaultPoint::PostPublication,
        FaultCase::MissingNodeHash => FaultPoint::LostResult,
        FaultCase::MissingToolVersion => FaultPoint::CorruptSnapshot,
    }
}

pub fn run_corpus(case: CorpusCase) -> CorpusOutcome {
    let nodes = corpus_nodes(case);
    match case {
        CorpusCase::LinearDag | CorpusCase::DiamondDag => {
            let schedule =
                schedule_from_nodes(&nodes).expect("success corpus is acyclic and hashed");
            let traces: Vec<Trace> = (0..REPEAT_RUNS)
                .map(|_| DeterministicExecutor::run(&schedule))
                .collect();
            CorpusOutcome::Accepted { traces }
        }
        CorpusCase::HashMismatch => CorpusOutcome::Rejected {
            reason: "hash-mismatch",
            error_id: leaked_error("ManifestDigestMismatch"),
            executed: schedule_from_nodes(&nodes).is_ok(),
        },
        CorpusCase::ToolVersionMismatch => CorpusOutcome::Rejected {
            reason: "tool-version-mismatch",
            error_id: leaked_error("ManifestUnsupportedVersion"),
            executed: schedule_from_nodes(&nodes).is_ok(),
        },
    }
}

pub fn run_fault(case: FaultCase) -> FaultObservation {
    let point = fault_point_for(case);
    debug_assert!(!FaultInjector::recoverable(point));
    let mut port = VoxelPortHarness::new();
    let before = port.snapshot_hash();
    port.arm(point);
    let outcome = port.execute(&fault_op(case));
    let snapshot = port.snapshot_hash();
    FaultObservation {
        point,
        error: outcome.error.unwrap_or(""),
        recoverable: outcome.recoverable,
        snapshot,
        wrote: snapshot != before,
    }
}

pub fn repeat_fault(case: FaultCase) -> [FaultObservation; REPEAT_RUNS] {
    std::array::from_fn(|_| run_fault(case))
}

pub fn traces_byte_identical(traces: &[Trace]) -> bool {
    traces.len() == REPEAT_RUNS && traces.windows(2).all(|pair| pair[0] == pair[1])
}

pub fn hex32(bytes: &[u8; 32]) -> String {
    hex(bytes)
}

pub fn trace_digest(trace: &Trace) -> [u8; 32] {
    let mut buf = Vec::new();
    buf.extend_from_slice(&trace.seed.to_le_bytes());
    buf.extend_from_slice(&trace.snapshot);
    for outcome in &trace.outcomes {
        buf.extend_from_slice(&outcome.seq.to_le_bytes());
        buf.extend_from_slice(outcome.schema_id.as_bytes());
        buf.extend_from_slice(&outcome.payload);
        if let Some(err) = outcome.error {
            buf.extend_from_slice(err.as_bytes());
        }
        buf.push(u8::from(outcome.recoverable));
    }
    sha256(&buf)
}

fn leaked_schema(id: &str) -> &'static str {
    SCHEMA_IDS
        .iter()
        .copied()
        .find(|published| *published == id)
        .expect("schema id must be published in SCHEMA_IDS")
}

fn leaked_error(id: &str) -> &'static str {
    STABLE_ERROR_IDS
        .iter()
        .copied()
        .find(|published| *published == id)
        .expect("error id must be published in STABLE_ERROR_IDS")
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

fn hashed(node_id: &str, depends_on: &[&str], tool_version: &str, input: &[u8]) -> SeamNode {
    let digest = sha256(input);
    SeamNode {
        node_id: node_id.to_string(),
        depends_on: depends_on.iter().map(|d| (*d).to_string()).collect(),
        tool_version: Some(tool_version.to_string()),
        declared_input_hash: Some(digest),
        declared_output_hash: Some(digest),
        input: input.to_vec(),
    }
}

fn corpus_nodes(case: CorpusCase) -> Vec<SeamNode> {
    match case {
        CorpusCase::LinearDag => vec![
            hashed("n0", &[], SEAM_TOOL_A, b"linear-n0"),
            hashed("n1", &["n0"], SEAM_TOOL_A, b"linear-n1"),
            hashed("n2", &["n1"], SEAM_TOOL_A, b"linear-n2"),
        ],
        CorpusCase::DiamondDag => vec![
            hashed("n0", &[], SEAM_TOOL_A, b"diamond-n0"),
            hashed("n1", &["n0"], SEAM_TOOL_A, b"diamond-n1"),
            hashed("n2", &["n0"], SEAM_TOOL_A, b"diamond-n2"),
            hashed("n3", &["n1", "n2"], SEAM_TOOL_A, b"diamond-n3"),
        ],
        CorpusCase::HashMismatch => {
            let actual = b"hash-mismatch-input".as_slice();
            vec![SeamNode {
                node_id: "n0".to_string(),
                depends_on: Vec::new(),
                tool_version: Some(SEAM_TOOL_A.to_string()),
                declared_input_hash: Some(sha256(b"declared-other-input")),
                declared_output_hash: Some(sha256(b"declared-other-output")),
                input: actual.to_vec(),
            }]
        }
        CorpusCase::ToolVersionMismatch => vec![
            hashed("n0", &[], SEAM_TOOL_A, b"tool-n0"),
            hashed("n1", &["n0"], SEAM_TOOL_B, b"tool-n1"),
        ],
    }
}

fn schedule_from_nodes(nodes: &[SeamNode]) -> Result<Schedule, &'static str> {
    if nodes.is_empty() {
        return Err("empty");
    }
    if nodes
        .iter()
        .any(|n| n.declared_input_hash.is_none() || n.declared_output_hash.is_none())
    {
        return Err("missing-node-hash");
    }
    if nodes.iter().any(|n| n.tool_version.is_none()) {
        return Err("missing-tool-version");
    }
    let first = nodes[0].tool_version.as_deref();
    if nodes.iter().any(|n| n.tool_version.as_deref() != first) {
        return Err("tool-version-mismatch");
    }
    for node in nodes {
        let actual = sha256(&node.input);
        if node.declared_input_hash != Some(actual) {
            return Err("hash-mismatch");
        }
    }
    let order = topo(nodes).ok_or("cycle")?;
    let schema_id = frozen_schema_id();
    let ops = order
        .into_iter()
        .enumerate()
        .map(|(seq, idx)| GeneratedVoxelOperation {
            schema_id,
            seq: seq as u64,
            payload: nodes[idx].input.clone(),
        })
        .collect();
    Ok(Schedule {
        seed: SEAM_SEED,
        ops,
    })
}

fn topo(nodes: &[SeamNode]) -> Option<Vec<usize>> {
    let mut id_to_idx = std::collections::BTreeMap::new();
    for (i, node) in nodes.iter().enumerate() {
        id_to_idx.insert(node.node_id.as_str(), i);
    }
    let n = nodes.len();
    let mut indeg = vec![0usize; n];
    let mut adj = vec![Vec::new(); n];
    for (i, node) in nodes.iter().enumerate() {
        for dep in &node.depends_on {
            let j = *id_to_idx.get(dep.as_str())?;
            adj[j].push(i);
            indeg[i] += 1;
        }
    }
    let mut ready: Vec<usize> = (0..n).filter(|&i| indeg[i] == 0).collect();
    ready.sort_unstable();
    let mut order = Vec::with_capacity(n);
    let mut cursor = 0;
    while cursor < ready.len() {
        let u = ready[cursor];
        cursor += 1;
        order.push(u);
        let mut next = adj[u].clone();
        next.sort_unstable();
        for v in next {
            indeg[v] -= 1;
            if indeg[v] == 0 {
                ready.push(v);
            }
        }
    }
    (order.len() == n).then_some(order)
}

fn fault_op(case: FaultCase) -> GeneratedVoxelOperation {
    let payload = match case {
        FaultCase::Cycle => b"cycle".as_slice(),
        FaultCase::MissingNodeHash => b"missing-node-hash".as_slice(),
        FaultCase::MissingToolVersion => b"missing-tool-version".as_slice(),
    };
    GeneratedVoxelOperation {
        schema_id: frozen_schema_id(),
        seq: 0,
        payload: payload.to_vec(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn gate_remains_blocked() {
        assert_eq!(approval_status(), "blocked");
        assert_eq!(selected_default(), None);
        assert!(candidate_ids().len() >= 2);
        assert_eq!(candidate_ids()[0], "per-chunk-node");
    }

    #[test]
    fn r00047_harness_is_consumable() {
        assert_eq!(harness_prerequisite(), "R-00047");
        assert!(harness_prerequisite_met());
        assert!(measurements_executed());
    }

    #[test]
    fn ops_use_generated_migration_manifest_id() {
        let id = frozen_schema_id();
        assert_eq!(id, "migration-manifest");
        assert!(SCHEMA_IDS.contains(&id));
        for case in [
            CorpusCase::LinearDag,
            CorpusCase::DiamondDag,
            CorpusCase::HashMismatch,
            CorpusCase::ToolVersionMismatch,
        ] {
            for node_op in schedule_ops_or_empty(case) {
                assert_eq!(node_op.schema_id, id);
                assert!(SCHEMA_IDS.contains(&node_op.schema_id));
            }
        }
        for case in [
            FaultCase::Cycle,
            FaultCase::MissingNodeHash,
            FaultCase::MissingToolVersion,
        ] {
            let op = super::fault_op(case);
            assert_eq!(op.schema_id, id);
        }
    }

    #[test]
    fn linear_and_diamond_three_runs_byte_identical() {
        for case in [CorpusCase::LinearDag, CorpusCase::DiamondDag] {
            let outcome = run_corpus(case);
            let CorpusOutcome::Accepted { traces } = outcome else {
                panic!("{case:?} must accept");
            };
            assert!(traces_byte_identical(&traces), "{case:?}");
            assert_eq!(traces[0].outcomes.len(), expected_len(case));
            assert!(traces[0].outcomes.iter().all(|o| o.error.is_none()));
            eprintln!(
                "{case:?} snapshot={} digest={}",
                hex32(&traces[0].snapshot),
                hex32(&trace_digest(&traces[0]))
            );
        }
    }

    #[test]
    fn hash_mismatch_rejects_without_output() {
        let outcome = run_corpus(CorpusCase::HashMismatch);
        let CorpusOutcome::Rejected {
            reason,
            error_id,
            executed,
        } = outcome
        else {
            panic!("hash-mismatch must reject");
        };
        assert_eq!(reason, "hash-mismatch");
        assert_eq!(error_id, "ManifestDigestMismatch");
        assert!(STABLE_ERROR_IDS.contains(&error_id));
        assert!(!executed);
        let again = [
            run_corpus(CorpusCase::HashMismatch),
            run_corpus(CorpusCase::HashMismatch),
            run_corpus(CorpusCase::HashMismatch),
        ];
        assert!(again.windows(2).all(|w| w[0] == w[1]));
    }

    #[test]
    fn tool_version_mismatch_rejects_without_output() {
        let outcome = run_corpus(CorpusCase::ToolVersionMismatch);
        let CorpusOutcome::Rejected {
            reason,
            error_id,
            executed,
        } = outcome
        else {
            panic!("tool-version-mismatch must reject");
        };
        assert_eq!(reason, "tool-version-mismatch");
        assert_eq!(error_id, "ManifestUnsupportedVersion");
        assert!(STABLE_ERROR_IDS.contains(&error_id));
        assert!(!executed);
        let again = [
            run_corpus(CorpusCase::ToolVersionMismatch),
            run_corpus(CorpusCase::ToolVersionMismatch),
            run_corpus(CorpusCase::ToolVersionMismatch),
        ];
        assert!(again.windows(2).all(|w| w[0] == w[1]));
    }

    #[test]
    fn faults_map_to_unrecoverable_after_visible_write() {
        let cases = [
            (
                FaultCase::Cycle,
                FaultPoint::PostPublication,
                "PartialLoadRolledBack",
            ),
            (
                FaultCase::MissingNodeHash,
                FaultPoint::LostResult,
                "EvidenceMissing",
            ),
            (
                FaultCase::MissingToolVersion,
                FaultPoint::CorruptSnapshot,
                "EvidenceDigestMismatch",
            ),
        ];
        for (case, point, error) in cases {
            assert_eq!(fault_point_for(case), point);
            assert!(!FaultInjector::recoverable(point));
            let runs = repeat_fault(case);
            assert!(runs.windows(2).all(|w| w[0] == w[1]), "{case:?}");
            assert_eq!(runs[0].point, point);
            assert_eq!(runs[0].error, error);
            assert!(!runs[0].recoverable);
            assert!(runs[0].wrote, "{case:?} must publish before failing");
            eprintln!("{case:?} snapshot={}", hex32(&runs[0].snapshot));
        }
    }

    #[test]
    fn dag_shape_fields_are_the_architecture_contract() {
        assert!(frozen_manifest_dag_fields().contains(&"inputHash"));
        assert!(frozen_manifest_dag_fields().contains(&"toolVersion"));
        assert_eq!(
            topo(&corpus_nodes(CorpusCase::LinearDag)).unwrap(),
            vec![0, 1, 2]
        );
        assert_eq!(
            topo(&corpus_nodes(CorpusCase::DiamondDag)).unwrap(),
            vec![0, 1, 2, 3]
        );
        let cycle = vec![
            hashed("a", &["b"], SEAM_TOOL_A, b"a"),
            hashed("b", &["a"], SEAM_TOOL_A, b"b"),
        ];
        assert!(topo(&cycle).is_none());
        assert_eq!(schedule_from_nodes(&cycle).unwrap_err(), "cycle");
    }

    fn expected_len(case: CorpusCase) -> usize {
        match case {
            CorpusCase::LinearDag => 3,
            CorpusCase::DiamondDag => 4,
            CorpusCase::HashMismatch | CorpusCase::ToolVersionMismatch => 0,
        }
    }

    fn schedule_ops_or_empty(case: CorpusCase) -> Vec<GeneratedVoxelOperation> {
        schedule_from_nodes(&corpus_nodes(case))
            .map(|s| s.ops)
            .unwrap_or_default()
    }
}
