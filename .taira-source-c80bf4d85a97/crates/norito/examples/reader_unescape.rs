//! Example: Streaming tokens and unescaping strings with Norito JSON.
//! Run: `cargo run -p norito --example reader_unescape --features json -- "{\"name\":\"al\\u0069ce\"}"`

#[cfg(feature = "json")]
use norito::json::{Reader, Token, unescape_json_string};

#[cfg(feature = "json")]
fn main() {
    let input = std::env::args()
        .nth(1)
        .unwrap_or_else(|| r#"{"name":"al\u0069ce"}"#.to_string());
    let mut rdr = Reader::new(&input);
    while let Some(tok) = rdr.next().expect("tokenize") {
        match tok {
            Token::KeyBorrowed(k) => {
                let key = unescape_json_string(k).expect("unescape key");
                println!("key: {key}");
            }
            Token::StringBorrowed(s) => {
                let val = unescape_json_string(s).expect("unescape val");
                println!("value: {val}");
            }
            Token::Number(n) => println!("value: {n}"),
            Token::Bool(b) => println!("value: {b}"),
            Token::Null => println!("value: null"),
            Token::StartObject => println!("{{"),
            Token::EndObject => println!("}}"),
            Token::StartArray => println!("["),
            Token::EndArray => println!("]"),
        }
    }
}

#[cfg(not(feature = "json"))]
fn main() {
    eprintln!("Enable the `json` feature to run this example");
}
