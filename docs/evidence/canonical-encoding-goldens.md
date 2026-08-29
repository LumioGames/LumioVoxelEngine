# Canonical encoding — goldens and the pre-cut digest comparison

Evidence for the `canonical_object_pairs` fix adjudicated 2026-08-29
(`LumioGameEngineArchitecture/docs/plans/2026-08-29-canonical-object-pairs-adjudication.md`,
§3.4 and §5). Regenerate everything below with:

```
python3 tools/canonical/canonical_encoding_oracle.py            # goldens
python3 tools/canonical/canonical_encoding_oracle.py --compare  # old vs new
```

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

## What is not established here

Whether any receipt written before the cut was ever persisted. This repository has no
persistence at all — the only `fs::write` / `File::create` / `write_all` calls in the
workspace are in two test files, and the receipt ledger is an in-process map — so the
question cannot be answered from here, and the Host side was not checked. The `old_*`
half of the oracle exists so that such a receipt could still be recomputed if one
turns up.
