use sagex_crypto::{MlDsa44, MlKem768, aes::KeyEncapsulation};

fn main() {
    let cap = KeyEncapsulation::new::<MlDsa44>(b"mainak is my name").unwrap();
    println!("Hello, world!");
}
