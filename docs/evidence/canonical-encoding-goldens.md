# Canonical encoding — goldens and the pre-cut digest comparison

Evidence for the `canonical_object_pairs` fix adjudicated 2026-08-29
(`LumioGameEngineArchitecture/docs/plans/2026-08-29-canonical-object-pairs-adjudication.md`,
§3.4 and §5). Regenerate the digests below with:

```
python3 tools/canonical/canonical_encoding_oracle.py            # goldens
python3 tools/canonical/canonical_encoding_oracle.py --compare  # old vs new
```

The decode-cost table has its own command, given in that section.

## Why the goldens live outside the crate

The expected digests pinned in `crates/lumio-voxel-ops/tests/canonical_injection.rs`
and `crates/lumio-voxel-ops/tests/mutation_receipt.rs` are produced by
`tools/canonical/canonical_encoding_oracle.py`, which implements the encoding from
its written rules rather than by calling the Rust code. That independence is the
point: `mutation_receipt.rs` previously computed its own expected value by
re-implementing the encoder inline, so the assertion held whether or not values were
escaped — it could not fail. An expected value the implementation cannot produce for
itself is what turns that assertion back into a judgment.

## The break, surface by surface

`fields` values were pushed bare into the old encoding, so the mutation fingerprint
necessarily changes. The other four surfaces already sorted their values into strings
and integers by hand, so adding escaping and types is an identity on well-formed data
— their digests are unchanged. What does change them all is the form member, which is
present only in the fingerprint.

The consequence worth stating plainly: **snapshot manifest bytes are unchanged, so a
snapshot written before this change still restores after it.**

```
surface                          verdict  digest
------------------------------------------------------------------------------------------------------------
fingerprint: order_independent   CHANGED  old e8994a3841a91d53ced1a3d2a4cb0338e17fda412052ff21e920da45c79fbde2
                                          new 5fb39c21495127d88c5e884668b9e9585a3dca72a7ee2ead9a2df4592333f075
                                          old bytes {"generation":1,"k1":1,"k2":2,"txn_id":"txn-1","world_id":"world-a"}
                                          new bytes {"canonicalForm":"VoxelCanonicalObjectV1","generation":1,"k1":"1","k2":"2","txn_id":"txn-1","world_id":"world-a"}

fingerprint: field_sensitive     CHANGED  old 30ab697796f64816bdd645fcedea4838fd22aab692ee51048b7bb3afc5e8d07a
                                          new bb20b986f36de85f4109b7fe854d876da6d113665211beeeba2bdb81bedc5e6b
                                          old bytes {"generation":1,"k1":1,"k2":3,"txn_id":"txn-1","world_id":"world-a"}
                                          new bytes {"canonicalForm":"VoxelCanonicalObjectV1","generation":1,"k1":"1","k2":"3","txn_id":"txn-1","world_id":"world-a"}

fingerprint: no_fields           CHANGED  old 8e3d1077569756bed1e40c8dad5db2b10339a9757e888519dafcab51b838f3a7
                                          new 5698db6879ec4f6c542f6e76ca0e22153903f074f0c33292bc6462175aa74743
                                          old bytes {"generation":1,"txn_id":"txn-1","world_id":"world-a"}
                                          new bytes {"canonicalForm":"VoxelCanonicalObjectV1","generation":1,"txn_id":"txn-1","world_id":"world-a"}

fingerprint: world_revision_only CHANGED  old 19efd04c5cb5efa5f0cebaccc4a75829491e17e7061a74ba728716265804be0e
                                          new 5d3ad31a4cc2bcb575f39caed7aa8e7c7726ce67577c9a2d7c5acff674203705
                                          old bytes {"generation":1,"txn_id":"txn-1","world_id":"world-a","world_revision":0}
                                          new bytes {"canonicalForm":"VoxelCanonicalObjectV1","generation":1,"txn_id":"txn-1","world_id":"world-a","world_revision":"0"}

fingerprint: one_chunk_edit      CHANGED  old 544afe06ad72eb61354eedc05e97068566a08552bf40f0cf2d3cdd7cd98446c4
                                          new e372e9671f7d3e3c9540f8a2dc56987122df9a3afea7d9343e5110bbb69d7087
                                          old bytes {"c:0:0:0":payload,"generation":1,"txn_id":"txn-2","world_id":"world-a","world_revision":3}
                                          new bytes {"c:0:0:0":"payload","canonicalForm":"VoxelCanonicalObjectV1","generation":1,"txn_id":"txn-2","world_id":"world-a","world_revision":"3"}

receipt bytes                    SAME     old d19842d5d0d76f632bef94b5401d51ef5559fa2fbe38a9ae76bed86323643411
                                          new d19842d5d0d76f632bef94b5401d51ef5559fa2fbe38a9ae76bed86323643411

snapshot manifest bytes          SAME     old b8648199e324d283c74b312778e4dd2912245b28a7aeeb7606a7dad0ed048904
                                          new b8648199e324d283c74b312778e4dd2912245b28a7aeeb7606a7dad0ed048904

restore shadow candidate hash    SAME     old 6305a35094be47e6a81c474a85b508b0c19feda6758cde168b70acd503b6a301
                                          new 6305a35094be47e6a81c474a85b508b0c19feda6758cde168b70acd503b6a301

query plan hash                  SAME     old 43b8de44cac8470c2fdfc57ca34dee06b0ea7ed968e8cb4f40a5c5b72be90fd2
                                          new 43b8de44cac8470c2fdfc57ca34dee06b0ea7ed968e8cb4f40a5c5b72be90fd2
```

