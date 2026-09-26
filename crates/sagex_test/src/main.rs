use sagex_crypto::backend::tpm::is_available;

fn main() {
    // match is_available() {
    //     Err(x) => println!("err{:?}", x),
    //     Ok(x) => println!("res{:?}", x),
    // }
    println!("{}", is_available());
    println!("Hello, world!");
}
