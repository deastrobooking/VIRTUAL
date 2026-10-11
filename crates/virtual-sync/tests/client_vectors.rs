//! The committed client test vectors must match the current encoder.
use std::path::Path;

#[test]
fn committed_client_vectors_match_the_encoder() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../clients");
    let header = std::fs::read_to_string(root.join("c/test/vectors.h")).unwrap();
    let json = std::fs::read_to_string(root.join("test-vectors.json")).unwrap();
    assert_eq!(
        header,
        virtual_sync::vectors::c_header(),
        "run `cargo run -p virtual-sync --example vectors`"
    );
    assert_eq!(json, virtual_sync::vectors::json());
}
