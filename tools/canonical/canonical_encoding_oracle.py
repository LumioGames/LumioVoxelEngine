#!/usr/bin/env python3
"""Independent oracle for the Voxel canonical object encoding, old and new.

WHAT THIS IS FOR
----------------
Two jobs, both offline:

1. **Expected values for tests.** The `new_*` half implements the encoding from its
   written rules rather than from `crates/lumio-voxel-ops/src/canonical.rs`, so the
   digests it produces are an independent check on that module rather than a copy of
   it. The goldens pinned in `crates/lumio-voxel-ops/tests/` came from here; see
   `docs/evidence/canonical-encoding-goldens.md`.

2. **Retrospective recompute of pre-cut digests.** The `old_*` half is a faithful
   port of the vendored `canonical_object_pairs` plus the four copies of `quote()`
   that preceded it. A fingerprint recorded in a receipt written before the cut
   cannot be recomputed by current code; this reproduces it so an audit trail stays
   readable. Whether any such receipt exists was NOT verified — no persistence
   exists in this repository, and the Host side was not checked.

NOT A PRODUCTION PATH. No production code may import or shell out to this file. The
old encoding it reproduces is the defect: it concatenates unescaped names and
unquoted values and accepts a repeated name, so distinct requests can share a digest.
It is kept here to read history, never to write it.

Usage:
    python3 tools/canonical/canonical_encoding_oracle.py            # print goldens
    python3 tools/canonical/canonical_encoding_oracle.py --compare  # old vs new table
"""

import hashlib
import sys

CANONICAL_FORM_FIELD = "canonicalForm"
CANONICAL_FORM_ID = "VoxelCanonicalObjectV1"


# --------------------------------------------------------------------------
# old: the pre-cut encoding, reproduced for retrospective recompute only
# --------------------------------------------------------------------------

def old_encode(pairs):
    """Port of the vendored `canonical_object_pairs`: sort by name, concatenate."""
    ordered = sorted(pairs, key=lambda kv: kv[0].encode("utf-8"))
    return "{" + ",".join('"%s":%s' % (k, v) for k, v in ordered) + "}"


def old_quote(value):
    """Port of the four copies of `quote()`: add quotes, escape nothing."""
    return '"' + value + '"'


def old_fingerprint(txn_id, world_id, generation, fields):
    """Pre-cut `canonical_fingerprint`. Field values were pushed bare."""
    pairs = list(fields.items()) + [
        ("txn_id", old_quote(txn_id)),
        ("world_id", old_quote(world_id)),
        ("generation", str(generation)),
    ]
    return old_encode(pairs)


# --------------------------------------------------------------------------
# new: the current encoding, written from its rules
# --------------------------------------------------------------------------

def escape(value):
    out = []
    for ch in value:
        if ch == '"':
            out.append('\\"')
        elif ch == "\\":
            out.append("\\\\")
        elif ord(ch) < 0x20:
            out.append("\\u%04x" % ord(ch))
        else:
            out.append(ch)
    return "".join(out)


def render(value):
    kind, payload = value
    if kind == "text":
        return '"' + escape(payload) + '"'
    if kind == "uint":
        if payload < 0:
            raise ValueError("uint must be non-negative")
        return str(payload)
    if kind == "array":
        return "[" + ",".join('"' + escape(e) + '"' for e in payload) + "]"
    raise ValueError("unknown value kind %r" % (kind,))


def new_encode(members):
    names = [name for name, _ in members]
    if len(set(names)) != len(names):
        raise ValueError("duplicate member name")
    ordered = sorted(members, key=lambda kv: kv[0].encode("utf-8"))
    return "{" + ",".join('"%s":%s' % (escape(k), render(v)) for k, v in ordered) + "}"


def text(value):
    return ("text", value)


def uint(value):
    return ("uint", value)


def array(values):
    return ("array", list(values))


def new_fingerprint(txn_id, world_id, generation, fields):
    """Current `canonical_fingerprint`.

    Field values are strings by declared type, so they encode as strings: no name is
    special-cased into an integer. The form member makes the encoding self-naming, so
    a later format change shows up as a different form id rather than as digests that
    quietly stopped matching.
    """
    members = [(k, text(v)) for k, v in fields.items()] + [
        (CANONICAL_FORM_FIELD, text(CANONICAL_FORM_ID)),
        ("txn_id", text(txn_id)),
        ("world_id", text(world_id)),
        ("generation", uint(generation)),
    ]
    return new_encode(members)


