use lumio_voxel_contracts::{
    BASELINE_ID, ContractLoadError, verify_artifact_hashes, verify_artifact_hashes_at,
};
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

fn generated() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("generated")
}

fn copy_tree(src: &Path, dst: &Path) {
    for entry in fs::read_dir(src).unwrap() {
        let entry = entry.unwrap();
        let from = entry.path();
        let to = dst.join(entry.file_name());
        if from.is_dir() {
            fs::create_dir_all(&to).unwrap();
            copy_tree(&from, &to);
        } else {
            fs::create_dir_all(dst).unwrap();
            fs::copy(&from, &to).unwrap();
        }
    }
}

#[test]
fn published_hashes_match_locked_packages() {
    verify_artifact_hashes().expect("generated artifacts must verify");
}

#[test]
fn baseline_is_v14() {
    assert_eq!(BASELINE_ID, "LGE-V1.4-2026-08-27");
}

#[test]
fn tamper_fails_then_restore_passes() {
    let src = generated();
    verify_artifact_hashes_at(&src).unwrap();
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let tmp = std::env::temp_dir().join(format!("lve-r45-tamper-{nonce}"));
    let _ = fs::remove_dir_all(&tmp);
    fs::create_dir_all(&tmp).unwrap();
    copy_tree(&src, &tmp);
    verify_artifact_hashes_at(&tmp).unwrap();

    let victim = tmp.join("rust/lumio-gen-contract-types/src/lib.rs");
    let original = fs::read(&victim).unwrap();
    fs::write(&victim, b"pub struct FakeDto;\n").unwrap();
    let err = verify_artifact_hashes_at(&tmp).unwrap_err();
    assert!(
        matches!(err, ContractLoadError::HashMismatch { .. }),
        "expected HashMismatch, got {err:?}"
    );

    fs::write(&victim, original).unwrap();
    verify_artifact_hashes_at(&tmp).unwrap();
    let _ = fs::remove_dir_all(&tmp);
}
