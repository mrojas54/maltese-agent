//! Deterministic Caesar-shift decoder. The "real" decoding logic the noir
//! service pretends to do; used by tests and the fake LLM.

pub fn caesar_decode(ciphertext: &str, shift: u8) -> String {
    ciphertext
        .chars()
        .map(|c| {
            if c.is_ascii_alphabetic() {
                let base = if c.is_ascii_uppercase() { b'A' } else { b'a' };
                let offset = (c as u8 - base + 26 - (shift % 26)) % 26;
                (base + offset) as char
            } else {
                c
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decodes_simple_shift() {
        assert_eq!(caesar_decode("Khoor", 3), "Hello");
    }
    #[test]
    fn decodes_falcon_phrase() {
        assert_eq!(caesar_decode("Wkh idofrq lv jrqh", 3), "The falcon is gone");
    }
    #[test]
    fn decodes_bird_phrase() {
        assert_eq!(caesar_decode("Wkh elug", 3), "The bird");
    }
    #[test]
    fn preserves_non_alpha() {
        assert_eq!(caesar_decode("hi, friend!", 0), "hi, friend!");
    }
}