def sha256_hex(canonical):
    return hashlib.sha256(canonical.encode("utf-8")).hexdigest()


# --------------------------------------------------------------------------
# vectors
# --------------------------------------------------------------------------

# (name, txn_id, world_id, generation, fields)
FINGERPRINT_VECTORS = [
    ("order_independent", "txn-1", "world-a", 1, {"k1": "1", "k2": "2"}),
    ("field_sensitive", "txn-1", "world-a", 1, {"k1": "1", "k2": "3"}),
    ("no_fields", "txn-1", "world-a", 1, {}),
    ("forged_absorb_one", "t", "world-a", 1, {"a": '1,"b":2'}),
    ("honest_two_fields", "t", "world-a", 1, {"a": "1", "b": "2"}),
    ("forged_absorb_two", "t", "world-a", 1, {"a": '1,"b":2,"c":3'}),
    ("honest_three_fields", "t", "world-a", 1, {"a": "1", "b": "2", "c": "3"}),
    ("forged_close_own_quotes", "t", "world-a", 1, {"a": '1","b":"2'}),
    ("forged_key_close_quotes", "t", "world-a", 1, {'a":"1","b': "2"}),
    ("forged_txn_id_append", 't","u":"9', "world-a", 1, {}),
    ("honest_extra_field", "t", "world-a", 1, {"u": '"9"'}),
    ("escape_boundary", 'q"\\z', "world-a", 1,
     {"nl": "a\nb", "bs": "c\\d", "dq": 'e"f'}),
    ("world_revision_only", "txn-1", "world-a", 1, {"world_revision": "0"}),
    ("one_chunk_edit", "txn-2", "world-a", 1,
     {"world_revision": "3", "c:0:0:0": "payload"}),
]

HEX = "5" * 64

# (name, old pairs, new members) for the surfaces that are not the fingerprint
SURFACE_VECTORS = [
    (
        "receipt bytes",
        [("txn_id", old_quote("txn-1")), ("old_root", old_quote(HEX)),
         ("new_root", old_quote(HEX)), ("fingerprint", old_quote(HEX))],
        [("txn_id", text("txn-1")), ("old_root", text(HEX)),
         ("new_root", text(HEX)), ("fingerprint", text(HEX))],
    ),
    (
        "snapshot manifest bytes",
        [("schemaId", old_quote("voxel-snapshot-payload")),
         ("headerSchemaId", old_quote("snapshot-header")),
         ("magic", old_quote("LUMIOSNP1")), ("schemaEpoch", "1"),
         ("worldId", old_quote("world-a")), ("contextId", old_quote("ctx-1")),
         ("generation", "1"), ("worldRevision", "7"),
         ("chunkRevision.c:0:0:0", "7"), ("configHash", old_quote(HEX)),
         ("rootIdentity", old_quote(HEX))],
        [("schemaId", text("voxel-snapshot-payload")),
         ("headerSchemaId", text("snapshot-header")),
         ("magic", text("LUMIOSNP1")), ("schemaEpoch", uint(1)),
         ("worldId", text("world-a")), ("contextId", text("ctx-1")),
         ("generation", uint(1)), ("worldRevision", uint(7)),
         ("chunkRevision.c:0:0:0", uint(7)), ("configHash", text(HEX)),
         ("rootIdentity", text(HEX))],
    ),
    (
        "restore shadow candidate hash",
        [("configHash", old_quote(HEX)), ("contextId", old_quote("ctx-1")),
         ("generation", "1"), ("replacement", old_quote(HEX)),
         ("rootIdentity", old_quote(HEX)), ("worldId", old_quote("world-a")),
         ("worldRevision", "7")],
        [("configHash", text(HEX)), ("contextId", text("ctx-1")),
         ("generation", uint(1)), ("replacement", text(HEX)),
         ("rootIdentity", text(HEX)), ("worldId", text("world-a")),
         ("worldRevision", uint(7))],
    ),
    (
        "query plan hash",
        [("schemaId", old_quote("voxel-query")), ("queryId", old_quote("q-1")),
         ("worldId", old_quote("world-a")), ("context", old_quote("ctx-1")),
         ("canonicalChunks", '["c:0:0:0","c:0:0:1"]'),
         ("stampWorldId", old_quote("world-a")),
         ("stampContextId", old_quote("ctx-1")), ("generation", "1"),
         ("worldRevision", "7"), ("chunkRevision.c:0:0:0", "7"),
         ("configHash", old_quote(HEX)), ("maxChunks", "64")],
        [("schemaId", text("voxel-query")), ("queryId", text("q-1")),
         ("worldId", text("world-a")), ("context", text("ctx-1")),
         ("canonicalChunks", array(["c:0:0:0", "c:0:0:1"])),
         ("stampWorldId", text("world-a")), ("stampContextId", text("ctx-1")),
         ("generation", uint(1)), ("worldRevision", uint(7)),
         ("chunkRevision.c:0:0:0", uint(7)), ("configHash", text(HEX)),
         ("maxChunks", uint(64))],
    ),
]