`fields` values are encoded as strings, not integers, even when a name like
`world_revision` holds digits. Deciding a value's type from its name would reinstate
exactly the "the caller passed the right thing" assumption this encoding removes; the
cost is that the first two rows above change, which is intended.

## Fingerprint goldens (current encoding)

```
# fingerprint goldens (current encoding)

order_independent
  bytes  {"canonicalForm":"VoxelCanonicalObjectV1","generation":1,"k1":"1","k2":"2","txn_id":"txn-1","world_id":"world-a"}
  sha256 5fb39c21495127d88c5e884668b9e9585a3dca72a7ee2ead9a2df4592333f075

field_sensitive
  bytes  {"canonicalForm":"VoxelCanonicalObjectV1","generation":1,"k1":"1","k2":"3","txn_id":"txn-1","world_id":"world-a"}
  sha256 bb20b986f36de85f4109b7fe854d876da6d113665211beeeba2bdb81bedc5e6b

no_fields
  bytes  {"canonicalForm":"VoxelCanonicalObjectV1","generation":1,"txn_id":"txn-1","world_id":"world-a"}
  sha256 5698db6879ec4f6c542f6e76ca0e22153903f074f0c33292bc6462175aa74743

forged_absorb_one
  bytes  {"a":"1,\"b\":2","canonicalForm":"VoxelCanonicalObjectV1","generation":1,"txn_id":"t","world_id":"world-a"}
  sha256 e85d6d8b9a46381b10225c217678d8f19aac4f6251e881eeccf791097bfcb52a

honest_two_fields
  bytes  {"a":"1","b":"2","canonicalForm":"VoxelCanonicalObjectV1","generation":1,"txn_id":"t","world_id":"world-a"}
  sha256 d1d5607df6b923ab7c53b70ffca2ac7e5d7a0472df7db7f39213c4cc1178dc85

forged_absorb_two
  bytes  {"a":"1,\"b\":2,\"c\":3","canonicalForm":"VoxelCanonicalObjectV1","generation":1,"txn_id":"t","world_id":"world-a"}
  sha256 d88dee1c6a4f232f3fab88b4f409191f1685ee0f147f71acd4afa9f05f41ef98

honest_three_fields
  bytes  {"a":"1","b":"2","c":"3","canonicalForm":"VoxelCanonicalObjectV1","generation":1,"txn_id":"t","world_id":"world-a"}
  sha256 eb9a2c4b8e94804c560c07f27a345ebc9f6b09287510e758efe42aa5bafa4f82

forged_close_own_quotes
  bytes  {"a":"1\",\"b\":\"2","canonicalForm":"VoxelCanonicalObjectV1","generation":1,"txn_id":"t","world_id":"world-a"}
  sha256 2d61bd6d59add7a292abf80f9e2df50197479129c3a7acf4204887c1f2689e3c

forged_key_close_quotes
  bytes  {"a\":\"1\",\"b":"2","canonicalForm":"VoxelCanonicalObjectV1","generation":1,"txn_id":"t","world_id":"world-a"}
  sha256 2fd1acdb9bfd53eecae462e386c7ffa6ea6b74b82a3edd542d190a356c408912

forged_txn_id_append
  bytes  {"canonicalForm":"VoxelCanonicalObjectV1","generation":1,"txn_id":"t\",\"u\":\"9","world_id":"world-a"}
  sha256 43e486625335b5d174229284fcf35b33eb97a6fd8cb46c03a450036e1e4ef4fc

honest_extra_field
  bytes  {"canonicalForm":"VoxelCanonicalObjectV1","generation":1,"txn_id":"t","u":"\"9\"","world_id":"world-a"}
  sha256 285c8ffee661bf7317e05287207b1ddef089f590b6fef8e57cf54254c1167b0c

escape_boundary
  bytes  {"bs":"c\\d","canonicalForm":"VoxelCanonicalObjectV1","dq":"e\"f","generation":1,"nl":"a\u000ab","txn_id":"q\"\\z","world_id":"world-a"}
  sha256 a36ba67b02000ea7ebe8d23d5babd60ca72770402f331bf58609c833ef524993

world_revision_only
  bytes  {"canonicalForm":"VoxelCanonicalObjectV1","generation":1,"txn_id":"txn-1","world_id":"world-a","world_revision":"0"}
  sha256 5d3ad31a4cc2bcb575f39caed7aa8e7c7726ce67577c9a2d7c5acff674203705

one_chunk_edit
  bytes  {"c:0:0:0":"payload","canonicalForm":"VoxelCanonicalObjectV1","generation":1,"txn_id":"txn-2","world_id":"world-a","world_revision":"3"}
  sha256 e372e9671f7d3e3c9540f8a2dc56987122df9a3afea7d9343e5110bbb69d7087
```

