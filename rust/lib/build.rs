fn main() {
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-changed=../shared-data/build-name.txt");
    let name = std::fs::read_to_string("../shared-data/build-name.txt")
        .expect("lib reads shared-data/build-name.txt");
    println!("cargo:rustc-env=LAB_LIB_BUILD={}", name.trim());
}
