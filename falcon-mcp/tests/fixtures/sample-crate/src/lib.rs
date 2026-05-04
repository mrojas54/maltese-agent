pub fn add(a: i32, b: i32) -> i32 {
    a + b
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn add_works() {
        assert_eq!(add(2, 3), 5);
    }

    #[test]
    fn add_broken() {
        // Intentionally failing — the cargo_test integration tests assert
        // that this failure is reported correctly.
        assert_eq!(add(2, 3), 99);
    }
}
