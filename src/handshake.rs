use base64::engine::general_purpose::STANDARD;
use base64::Engine;
use sha1_smol::Sha1;

pub fn handshake(mut key: String) -> String {
    key.push_str("258EAFA5-E914-47DA-95CA-C5AB0DC85B11");
    let mut hash = Sha1::new();
    hash.update(key.as_bytes());
    STANDARD.encode(hash.digest().bytes())
}

#[cfg(test)]
mod tests {
    #[test]
    fn handshake() {
        assert_eq!(
            super::handshake("dGhlIHNhbXBsZSBub25jZQ==".to_owned()),
            "s3pPLMBiTxaQ9kYGzzhZRbK+xOo=",
        );
    }
}