# The manifest that `two_encodes_of_same_ref_are_byte_identical_and_decode_back`
# captures, member for member. ADR 0011 says a snapshot written before the typed
# encoding still restores after it, and nothing in the repository held that claim to
# anything — the Rust test asserted only that encoding twice agrees and that decode
# inverts it, both of which stay true through any change to `ManifestAdapter::object`.
# Pinning the digest here is what turns the claim into a judgment.
#
# `configHash` and `rootIdentity` are fixture inputs, not results: they are sha256
# over domain structures that this oracle deliberately does not reimplement, so they
# are transcribed from the fixture the same way "world-a" is. What the oracle derives
# independently is the part under test — how that member set becomes bytes, and the
# digest of those bytes.
SNAPSHOT_MANIFEST_FIXTURE = [
    ("schemaId", text("voxel-snapshot-payload")),
    ("headerSchemaId", text("snapshot-header")),
    ("magic", text("LUMIOSNP1")),
    ("schemaEpoch", uint(1)),
    ("worldId", text("world-a")),
    ("contextId", text("ctx-1")),
    ("generation", uint(1)),
    ("worldRevision", uint(0)),
    ("configHash",
     text("aac0591628275ee9f9df6cadb2b9e21ec3b97021f6e0592b1f3883107e546cde")),
    ("rootIdentity",
     text("efd3b6f99cd27fdfe35404e4c9b8b8d5fd60eb44d1d7c44bbf84c8bc20658ba1")),
]


def print_goldens():
    print("# fingerprint goldens (current encoding)\n")
    for name, txn, world, gen, fields in FINGERPRINT_VECTORS:
        canonical = new_fingerprint(txn, world, gen, fields)
        print("%s" % name)
        print("  bytes  %s" % canonical)
        print("  sha256 %s\n" % sha256_hex(canonical))

    print("# snapshot manifest golden (r00134-canon capture)\n")
    canonical = new_encode(SNAPSHOT_MANIFEST_FIXTURE)
    print("snapshot_manifest")
    print("  bytes  %s" % canonical)
    print("  sha256 %s\n" % sha256_hex(canonical))


def print_comparison():
    print("%-32s %-8s %s" % ("surface", "verdict", "digest"))
    print("-" * 108)
    rows = []
    for name, txn, world, gen, fields in FINGERPRINT_VECTORS[:3] + FINGERPRINT_VECTORS[-2:]:
        rows.append(("fingerprint: " + name,
                     old_fingerprint(txn, world, gen, fields),
                     new_fingerprint(txn, world, gen, fields)))
    for name, old_pairs, new_members in SURFACE_VECTORS:
        rows.append((name, old_encode(old_pairs), new_encode(new_members)))
    for name, old_bytes, new_bytes in rows:
        old_hex, new_hex = sha256_hex(old_bytes), sha256_hex(new_bytes)
        verdict = "SAME" if old_hex == new_hex else "CHANGED"
        print("%-32s %-8s old %s" % (name, verdict, old_hex))
        print("%-32s %-8s new %s" % ("", "", new_hex))
        if verdict == "CHANGED":
            print("%-32s %-8s old bytes %s" % ("", "", old_bytes))
            print("%-32s %-8s new bytes %s" % ("", "", new_bytes))
        print()


if __name__ == "__main__":
    if "--compare" in sys.argv:
        print_comparison()
    else:
        print_goldens()
