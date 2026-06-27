// Generation de codes courts uniques pour les groupes.
// Algorithme : hash FNV-1a du nom (deterministe, reproductible cross-instance)
// encode en base36, tronque a 5 chars. En cas de collision, on ajoute un
// discriminant incremental au hash avant re-encodage.
// Cela garantit que le meme nom produit le meme code sur deux instances
// independantes (sauf collision avec un groupe existant).

const BASE36: &[u8] = b"abcdefghijklmnopqrstuvwxyz0123456789";
const CODE_LEN: usize = 5;

fn fnv1a_hash(input: &str) -> u64 {
    let mut hash: u64 = 0xcbf29ce484222325;
    for byte in input.as_bytes() {
        hash ^= *byte as u64;
        hash = hash.wrapping_mul(0x100000001b3);
    }
    hash
}

fn encode_base36(mut value: u64) -> String {
    let mut result = Vec::with_capacity(CODE_LEN);
    for _ in 0..CODE_LEN {
        result.push(BASE36[(value % 36) as usize]);
        value /= 36;
    }
    result.into_iter().map(|b| b as char).collect()
}

pub fn generate_code(name: &str, existing_codes: &[String]) -> String {
    let base_hash = fnv1a_hash(&name.to_lowercase());
    for discriminant in 0u64..1000 {
        let hash = base_hash.wrapping_add(discriminant);
        let code = encode_base36(hash);
        if !existing_codes.iter().any(|c| c.eq_ignore_ascii_case(&code)) {
            return code;
        }
    }
    encode_base36(base_hash.wrapping_add(fastrand::u64(..)))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn deterministic_same_name_same_code() {
        let code1 = generate_code("saber", &[]);
        let code2 = generate_code("saber", &[]);
        assert_eq!(code1, code2);
    }

    #[test]
    fn different_names_different_codes() {
        let code1 = generate_code("alpha", &[]);
        let code2 = generate_code("bravo", &[]);
        assert_ne!(code1, code2);
    }

    #[test]
    fn code_is_5_chars_alphanumeric() {
        let code = generate_code("test-group", &[]);
        assert_eq!(code.len(), 5);
        assert!(code.chars().all(|c| c.is_ascii_alphanumeric()));
    }

    #[test]
    fn avoids_collision_with_existing() {
        let first = generate_code("mygroup", &[]);
        let second = generate_code("mygroup", &[first.clone()]);
        assert_ne!(first, second);
        assert_eq!(second.len(), 5);
    }

    #[test]
    fn case_insensitive_collision_check() {
        let first = generate_code("test", &[]);
        let upper = first.to_uppercase();
        let second = generate_code("test", &[upper]);
        assert_ne!(first.to_lowercase(), second.to_lowercase());
    }
}
