use rand::Rng;

pub fn generate_api_key() -> String {
    let prefix = "aero_";
    let random_part: String = (0..48)
        .map(|_| {
            let idx = rand::thread_rng().gen_range(0..62);
            match idx {
                0..=9 => (b'0' + idx) as char,
                10..=35 => (b'a' + idx - 10) as char,
                _ => (b'A' + idx - 36) as char,
            }
        })
        .collect();
    format!("{prefix}{random_part}")
}

pub fn key_prefix(key: &str) -> String {
    if key.len() >= 12 {
        key[..12].to_string()
    } else {
        key.to_string()
    }
}
