use sagex_crypto::{MlDsa65, aes::KeyEncapsulation};

fn main() {
    let _cap = KeyEncapsulation::new::<MlDsa65>(b"mainak is my name").unwrap();
    println!("Hello, world!");
}
