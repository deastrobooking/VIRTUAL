//! Regenerates the client test vectors:
//! `cargo run -p virtual-sync --example vectors`.
fn main() -> std::io::Result<()> {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../clients");
    std::fs::write(
        root.join("c/test/vectors.h"),
        virtual_sync::vectors::c_header(),
    )?;
    std::fs::write(
        root.join("test-vectors.json"),
        virtual_sync::vectors::json(),
    )?;
    println!("wrote clients/c/test/vectors.h and clients/test-vectors.json");
    Ok(())
}
