fn main() {
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rustc-env=LAB_LIB_BUILD=lib");
}
