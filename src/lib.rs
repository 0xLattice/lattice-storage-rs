//! CID-verified file layout from 0xLattice/lattice-storage. This library does not
//! choose a chain, pin content, contact a node or promise availability.
#![forbid(unsafe_code)]
use base64::{
    Engine,
    engine::{
        DecodePaddingMode,
        general_purpose::{GeneralPurpose, GeneralPurposeConfig, STANDARD},
    },
};
use lattice_cashew::cbor::{self, CborValue as Value};
use lattice_sdk::volumes::{DagCborLimits, decode_dag_cbor};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
pub const CHUNK_BYTES: usize = 1 << 20;
pub const MAX_TYPE_LENGTH: usize = 255;
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileVolume {
    pub cid: String,
    pub entries: BTreeMap<String, String>,
}
#[derive(Debug, thiserror::Error)]
pub enum FileError {
    #[error("file type exceeds 255 UTF-16 code units")]
    TypeLength,
    #[error("chunks do not add up to the file")]
    ChunkLengths,
    #[error("file exceeds addressable memory")]
    Size,
    #[error(transparent)]
    Encoding(#[from] lattice_cashew::Error),
}
fn cid(bytes: &[u8]) -> String {
    let mut data = vec![1, 0x71, 0x12, 32];
    data.extend_from_slice(&Sha256::digest(bytes));
    format!(
        "b{}",
        data_encoding::BASE32_NOPAD
            .encode(&data)
            .to_ascii_lowercase()
    )
}
fn volume(value: &Value) -> Result<FileVolume, FileError> {
    let bytes = cbor::encode_value(value)?;
    let cid = cid(&bytes);
    Ok(FileVolume {
        cid: cid.clone(),
        entries: BTreeMap::from([(cid, STANDARD.encode(bytes))]),
    })
}
fn map(values: impl IntoIterator<Item = (&'static str, Value)>) -> Value {
    Value::Map(
        values
            .into_iter()
            .map(|(k, v)| (Value::Text(k.into()), v))
            .collect(),
    )
}
/// Root first, followed by chunks in file order. Repeated chunks intentionally
/// remain repeated, matching the reference layout's ordered volume sequence.
pub fn file_volumes(media_type: &str, bytes: &[u8]) -> Result<Vec<FileVolume>, FileError> {
    if media_type.encode_utf16().count() > MAX_TYPE_LENGTH {
        return Err(FileError::TypeLength);
    }
    let size = u64::try_from(bytes.len()).map_err(|_| FileError::Size)?;
    if bytes.len() <= CHUNK_BYTES {
        return Ok(vec![volume(&map([
            ("type", Value::Text(media_type.into())),
            ("size", Value::Integer(size.into())),
            ("bytes", Value::Bytes(bytes.to_vec())),
        ]))?]);
    }
    let chunks = bytes
        .chunks(CHUNK_BYTES)
        .map(|bytes| volume(&map([("bytes", Value::Bytes(bytes.to_vec()))])))
        .collect::<Result<Vec<_>, _>>()?;
    let root = volume(&map([
        ("type", Value::Text(media_type.into())),
        ("size", Value::Integer(size.into())),
        (
            "chunks",
            Value::Array(chunks.iter().map(|v| Value::Text(v.cid.clone())).collect()),
        ),
    ]))?;
    let mut result = Vec::with_capacity(chunks.len() + 1);
    result.push(root);
    result.extend(chunks);
    Ok(result)
}
pub fn is_dag_cbor_cid(value: &str) -> bool {
    let Some(value) = value.strip_prefix('b') else {
        return false;
    };
    if !value
        .bytes()
        .all(|b| b.is_ascii_lowercase() || (b'2'..=b'7').contains(&b))
    {
        return false;
    }
    let Ok(bytes) = data_encoding::BASE32_NOPAD.decode(value.to_ascii_uppercase().as_bytes())
    else {
        return false;
    };
    bytes.len() == 36 && bytes[..4] == [1, 0x71, 0x12, 32]
}
/// Reads only the root's entry. Other entries, including invalid ones, are
/// ignored as in the reference. Identity CIDs cannot authenticate file blocks.
pub fn read_block(volume: &FileVolume) -> Option<Value> {
    let encoded = volume.entries.get(&volume.cid)?;
    let text: Vec<u8> = encoded
        .bytes()
        .filter(|b| !matches!(b, b' ' | b'\t' | b'\n' | b'\r' | 0x0c))
        .collect();
    let engine = GeneralPurpose::new(
        &base64::alphabet::STANDARD,
        GeneralPurposeConfig::new()
            .with_decode_padding_mode(DecodePaddingMode::Indifferent)
            .with_decode_allow_trailing_bits(true),
    );
    let bytes = engine.decode(text).ok()?;
    if cid(&bytes) != volume.cid {
        return None;
    }
    let value = decode_dag_cbor(&bytes, DagCborLimits::default()).ok()?;
    // JavaScript's decoded object cannot retain this own key. The reference
    // re-encoding check therefore rejects it, including in nested objects.
    let mut pending = vec![&value];
    while let Some(value) = pending.pop() {
        match value {
            Value::Map(entries) => {
                for (key, value) in entries {
                    if matches!(key,Value::Text(s) if s=="__proto__") {
                        return None;
                    }
                    pending.push(value);
                }
            }
            Value::Array(values) => pending.extend(values),
            _ => {}
        }
    }
    Some(value)
}
#[derive(Debug, Clone)]
enum Layout {
    Inline(Vec<u8>),
    Chunks(Vec<String>),
}
/// A structurally checked file root. Constructed only by `read_root`.
#[derive(Debug, Clone)]
pub struct FileRoot {
    media_type: String,
    size: u64,
    layout: Layout,
}
impl FileRoot {
    pub fn media_type(&self) -> &str {
        &self.media_type
    }
    pub fn size(&self) -> u64 {
        self.size
    }
    pub fn bytes(&self) -> Option<&[u8]> {
        match &self.layout {
            Layout::Inline(bytes) => Some(bytes),
            _ => None,
        }
    }
    pub fn chunks(&self) -> Option<&[String]> {
        match &self.layout {
            Layout::Chunks(chunks) => Some(chunks),
            _ => None,
        }
    }
    pub fn chunk_length(&self, index: usize) -> Option<usize> {
        let chunks = self.chunks()?;
        if index >= chunks.len() {
            return None;
        }
        let offset = u64::try_from(index).ok()?.checked_mul(CHUNK_BYTES as u64)?;
        usize::try_from((self.size - offset).min(CHUNK_BYTES as u64)).ok()
    }
}
fn field<'a>(entries: &'a [(Value, Value)], name: &str) -> Option<&'a Value> {
    entries
        .iter()
        .find_map(|(k, v)| matches!(k,Value::Text(s)if s==name).then_some(v))
}
pub fn read_root(volume: &FileVolume, max_bytes: Option<u64>) -> Option<FileRoot> {
    let Value::Map(entries) = read_block(volume)? else {
        return None;
    };
    if entries.len() != 3 {
        return None;
    }
    let Value::Text(media_type) = field(&entries, "type")? else {
        return None;
    };
    if media_type.encode_utf16().count() > MAX_TYPE_LENGTH {
        return None;
    }
    let Value::Integer(size) = field(&entries, "size")? else {
        return None;
    };
    let size = u64::try_from(i128::from(*size)).ok()?;
    if max_bytes.is_some_and(|limit| size > limit) {
        return None;
    }
    let layout = if let Some(Value::Bytes(bytes)) = field(&entries, "bytes") {
        if size != bytes.len() as u64 || bytes.len() > CHUNK_BYTES {
            return None;
        }
        Layout::Inline(bytes.clone())
    } else {
        let Value::Array(chunks) = field(&entries, "chunks")? else {
            return None;
        };
        if size <= CHUNK_BYTES as u64 || chunks.len() as u64 != size.div_ceil(CHUNK_BYTES as u64) {
            return None;
        }
        Layout::Chunks(
            chunks
                .iter()
                .map(|v| match v {
                    Value::Text(cid) if is_dag_cbor_cid(cid) => Some(cid.clone()),
                    _ => None,
                })
                .collect::<Option<_>>()?,
        )
    };
    Some(FileRoot {
        media_type: media_type.clone(),
        size,
        layout,
    })
}
pub fn read_chunk(volume: &FileVolume, root: &FileRoot, index: usize) -> Option<Vec<u8>> {
    if root.chunks()?.get(index)? != &volume.cid {
        return None;
    }
    let Value::Map(entries) = read_block(volume)? else {
        return None;
    };
    if entries.len() != 1 {
        return None;
    }
    let Value::Bytes(bytes) = field(&entries, "bytes")? else {
        return None;
    };
    (bytes.len() == root.chunk_length(index)?).then(|| bytes.clone())
}
/// Assemble already verified chunks in order; callers use `read_chunk` to bind
/// each chunk to its committed CID. Length checks alone do not authenticate it.
pub fn assemble(root: &FileRoot, chunks: &[Vec<u8>]) -> Result<Vec<u8>, FileError> {
    if let Some(bytes) = root.bytes() {
        return Ok(bytes.to_vec());
    }
    if chunks.len() != root.chunks().expect("chunked").len()
        || chunks
            .iter()
            .enumerate()
            .any(|(i, b)| root.chunk_length(i) != Some(b.len()))
    {
        return Err(FileError::ChunkLengths);
    }
    let mut result = Vec::new();
    result
        .try_reserve_exact(usize::try_from(root.size).map_err(|_| FileError::Size)?)
        .map_err(|_| FileError::Size)?;
    for chunk in chunks {
        result.extend_from_slice(chunk);
    }
    Ok(result)
}
