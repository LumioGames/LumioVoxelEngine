use lumio_voxel_test_support::generated_clean::{self, LockedFile};

#[test]
fn sha256_empty_matches_published_digest() {
    assert_eq!(
        generated_clean::sha256_hex(b""),
        "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
    );
}
use lumio_voxel_test_support::workspace_root_from_manifest;
use std::fs;
use std::time::{SystemTime, UNIX_EPOCH};

#[test]
fn empty_gitkeep_matches_lock() {
    let root = workspace_root_from_manifest(env!("CARGO_MANIFEST_DIR"));
    let json = fs::read_to_string(root.join(generated_clean::LOCK_PATH)).unwrap();
    let locked = generated_clean::lock_from_json(&json);
    let generated = generated_clean::workspace_generated_dir(&root);
    let v = generated_clean::violations(&generated, &locked);
    assert!(v.is_empty(), "{v:?}");
}

#[test]
fn handwritten_generated_file_is_rejected() {
    // Runs against a temp tree: writing the rogue file into the real generated
    // dir races with empty_gitkeep_matches_lock, which scans that same dir.
    let dir = std::env::temp_dir().join("lve-generated-clean-handwritten");
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    fs::write(dir.join(".gitkeep"), b"").unwrap();
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    fs::write(
        dir.join(format!("handwritten-{nonce}.rs")),
        b"pub struct FakeDto;",
    )
    .unwrap();
    let locked = vec![LockedFile {
        rel: ".gitkeep".into(),
        sha256: generated_clean::sha256_hex(b""),
    }];
    let v = generated_clean::violations(&dir, &locked);
    let _ = fs::remove_dir_all(&dir);
    assert!(
        v.iter().any(|s| s.contains("未锁定文件")),
        "expected handwritten file to fail, got {v:?}"
    );
}

#[test]
fn hash_mismatch_is_rejected() {
    let dir = std::env::temp_dir().join("lve-generated-clean-mismatch");
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    fs::write(dir.join(".gitkeep"), b"tampered").unwrap();
    let locked = vec![LockedFile {
        rel: ".gitkeep".into(),
        sha256: generated_clean::sha256_hex(b""),
    }];
    let v = generated_clean::violations(&dir, &locked);
    let _ = fs::remove_dir_all(&dir);
    assert!(
        v.iter().any(|s| s.contains("hash 与 lock 不一致")),
        "expected hash mismatch, got {v:?}"
    );
}
