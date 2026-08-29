//! Voxel-local canonical object encoding: one encoder, one decoder, one escape rule.
//!
//! This is deliberately **not** an implementation of the published `CanonicalJsonV1`
//! form and must not be described as one. That form's member-name grammar
//! (`^[A-Za-z][A-Za-z0-9]*$`) excludes every name used here — `txn_id`, `c:0:0:0`,
//! `chunkRevision.c:0:0:0`. What this module guarantees is narrower and local:
//! distinct member sets encode to distinct bytes.
//!
//! Injectivity does not rest on callers passing well-formed pieces. A caller hands
//! in a typed [`CanonicalValue`], never pre-encoded bytes, so the encoder — not the
//! caller — decides how each value is delimited. Every string is quoted and escaped,
//! every integer is bare and separator-free, so `"`, `,`, `:`, `{`, `}`, `[` and `]`
//! only ever occur in structural positions. A repeated member name is rejected
//! rather than emitted twice.

#![forbid(unsafe_code)]

use std::collections::BTreeMap;
use std::collections::btree_map::Entry;

/// One canonical member value. The encoder renders each variant; there is no
/// variant that carries pre-encoded bytes.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum CanonicalValue {
    /// A string. Quoted and escaped on encode.
    Text(String),
    /// A non-negative integer. Rendered bare, shortest form.
    Uint(u64),
    /// An ordered list of strings. Element order is significant.
    TextArray(Vec<String>),
}

impl CanonicalValue {
    pub fn text(value: impl Into<String>) -> Self {
        Self::Text(value.into())
    }

    /// The string this value holds, or `None` for the other variants.
    pub fn as_text(&self) -> Option<&str> {
        match self {
            Self::Text(value) => Some(value),
            _ => None,
        }
    }

    /// The integer this value holds, or `None` for the other variants.
    pub fn as_uint(&self) -> Option<u64> {
        match self {
            Self::Uint(value) => Some(*value),
            _ => None,
        }
    }
}

/// A member name offered twice. A canonical object has at most one member per name,
/// and the encoder refuses rather than picking a winner.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DuplicateMember {
    key: String,
}

impl DuplicateMember {
    pub fn key(&self) -> &str {
        &self.key
    }
}

impl std::fmt::Display for DuplicateMember {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "duplicate canonical member {:?}", self.key)
    }
}

impl std::error::Error for DuplicateMember {}

/// Why some bytes are not a canonical object.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DecodeError {
    /// Not parseable as a canonical object at all.
    Malformed,
    /// A member name occurred twice.
    DuplicateMember,
    /// Parseable, but not the canonical spelling of what it parses to —
    /// members out of order, or a value written in a non-minimal escape form.
    NotCanonical,
}

impl std::fmt::Display for DecodeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let text = match self {
            Self::Malformed => "malformed canonical object",
            Self::DuplicateMember => "duplicate canonical member",
            Self::NotCanonical => "object is not in canonical form",
        };
        f.write_str(text)
    }
}

impl std::error::Error for DecodeError {}

/// A canonical object under construction or freshly decoded.
///
/// Members are held in a map keyed by name, so two members can never share a name.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct CanonicalObject {
    members: BTreeMap<String, CanonicalValue>,
}

impl CanonicalObject {
    pub fn new() -> Self {
        Self::default()
    }

    /// Add one member. Rejects a name that is already present instead of
    /// overwriting it, so a caller cannot silently shadow an earlier member.
    pub fn insert(
        &mut self,
        key: impl Into<String>,
        value: CanonicalValue,
    ) -> Result<(), DuplicateMember> {
        let key = key.into();
        match self.members.entry(key) {
            Entry::Occupied(entry) => Err(DuplicateMember {
                key: entry.key().clone(),
            }),
            Entry::Vacant(entry) => {
                entry.insert(value);
                Ok(())
            }
        }
    }

    /// Add a string member.
    pub fn insert_text(
        &mut self,
        key: impl Into<String>,
        value: impl Into<String>,
    ) -> Result<(), DuplicateMember> {
        self.insert(key, CanonicalValue::Text(value.into()))
    }

    /// Add an integer member.
    pub fn insert_uint(
        &mut self,
        key: impl Into<String>,
        value: u64,
    ) -> Result<(), DuplicateMember> {
        self.insert(key, CanonicalValue::Uint(value))
    }

    pub fn len(&self) -> usize {
        self.members.len()
    }