## Snapshot manifest golden

The consequence stated above — a snapshot written before the change still restores
after it — had nothing holding it. `two_encodes_of_same_ref_are_byte_identical_and_decode_back`
asserted that encoding twice agrees, that decode inverts encode, and two `contains`
substrings; all four are self-referential and stay green through any change to
`ManifestAdapter::object`. The digest below is now asserted there, so a change to
the manifest member set fails the build instead of silently re-dating every archive.

`configHash` and `rootIdentity` are fixture inputs transcribed from the capture, not
values this oracle derives: they are sha256 over domain structures the oracle
deliberately does not reimplement. What the oracle derives independently is the part
under test — how that member set becomes bytes, and the digest of those bytes.

```
snapshot_manifest
  bytes  {"configHash":"aac0591628275ee9f9df6cadb2b9e21ec3b97021f6e0592b1f3883107e546cde","contextId":"ctx-1","generation":1,"headerSchemaId":"snapshot-header","magic":"LUMIOSNP1","rootIdentity":"efd3b6f99cd27fdfe35404e4c9b8b8d5fd60eb44d1d7c44bbf84c8bc20658ba1","schemaEpoch":1,"schemaId":"voxel-snapshot-payload","worldId":"world-a","worldRevision":0}
  sha256 b513120c559bd74211a4ed775914f666c2e65a4b21579426c690939be136880f
```

## Decode cost against manifest size

`RestorePreflight::validate` takes bytes the Host read back off the filesystem
(VOX-D-008 puts DAG orchestration, fsync and the Active-pointer swap on the Host), so
the length is chosen by whatever wrote the file. The first typed decoder validated the
*remaining* buffer once per character, making a string quadratic in the bytes after
it. Reproduce with:

```
cargo run --release -p lumio-voxel-test-support --example bench-canonical-decode
```

Manifest shape, N members of `chunkRevision.c:<i>:0:0`, release build, best of three,
same machine and same command for all three columns. The baseline column is the
pre-ADR-0011 `decode_canonical_pairs`, measured from a standalone port of the code at
`0a62388^` — it is the linear reference, not a target: the typed decoder is legitimately
~2.5× dearer because it builds typed values, unescapes, and re-encodes to check
canonicity.

```
chunks    bytes    baseline (0a62388^)   quadratic (6f93701)   linear (this change)
  1,000   28,233               185 µs              6,057 µs                 360 µs
  4,000  115,233               687 µs             89,991 µs               1,617 µs
 16,000  469,233             2,941 µs          1,563,167 µs               7,394 µs

µs per KB   (flat = linear, rising = quadratic)
  1,000                          6.7                 219.7                  13.1
  4,000                          6.1                 799.7                  14.4
 16,000                          6.4               3,411.3                  16.1
```

A 16k-chunk manifest went from 1.56 s to 7.4 ms. No byte-length ceiling was added to
`validate`; the reasoning is in
[ADR 0012](../../.spec/decisions/0012-canonical-decode-cost-and-refusal-naming.md).

## Residual gap: the form member covers one surface, not five

`canonicalForm = VoxelCanonicalObjectV1` is present only in the fingerprint. The
snapshot manifest, receipt, restore shadow and query plan carry no form member, so
those four surfaces' bytes do not identify their own encoding — a future format
change would show up there as digests that quietly stopped matching rather than as a
different form id.

This is the price of the row above: those four surfaces are byte-identical across the
cut *because* nothing was added to them. Naming it here rather than leaving it inside
the trade-off, because a reader checking "is the encoding self-identifying?" gets
different answers depending on which surface they look at, and should not have to
infer that from a table. Adjudicated and held; closing it means accepting a digest
break on all four.

## What is not established here

Whether any receipt written before the cut was ever persisted. This repository has no
persistence at all — the only `fs::write` / `File::create` / `write_all` calls in the
workspace are in two test files, and the receipt ledger is an in-process map — so the
question cannot be answered from here, and the Host side was not checked. The `old_*`
half of the oracle exists so that such a receipt could still be recomputed if one
turns up.
