use std::process::exit;

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.first().map(String::as_str) == Some("--version") {
        println!("decka-headless {}", decka_core::VERSION);
        exit(0);
    }
    println!("not implemented");
    exit(1);
}