//! CurseForge identifies files by a fingerprint: 32-bit MurmurHash2 (seed 1)
//! of the file with whitespace bytes removed.

/// CurseForge's file fingerprint: 32-bit MurmurHash2 (seed 1) of the file
/// with every whitespace byte (tab, LF, CR, space) removed.
pub fn fingerprint(bytes: &[u8]) -> u32 {
    const M: u32 = 0x5bd1_e995;
    let skip = |b: &u8| matches!(b, 9 | 10 | 13 | 32);
    // Same as `murmur2` on the filtered bytes, without copying the file.
    let len = bytes.iter().filter(|b| !skip(b)).count() as u32;
    let mut h = 1 ^ len;
    let (mut word, mut filled) = (0u32, 0u32);
    for &b in bytes.iter().filter(|b| !skip(b)) {
        word |= u32::from(b) << (8 * filled);
        filled += 1;
        if filled == 4 {
            let mut k = word.wrapping_mul(M);
            k ^= k >> 24;
            k = k.wrapping_mul(M);
            h = h.wrapping_mul(M) ^ k;
            (word, filled) = (0, 0);
        }
    }
    if filled > 0 {
        h ^= word;
        h = h.wrapping_mul(M);
    }
    h ^= h >> 13;
    h = h.wrapping_mul(M);
    h ^ (h >> 15)
}

/// Reference MurmurHash2 — [`fingerprint`] streams the same computation.
#[cfg(test)]
fn murmur2(data: &[u8], seed: u32) -> u32 {
    const M: u32 = 0x5bd1_e995;
    let mut h = seed ^ data.len() as u32;
    let (chunks, tail) = data.as_chunks::<4>();
    for chunk in chunks {
        let mut k = u32::from_le_bytes(*chunk);
        k = k.wrapping_mul(M);
        k ^= k >> 24;
        k = k.wrapping_mul(M);
        h = h.wrapping_mul(M) ^ k;
    }
    if !tail.is_empty() {
        for (i, byte) in tail.iter().enumerate() {
            h ^= u32::from(*byte) << (8 * i);
        }
        h = h.wrapping_mul(M);
    }
    h ^= h >> 13;
    h = h.wrapping_mul(M);
    h ^ (h >> 15)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn murmur2_matches_the_reference_implementation() {
        // Values from the `murmurhash2` reference package.
        assert_eq!(murmur2(b"", 1), 1_540_447_798);
        assert_eq!(murmur2(b"a", 0x9747_b28c), 2_731_586_172);
        assert_eq!(murmur2(b"Hello, world!", 0x9747_b28c), 3_199_900_434);
        assert_eq!(murmur2(b"abcdefg", 1), 184_182_053);
    }

    #[test]
    fn streaming_fingerprint_matches_murmur2_on_the_filtered_bytes() {
        for sample in [&b""[..], b"a", b"ab c", b"abc d\te\r\nfgh", b"PK\x03\x04 some jar bytes\n"] {
            let filtered: Vec<u8> = sample.iter().copied().filter(|b| !matches!(b, 9 | 10 | 13 | 32)).collect();
            assert_eq!(fingerprint(sample), murmur2(&filtered, 1), "{sample:?}");
        }
    }

    #[test]
    fn fingerprint_ignores_whitespace() {
        assert_eq!(fingerprint(b"a b
	c"), fingerprint(b"abc"));
    }
}
