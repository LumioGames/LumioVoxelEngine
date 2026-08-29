//! FIPS 180-4 known-answer tests for the two hand-written SHA-256 implementations.
//!
//! The two implementations are kept separate deliberately. `generated_clean` hashes the
//! generated tree to detect hand-edits, and the contract-runtime hasher *lives inside that
//! tree* — `rust/lumio-gen-contract-runtime/src/sha256.rs` is itself a locked entry. Hashing
//! the tree with a hasher taken from it would let a tampered generated file certify itself,
//! so the guard must keep its own copy. `implementations_agree_*` below is what stops the
//! two copies from drifting apart.
//!
//! Expected digests: the five vectors in `nist_*` and `million_a_*` are the published
//! FIPS 180-4 / NIST CSRC answers; the padding-boundary digests were produced by an
//! independent reference (`openssl dgst -sha256`), not by either implementation under test.

use lumio_voxel_test_support::generated_clean::sha256_hex as guard_sha256_hex;

fn contract_sha256_hex(data: &[u8]) -> String {
    lumio_voxel_contracts::sha256(data)
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}

/// Every vector is asserted against both implementations, naming which one failed.
fn assert_kat(label: &str, data: &[u8], expected: &str) {
    assert_eq!(
        guard_sha256_hex(data),
        expected,
        "generated_clean::sha256_hex wrong for KAT {label} (len {})",
        data.len()
    );
    assert_eq!(
        contract_sha256_hex(data),
        expected,
        "lumio_gen_contract_runtime::sha256 wrong for KAT {label} (len {})",
        data.len()
    );
}

#[test]
fn nist_empty_vector() {
    assert_kat(
        "empty",
        b"",
        "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
    );
}

#[test]
fn nist_abc_vector() {
    assert_kat(
        "abc",
        b"abc",
        "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad",
    );
}

/// 448 bits: last block that still holds the length field, no extra padding block.
#[test]
fn nist_448_bit_vector() {
    assert_kat(
        "448-bit",
        b"abcdbcdecdefdefgefghfghighijhijkijkljklmklmnlmnomnopnopq",
        "248d6a61d20638b8e5c026930c3e6039a33ce45964ff2167f6ecedd419db06c1",
    );
}

/// 896 bits: forces the padding to spill into an additional block.
#[test]
fn nist_896_bit_vector() {
    assert_kat(
        "896-bit",
        b"abcdefghbcdefghicdefghijdefghijkefghijklfghijklmghijklmnhijklmnoijklmnopjklmnopqklmnopqrlmnopqrsmnopqrstnopqrstu",
        "cf5b16a778af8380036ce59e7b0492370b249b11e8f07a51afac45037afee9d1",
    );
}

/// One million 'a' — 15625 blocks, the published multi-block vector.
#[test]
fn million_a_multi_block_vector() {
    assert_kat(
        "1e6 x 'a'",
        &vec![b'a'; 1_000_000],
        "cdc76e5c9914fb9281a1c7e284d73e67f1809a48a497200e046d39ccc7112cd0",
    );
}

/// Lengths where the 0x80 terminator and the 64-bit length field change block layout.
#[test]
fn padding_boundary_lengths() {
    const BOUNDARY: [(usize, &str); 10] = [
        (
            55,
            "9f4390f8d30c2dd92ec9f095b65e2b9ae9b0a925a5258e241c9f1e910f734318",
        ),
        (
            56,
            "b35439a4ac6f0948b6d6f9e3c6af0f5f590ce20f1bde7090ef7970686ec6738a",
        ),
        (
            57,
            "f13b2d724659eb3bf47f2dd6af1accc87b81f09f59f2b75e5c0bed6589dfe8c6",
        ),
        (
            63,
            "7d3e74a05d7db15bce4ad9ec0658ea98e3f06eeecf16b4c6fff2da457ddc2f34",
        ),
        (
            64,
            "ffe054fe7ae0cb6dc65c3af9b61d5209f439851db43d0ba5997337df154668eb",
        ),
        (
            65,
            "635361c48bb9eab14198e76ea8ab7f1a41685d6ad62aa9146d301d4f17eb0ae0",
        ),
        (
            119,
            "31eba51c313a5c08226adf18d4a359cfdfd8d2e816b13f4af952f7ea6584dcfb",
        ),
        (
            120,
            "2f3d335432c70b580af0e8e1b3674a7c020d683aa5f73aaaedfdc55af904c21c",
        ),
        (
            127,
            "c57e9278af78fa3cab38667bef4ce29d783787a2f731d4e12200270f0c32320a",
        ),
        (
            128,
            "6836cf13bac400e9105071cd6af47084dfacad4e5e302c94bfed24e013afb73e",
        ),
    ];

    for (len, expected) in BOUNDARY {
        assert_kat(&format!("{len} x 'a'"), &vec![b'a'; len], expected);
    }
}

/// Deterministic byte pattern; avoids a dependency just to get varied inputs.
fn lcg_bytes(len: usize, seed: u64) -> Vec<u8> {
    let mut state = seed;
    (0..len)
        .map(|_| {
            state = state
                .wrapping_mul(6_364_136_223_846_793_005)
                .wrapping_add(1_442_695_040_888_963_407);
            (state >> 33) as u8
        })
        .collect()
}

/// Differential guard: the two copies must stay byte-identical as either one is edited.
#[test]
fn implementations_agree_on_length_sweep() {
    for len in 0..=200usize {
        let data = lcg_bytes(len, len as u64 + 1);
        assert_eq!(
            guard_sha256_hex(&data),
            contract_sha256_hex(&data),
            "implementations disagree at length {len}"
        );
    }
    for len in [1_000usize, 4_096, 65_536] {
        let data = lcg_bytes(len, len as u64);
        assert_eq!(
            guard_sha256_hex(&data),
            contract_sha256_hex(&data),
            "implementations disagree at length {len}"
        );
    }
}
