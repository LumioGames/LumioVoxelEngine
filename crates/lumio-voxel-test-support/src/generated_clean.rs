//! Generated-contract directory must match the hash lockfile (R-00041).
//! Handwritten files in the generated tree are rejected.

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

pub const GENERATED_DIR: &str = "crates/lumio-voxel-contracts/generated";
pub const LOCK_PATH: &str = "tools/architecture/generated-lock.json";

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LockedFile {
    pub rel: String,
    pub sha256: String,
}

pub fn violations(generated_root: &Path, locked: &[LockedFile]) -> Vec<String> {
    let mut out = Vec::new();
    let mut expected: BTreeMap<&str, &str> = BTreeMap::new();
    for item in locked {
        expected.insert(item.rel.as_str(), item.sha256.as_str());
    }

    if !generated_root.exists() {
        if !locked.is_empty() {
            out.push(format!(
                "生成目录缺失但 lock 非空: {}",
                generated_root.display()
            ));
        }
        return out;
    }

    let mut found = BTreeMap::new();
    walk(generated_root, generated_root, &mut found);
    for (rel, hash) in &found {
        match expected.get(rel.as_str()) {
            None => out.push(format!("生成目录出现未锁定文件（疑似手改）: {rel}")),
            Some(want) if *want != hash.as_str() => {
                out.push(format!("生成文件 hash 与 lock 不一致: {rel}"));
            }
            Some(_) => {}
        }
    }
    for rel in expected.keys() {
        if !found.contains_key(*rel) {
            out.push(format!("lock 中的生成文件缺失: {rel}"));
        }
    }
    out.sort();
    out
}

/// Deliberately not `lumio_voxel_contracts::sha256`: that hasher is itself a locked entry
/// under `GENERATED_DIR`, so auditing the tree with it would let a tampered generated file
/// certify its own lock hash. See ADR 0010; `tests/sha256_kat.rs` keeps both copies pinned
/// to the FIPS 180-4 answers and proves they still agree.
pub fn sha256_hex(bytes: &[u8]) -> String {
    sha256(bytes)
}

fn walk(root: &Path, dir: &Path, found: &mut BTreeMap<String, String>) {
    let entries = match fs::read_dir(dir) {
        Ok(v) => v,
        Err(_) => return,
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            walk(root, &path, found);
            continue;
        }
        let rel = path
            .strip_prefix(root)
            .unwrap_or(&path)
            .to_string_lossy()
            .replace('\\', "/");
        if rel == ".gitkeep" {
            let bytes = fs::read(&path).unwrap_or_default();
            found.insert(rel, sha256_hex(&bytes));
            continue;
        }
        let bytes = fs::read(&path).unwrap_or_default();
        found.insert(rel, sha256_hex(&bytes));
    }
}

