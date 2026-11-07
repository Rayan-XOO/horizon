//! `cargo run -p horizon-prepare --example from_client_jar -- CLIENT.jar OUT`: the Java-free part.
fn main() {
    let a: Vec<String> = std::env::args().collect();
    let n = horizon_prepare::device::from_client_jar(
        a[1].as_ref(),
        a[2].as_ref(),
        &|_| None,
        &|_, _| {},
    )
    .unwrap();
    println!("{n} resource files");
}
