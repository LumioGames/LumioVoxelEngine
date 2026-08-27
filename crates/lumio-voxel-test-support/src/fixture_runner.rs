//! Fixture runner: seed + sequence + expected/actual + hash, min replay.

use crate::deterministic_executor::Trace;
use crate::reference_harness::{GeneratedVoxelOperation, VoxelPortHarness};
use lumio_voxel_contracts::SCHEMA_IDS;
use std::path::Path;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FixtureResult {
    pub path: String,
    pub seed: u64,
    pub trace: Trace,
    pub expected_error: Option<String>,
    pub passed: bool,
}

pub fn run_fixture(path: &Path, port: &mut VoxelPortHarness) -> Result<FixtureResult, String> {
    let text = std::fs::read_to_string(path).map_err(|e| e.to_string())?;
    let seed = json_u64(&text, "seed").unwrap_or(0);
    let expect_error = json_string(&text, "expectError");
    let mut ops = Vec::new();
    if let Some(arr) = json_array_objects(&text, "ops") {
        for (i, obj) in arr.iter().enumerate() {
            let schema = json_string(obj, "schema_id").unwrap_or_else(|| "voxel-query".into());
            let leaked = SCHEMA_IDS
                .iter()
                .copied()
                .find(|id| *id == schema)
                .ok_or_else(|| format!("unknown schema_id {schema}"))?;
            let seq = json_u64(obj, "seq").unwrap_or(i as u64);
            let payload = json_string(obj, "payload").unwrap_or_default().into_bytes();
            ops.push(GeneratedVoxelOperation {
                schema_id: leaked,
                seq,
                payload,
            });
        }
    }
    let mut outcomes = Vec::with_capacity(ops.len());
    for op in &ops {
        outcomes.push(port.execute(op));
    }
    let trace = Trace {
        seed,
        outcomes,
        snapshot: port.snapshot_hash(),
    };
    let had_error = trace.outcomes.iter().any(|o| o.error.is_some());
    let passed = match &expect_error {
        None => !had_error,
        Some(want) => trace
            .outcomes
            .iter()
            .any(|o| o.error == Some(want.as_str())),
    };
    Ok(FixtureResult {
        path: path.display().to_string(),
        seed,
        trace,
        expected_error: expect_error,
        passed,
    })
}

fn json_string(obj: &str, key: &str) -> Option<String> {
    let pat = format!("\"{key}\":\"");
    let i = obj.find(&pat)?;
    let rest = &obj[i + pat.len()..];
    let end = rest.find('"')?;
    Some(rest[..end].to_string())
}

fn json_u64(obj: &str, key: &str) -> Option<u64> {
    let pat = format!("\"{key}\":");
    let i = obj.find(&pat)?;
    let rest = obj[i + pat.len()..].trim_start();
    let digits: String = rest.chars().take_while(|c| c.is_ascii_digit()).collect();
    digits.parse().ok()
}

fn json_array_objects<'a>(obj: &'a str, key: &str) -> Option<Vec<&'a str>> {
    let pat = format!("\"{key}\":[");
    let i = obj.find(&pat)?;
    let rest = &obj[i + pat.len()..];
    let end = rest.find(']')?;
    let inner = &rest[..end];
    let mut out = Vec::new();
    for part in inner.split("},{") {
        out.push(part);
    }
    Some(out)
}
