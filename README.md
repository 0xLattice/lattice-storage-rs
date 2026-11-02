# lattice-storage (Rust)

Rust port of the reusable file layout in [0xLattice/lattice-storage](https://github.com/0xLattice/lattice-storage). Files up to 1 MiB have one DAG-CBOR volume; larger files have a root followed by ordered 1 MiB chunk volumes. SHA-256 CIDs authenticate each block.

```rust
use lattice_storage::{file_volumes, read_root, read_chunk, assemble};
let volumes = file_volumes("text/plain", b"hello")?;
let root = read_root(&volumes[0], Some(10_000)).unwrap();
let chunks = volumes.iter().skip(1).enumerate()
    .map(|(i, volume)| read_chunk(volume, &root, i).unwrap()).collect::<Vec<_>>();
assert_eq!(assemble(&root, &chunks)?, b"hello");
# Ok::<(), lattice_storage::FileError>(())
```

Use `read_chunk` before `assemble`: assembly checks lengths, not content hashes. `FileRoot` can only be constructed through the structural checks in `read_root`. Pinning, payment, availability, and application records belong to callers.

Run `cargo test --locked` and `cargo clippy --all-targets --locked -- -D warnings`. See [PORTING.md](PORTING.md) for provenance and limits. Dependencies currently use sibling Rust checkouts; this crate is not published.
