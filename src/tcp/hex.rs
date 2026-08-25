// Codec hexadecimal minimal pour les octets bruts (prefixe de matching,
// corps de reponse mock) stockes dans le YAML. Prefere a une dependance
// `hex`/`base64` externe : quelques lignes suffisent, zero crate
// supplementaire a auditer pour cette premiere tranche du mock TCP (cf
// Cargo.toml, feature "tcp-proxy").

pub fn encode(bytes: &[u8]) -> String {
    let mut out = String::with_capacity(bytes.len() * 2);
    for b in bytes {
        out.push_str(&format!("{b:02x}"));
    }
    out
}

#[derive(Debug, PartialEq)]
pub enum HexDecodeError {
    OddLength,
    InvalidChar(char),
}

impl std::fmt::Display for HexDecodeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            HexDecodeError::OddLength => write!(f, "longueur hexadecimale impaire"),
            HexDecodeError::InvalidChar(c) => write!(f, "caractere hexadecimal invalide: {c:?}"),
        }
    }
}

pub fn decode(hex: &str) -> Result<Vec<u8>, HexDecodeError> {
    if hex.is_empty() {
        return Ok(Vec::new());
    }
    if !hex.len().is_multiple_of(2) {
        return Err(HexDecodeError::OddLength);
    }
    let mut out = Vec::with_capacity(hex.len() / 2);
    let bytes = hex.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        let hi = nibble(bytes[i] as char)?;
        let lo = nibble(bytes[i + 1] as char)?;
        out.push((hi << 4) | lo);
        i += 2;
    }
    Ok(out)
}

fn nibble(c: char) -> Result<u8, HexDecodeError> {
    match c {
        '0'..='9' => Ok(c as u8 - b'0'),
        'a'..='f' => Ok(c as u8 - b'a' + 10),
        'A'..='F' => Ok(c as u8 - b'A' + 10),
        other => Err(HexDecodeError::InvalidChar(other)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn encode_empty() {
        assert_eq!(encode(&[]), "");
    }

    #[test]
    fn decode_empty() {
        assert_eq!(decode("").unwrap(), Vec::<u8>::new());
    }

    #[test]
    fn roundtrip() {
        let bytes = [0x30, 0x0c, 0x02, 0x01, 0x00, 0xff, 0xab];
        let hex = encode(&bytes);
        assert_eq!(decode(&hex).unwrap(), bytes);
    }

    #[test]
    fn decode_uppercase_and_lowercase() {
        assert_eq!(decode("Ab").unwrap(), vec![0xab]);
        assert_eq!(decode("AB").unwrap(), vec![0xab]);
        assert_eq!(decode("ab").unwrap(), vec![0xab]);
    }

    #[test]
    fn decode_odd_length_errors() {
        assert_eq!(decode("abc"), Err(HexDecodeError::OddLength));
    }

    #[test]
    fn decode_invalid_char_errors() {
        assert_eq!(decode("zz"), Err(HexDecodeError::InvalidChar('z')));
    }

    #[test]
    fn encode_known_value() {
        assert_eq!(encode(&[0x30, 0x0c]), "300c");
    }
}