    pub fn is_empty(&self) -> bool {
        self.members.is_empty()
    }

    pub fn get(&self, key: &str) -> Option<&CanonicalValue> {
        self.members.get(key)
    }

    pub fn contains_key(&self, key: &str) -> bool {
        self.members.contains_key(key)
    }

    /// Members in canonical order: ascending by name, compared by code point.
    /// `BTreeMap<String, _>` orders by UTF-8 bytes, which for UTF-8 is code point order.
    pub fn members(&self) -> impl Iterator<Item = (&str, &CanonicalValue)> {
        self.members.iter().map(|(k, v)| (k.as_str(), v))
    }

    /// Canonical bytes for this object.
    pub fn encode(&self) -> String {
        let mut out = String::from("{");
        for (i, (key, value)) in self.members.iter().enumerate() {
            if i > 0 {
                out.push(',');
            }
            push_quoted(&mut out, key);
            out.push(':');
            push_value(&mut out, value);
        }
        out.push('}');
        out
    }

    /// Canonical bytes for this object.
    pub fn encode_bytes(&self) -> Vec<u8> {
        self.encode().into_bytes()
    }
}

fn push_value(out: &mut String, value: &CanonicalValue) {
    match value {
        CanonicalValue::Text(text) => push_quoted(out, text),
        // `u64::to_string` is decimal, unsigned, shortest form: no sign, no leading
        // zero, no separator, so it cannot contribute a structural byte.
        CanonicalValue::Uint(n) => out.push_str(&n.to_string()),
        CanonicalValue::TextArray(items) => {
            out.push('[');
            for (i, item) in items.iter().enumerate() {
                if i > 0 {
                    out.push(',');
                }
                push_quoted(out, item);
            }
            out.push(']');
        }
    }
}

/// Quote and escape one string. `"` and `\` are escaped because they would
/// otherwise close or corrupt the string; C0 controls are escaped so canonical
/// bytes stay a single printable line. Nothing else needs escaping for
/// injectivity, and escaping nothing else keeps the minimal form unique.
fn push_quoted(out: &mut String, raw: &str) {
    out.push('"');
    for ch in raw.chars() {
        match ch {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            c if (c as u32) < 0x20 => {
                out.push_str(&format!("\\u{:04x}", c as u32));
            }
            c => out.push(c),
        }
    }
    out.push('"');
}

/// Parse canonical object bytes, then require that they are the canonical
/// spelling of what was parsed.
///
/// The parser deliberately accepts more than [`CanonicalObject::encode`] emits —
/// any member order, and any `\uXXXX` escape, not only the minimal ones. The
/// re-encode comparison below is what rejects the surplus, so it is load-bearing
/// rather than true by construction.
pub fn decode(bytes: &[u8]) -> Result<CanonicalObject, DecodeError> {
    let object = parse(bytes)?;
    if object.encode().as_bytes() != bytes {
        return Err(DecodeError::NotCanonical);
    }
    Ok(object)
}

struct Parser<'a> {
    text: &'a str,
    at: usize,
}

fn parse(bytes: &[u8]) -> Result<CanonicalObject, DecodeError> {
    // UTF-8 is checked once for the whole input rather than once per character.
    // This rejects exactly what the per-character check rejected: anything `decode`
    // accepts is compared byte-for-byte against a re-encoded Rust `String`, so
    // accepted bytes were always valid UTF-8, and both spellings of the refusal are
    // `Malformed`. What changes is only the cost — validating the *remaining* buffer
    // at every character made a string quadratic in the bytes that follow it, and a
    // restore candidate is a file the Host read back, so its length is not ours to
    // assume small.
    let text = std::str::from_utf8(bytes).map_err(|_| DecodeError::Malformed)?;
    let mut parser = Parser { text, at: 0 };
    let object = parser.object()?;
    if parser.at != bytes.len() {
        return Err(DecodeError::Malformed);
    }
    Ok(object)
}

