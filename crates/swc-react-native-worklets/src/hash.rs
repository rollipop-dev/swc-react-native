// Port of `worklet_hash` in plugin-oxc/src/naming.rs.

pub fn worklet_hash(s: &str) -> u64 {
    let units: Vec<_> = s.encode_utf16().collect();
    let mut i = units.len();
    let mut hash1: u64 = 5381;
    let mut hash2: u64 = 52711;

    while i > 0 {
        i -= 1;
        let c = units[i] as u64;
        hash1 = (hash1.wrapping_mul(33)) ^ c;
        hash2 = (hash2.wrapping_mul(33)) ^ c;
    }

    (hash1 & 0xFFFFFFFF)
        .wrapping_mul(4096)
        .wrapping_add(hash2 & 0xFFFFFFFF)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn deterministic() {
        let h = worklet_hash("function foo(){return 1;}");
        assert_eq!(h, worklet_hash("function foo(){return 1;}"));
        assert_ne!(h, worklet_hash("function foo(){return 2;}"));
    }

    #[test]
    fn matches_official_oxc_vectors_and_javascript_utf16() {
        assert_eq!(
            worklet_hash("function testJs1(x){return x+2;}"),
            919891681460
        );
        assert_eq!(
            worklet_hash("function foo_testJs1(x){return x+2;}"),
            11633341088429
        );
        assert_eq!(
            worklet_hash("function f(){return \"\u{1F30D}\";}"),
            14475599329188
        );
    }
}
