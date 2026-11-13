mod hash;

fn main() {
    for arg in std::env::args().skip(1) {
        match hash::parse(&arg) {
            Ok(bytes) => println!("{} bytes", bytes.len()),
            Err(_) => println!("{}", hash::fingerprint(arg.as_bytes())),
        }
    }
}