impl Parser<'_> {
    fn bytes(&self) -> &[u8] {
        self.text.as_bytes()
    }

    fn peek(&self) -> Option<u8> {
        self.bytes().get(self.at).copied()
    }

    fn eat(&mut self, byte: u8) -> Result<(), DecodeError> {
        if self.peek() == Some(byte) {
            self.at += 1;
            return Ok(());
        }
        Err(DecodeError::Malformed)
    }

    fn object(&mut self) -> Result<CanonicalObject, DecodeError> {
        self.eat(b'{')?;
        let mut object = CanonicalObject::new();
        if self.peek() == Some(b'}') {
            self.at += 1;
            return Ok(object);
        }
        loop {
            let key = self.string()?;
            self.eat(b':')?;
            let value = self.value()?;
            object
                .insert(key, value)
                .map_err(|_| DecodeError::DuplicateMember)?;
            match self.peek() {
                Some(b',') => self.at += 1,
                Some(b'}') => {
                    self.at += 1;
                    return Ok(object);
                }
                _ => return Err(DecodeError::Malformed),
            }
        }
    }

    fn value(&mut self) -> Result<CanonicalValue, DecodeError> {
        match self.peek() {
            Some(b'"') => Ok(CanonicalValue::Text(self.string()?)),
            Some(b'[') => self.array(),
            Some(b'0'..=b'9') => Ok(CanonicalValue::Uint(self.uint()?)),
            _ => Err(DecodeError::Malformed),
        }
    }

    fn array(&mut self) -> Result<CanonicalValue, DecodeError> {
        self.eat(b'[')?;
        let mut items = Vec::new();
        if self.peek() == Some(b']') {
            self.at += 1;
            return Ok(CanonicalValue::TextArray(items));
        }
        loop {
            items.push(self.string()?);
            match self.peek() {
                Some(b',') => self.at += 1,
                Some(b']') => {
                    self.at += 1;
                    return Ok(CanonicalValue::TextArray(items));
                }
                _ => return Err(DecodeError::Malformed),
            }
        }
    }

    fn uint(&mut self) -> Result<u64, DecodeError> {
        let start = self.at;
        while matches!(self.peek(), Some(b'0'..=b'9')) {
            self.at += 1;
        }
        let digits = &self.bytes()[start..self.at];
        if digits.is_empty() || (digits.len() > 1 && digits[0] == b'0') {
            return Err(DecodeError::Malformed);
        }
        std::str::from_utf8(digits)
            .map_err(|_| DecodeError::Malformed)?
            .parse()
            .map_err(|_| DecodeError::Malformed)
    }

    fn string(&mut self) -> Result<String, DecodeError> {
        self.eat(b'"')?;
        let mut out = String::new();
        loop {
            let byte = self.peek().ok_or(DecodeError::Malformed)?;
            match byte {
                b'"' => {
                    self.at += 1;
                    return Ok(out);
                }
                b'\\' => {
                    self.at += 1;
                    out.push(self.escape()?);
                }
                // A raw control byte never appears in encoder output.
                0x00..=0x1f => return Err(DecodeError::Malformed),
                _ => {
                    // `text` is already valid UTF-8 and every branch advances by whole
                    // characters, so slicing here is a boundary check, not a scan.
                    // `str::get` rather than `[..]` so a boundary that somehow moved
                    // is a refusal instead of a panic.
                    let ch = self
                        .text
                        .get(self.at..)
                        .and_then(|rest| rest.chars().next())
                        .ok_or(DecodeError::Malformed)?;
                    self.at += ch.len_utf8();
                    out.push(ch);
                }
            }
        }
    }

    fn escape(&mut self) -> Result<char, DecodeError> {
        match self.peek().ok_or(DecodeError::Malformed)? {
            b'"' => {
                self.at += 1;
                Ok('"')
            }
            b'\\' => {
                self.at += 1;
                Ok('\\')
            }
            b'u' => {
                self.at += 1;
                let hex = self
                    .bytes()
                    .get(self.at..self.at + 4)
                    .ok_or(DecodeError::Malformed)?;
                let hex = std::str::from_utf8(hex).map_err(|_| DecodeError::Malformed)?;
                if !hex.bytes().all(|b| b.is_ascii_hexdigit()) {
                    return Err(DecodeError::Malformed);
                }
                let code = u32::from_str_radix(hex, 16).map_err(|_| DecodeError::Malformed)?;
                self.at += 4;
                // A surrogate half is not a scalar value. C# strings are UTF-16 and
                // can hold one; refusing it here is the symmetric half of refusing to
                // encode one, and keeps a lone surrogate from folding onto U+FFFD.
                char::from_u32(code).ok_or(DecodeError::Malformed)
            }
            // No other escape is emitted, so accepting one would admit a second
            // spelling for a byte string that already has one.
            _ => Err(DecodeError::Malformed),
        }
    }
}
