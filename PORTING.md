# Port status

Reference: 0xLattice/lattice-storage `fc24f13fa4380aa32beb991749112429c682cf23`, specifically `client/file.mjs` and its own vendored SDK. No application chain is ported. The upstream checkout contains no license file; no license grant is invented here and Cargo publishing is disabled.

Implemented: root-first file volumes, exact canonical block bytes/CIDs, duplicate chunk preservation, UTF-16 media-type length, SHA-256 DAG-CBOR CID restriction, own-entry-only verification, JavaScript-compatible base64 whitespace/padding, canonical CBOR refusal including hidden `__proto__` fields, root budgets and shape/size checks, ordered chunk authentication and assembly.

`tools/generate-vectors.mjs ../lattice-storage` checks the source commit and tracked cleanliness, then imports the unchanged upstream code. The checked-in fixture records source SHA-256 hashes and 26 layouts (empty through multi-chunk, ASCII and UTF-16 boundary types), 34 valid/malformed reads, and 6 CID cases. Rust checks exact CIDs/block hashes, authenticated round trips, read results, and refusal boundaries. This is finite differential evidence, not a formal proof. The upstream Node test suite also runs independently.

API/resource differences: Rust types exclude non-string entry values and invalid Unicode scalar strings. File sizes use u64 and allocation is fallible at assembly. Out-of-range chunk indexes return None instead of exposing the JavaScript helper's arithmetic for invalid indexes. Decoding uses the SDK port's explicit default depth/item/byte limits; encoding uses Cashew's resource limits. These limits may reject extremely large/deep inputs that JavaScript attempts to process. The API currently holds files/volumes in memory, as the reference does. Streaming, HTTP pins/fetch integration, shared manifest/release CI and distribution remain separate work.

Lattice-formal has no file-layout theorem currently bridged to this crate. Hash verification and layout tests must not be presented as a proof of availability or consensus correctness.
