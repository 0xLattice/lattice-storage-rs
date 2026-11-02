use base64::{Engine, engine::general_purpose::STANDARD};
use lattice_storage::*;
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
fn fixtures() -> Value {
    serde_json::from_str(include_str!("fixtures/javascript.json")).unwrap()
}
fn hash(bytes: &[u8]) -> String {
    hex::encode(Sha256::digest(bytes))
}
#[test]
fn exact_javascript_layouts_and_authenticated_round_trips() {
    for case in fixtures()["layouts"].as_array().unwrap() {
        let n = case["length"].as_u64().unwrap() as usize;
        let bytes: Vec<u8> = (0..n).map(|i| (i.wrapping_mul(31) + 7) as u8).collect();
        let volumes = file_volumes(case["type"].as_str().unwrap(), &bytes).unwrap();
        assert_eq!(
            json!(volumes.iter().map(|v| &v.cid).collect::<Vec<_>>()),
            case["cids"]
        );
        assert_eq!(
            json!(
                volumes
                    .iter()
                    .map(|v| hash(&STANDARD.decode(&v.entries[&v.cid]).unwrap()))
                    .collect::<Vec<_>>()
            ),
            case["blockHashes"]
        );
        let root = read_root(&volumes[0], None).unwrap();
        let chunks: Vec<_> = volumes
            .iter()
            .skip(1)
            .enumerate()
            .map(|(i, v)| read_chunk(v, &root, i).unwrap())
            .collect();
        assert_eq!(
            hash(&assemble(&root, &chunks).unwrap()),
            case["assembledHash"]
        );
        assert_eq!(root.size(), n as u64);
        if !chunks.is_empty() {
            assert!(assemble(&root, &chunks[..chunks.len() - 1]).is_err());
            assert!(read_chunk(&volumes[1], &root, chunks.len()).is_none());
        }
    }
    assert!(file_volumes(&"😀".repeat(128), &[]).is_err());
}
#[test]
fn javascript_read_and_cid_refusal_cases() {
    let data = fixtures();
    for case in data["roots"].as_array().unwrap() {
        let volume: FileVolume = serde_json::from_value(case["volume"].clone()).unwrap();
        assert_eq!(
            read_block(&volume).is_some(),
            case["block"].as_bool().unwrap(),
            "block {}",
            case["name"]
        );
        let root = read_root(&volume, case["maxBytes"].as_u64());
        let actual=root.map(|r|json!({"type":r.media_type(),"size":r.size().to_string(),"bytes":r.bytes().map(hex::encode),"chunks":r.chunks()}));
        assert_eq!(json!(actual), case["root"], "root {}", case["name"]);
    }
    for case in data["cidCases"].as_array().unwrap() {
        assert_eq!(
            is_dag_cbor_cid(case["cid"].as_str().unwrap()),
            case["valid"].as_bool().unwrap()
        );
    }
}
