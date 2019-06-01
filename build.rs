use chrono::offset::Utc;
fn main() {
    println!("cargo:rustc-env=BUILD_DATE={:?}", Utc::now());
}