/// SHA-256 without extra crates (FIPS 180-4, one-block + multi-block).
fn sha256(data: &[u8]) -> String {
    let mut h: [u32; 8] = [
        0x6a09e667, 0xbb67ae85, 0x3c6ef372, 0xa54ff53a, 0x510e527f, 0x9b05688c, 0x1f83d9ab,
        0x5be0cd19,
    ];
    let bit_len = (data.len() as u64) * 8;
    let mut buf = data.to_vec();
    buf.push(0x80);
    while (buf.len() % 64) != 56 {
        buf.push(0);
    }
    buf.extend_from_slice(&bit_len.to_be_bytes());
    for block in buf.chunks(64) {
        let mut w = [0u32; 64];
        for i in 0..16 {
            w[i] = u32::from_be_bytes(block[i * 4..i * 4 + 4].try_into().unwrap());
        }
        for i in 16..64 {
            let s0 = w[i - 15].rotate_right(7) ^ w[i - 15].rotate_right(18) ^ (w[i - 15] >> 3);
            let s1 = w[i - 2].rotate_right(17) ^ w[i - 2].rotate_right(19) ^ (w[i - 2] >> 10);
            w[i] = w[i - 16]
                .wrapping_add(s0)
                .wrapping_add(w[i - 7])
                .wrapping_add(s1);
        }
        let mut a = h;
        const K: [u32; 64] = [
            0x428a2f98, 0x71374491, 0xb5c0fbcf, 0xe9b5dba5, 0x3956c25b, 0x59f111f1, 0x923f82a4,
            0xab1c5ed5, 0xd807aa98, 0x12835b01, 0x243185be, 0x550c7dc3, 0x72be5d74, 0x80deb1fe,
            0x9bdc06a7, 0xc19bf174, 0xe49b69c1, 0xefbe4786, 0x0fc19dc6, 0x240ca1cc, 0x2de92c6f,
            0x4a7484aa, 0x5cb0a9dc, 0x76f988da, 0x983e5152, 0xa831c66d, 0xb00327c8, 0xbf597fc7,
            0xc6e00bf3, 0xd5a79147, 0x06ca6351, 0x14292967, 0x27b70a85, 0x2e1b2138, 0x4d2c6dfc,
            0x53380d13, 0x650a7354, 0x766a0abb, 0x81c2c92e, 0x92722c85, 0xa2bfe8a1, 0xa81a664b,
            0xc24b8b70, 0xc76c51a3, 0xd192e819, 0xd6990624, 0xf40e3585, 0x106aa070, 0x19a4c116,
            0x1e376c08, 0x2748774c, 0x34b0bcb5, 0x391c0cb3, 0x4ed8aa4a, 0x5b9cca4f, 0x682e6ff3,
            0x748f82ee, 0x78a5636f, 0x84c87814, 0x8cc70208, 0x90befffa, 0xa4506ceb, 0xbef9a3f7,
            0xc67178f2,
        ];
        for i in 0..64 {
            let s1 = a[4].rotate_right(6) ^ a[4].rotate_right(11) ^ a[4].rotate_right(25);
            let ch = (a[4] & a[5]) ^ ((!a[4]) & a[6]);
            let t1 = a[7]
                .wrapping_add(s1)
                .wrapping_add(ch)
                .wrapping_add(K[i])
                .wrapping_add(w[i]);
            let s0 = a[0].rotate_right(2) ^ a[0].rotate_right(13) ^ a[0].rotate_right(22);
            let maj = (a[0] & a[1]) ^ (a[0] & a[2]) ^ (a[1] & a[2]);
            let t2 = s0.wrapping_add(maj);
            a[7] = a[6];
            a[6] = a[5];
            a[5] = a[4];
            a[4] = a[3].wrapping_add(t1);
            a[3] = a[2];
            a[2] = a[1];
            a[1] = a[0];
            a[0] = t1.wrapping_add(t2);
        }
        for i in 0..8 {
            h[i] = h[i].wrapping_add(a[i]);
        }
    }
    h.iter().map(|w| format!("{w:08x}")).collect()
}

pub fn lock_from_json(json: &str) -> Vec<LockedFile> {
    let mut files = Vec::new();
    if let Some(start) = json.find('[')
        && let Some(end) = json.rfind(']')
    {
        let inner = &json[start + 1..end];
        for obj in inner.split('}').filter(|s| s.contains("rel")) {
            let rel = extract_string(obj, "rel");
            let sha = extract_string(obj, "sha256");
            if let (Some(rel), Some(sha256)) = (rel, sha) {
                files.push(LockedFile { rel, sha256 });
            }
        }
    }
    files
}

fn extract_string(obj: &str, key: &str) -> Option<String> {
    let pat = format!("\"{key}\"");
    let i = obj.find(&pat)?;
    let rest = &obj[i + pat.len()..];
    let q1 = rest.find('"')?;
    let rest = &rest[q1 + 1..];
    let q2 = rest.find('"')?;
    Some(rest[..q2].to_string())
}

pub fn workspace_generated_dir(workspace_root: &Path) -> PathBuf {
    workspace_root.join(GENERATED_DIR)
}
