// Thin binary: the platform lives in the `cctf_rs` library crate (src/lib.rs).
// Bootstrap (axum server, PgStore, AppState) will be wired here next.
fn main() {
    println!("Hello, world!");
}
