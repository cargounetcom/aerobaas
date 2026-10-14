//! AeroBaaS function example: reads a JSON payload from stdin, writes JSON to stdout.
//!
//! Build:  rustup target add wasm32-wasip1
//!         cargo build --release --target wasm32-wasip1
//! Deploy: base64 the file target/wasm32-wasip1/release/hello-rust.wasm into `wasm_base64`
//!         (POST /v1/functions or the MCP tool deploy_function).

use std::io::{Read, Write};

fn main() {
    let mut input = String::new();
    std::io::stdin().read_to_string(&mut input).unwrap_or_default();
    let payload: serde_json::Value = serde_json::from_str(&input).unwrap_or(serde_json::Value::Null);

    let name = payload
        .get("name")
        .and_then(|v| v.as_str())
        .unwrap_or("world");
    eprintln!("greeting {name}"); // stderr comes back as `logs`

    let out = serde_json::json!({ "message": format!("Hello, {name}!"), "length": name.len() });
    std::io::stdout()
        .write_all(out.to_string().as_bytes())
        .expect("write stdout");
}
