use std::arch::x86_64::{
    __m256i, _mm256_cmpeq_epi8, _mm256_loadu_si256, _mm256_movemask_epi8, _mm256_set1_epi8,
};

/// Looks for NEEDLE in the first 64 bytes of haystack.
#[cfg_attr(feature = "profiled", inline(never))]
unsafe fn memchr32_unchecked<const NEEDLE: u8>(haystack: &[u8]) -> usize {
    let ptr = haystack.as_ptr();
    let haystack_vec = unsafe { _mm256_loadu_si256(ptr as *const __m256i) };

    let needle_vec: __m256i = unsafe { _mm256_set1_epi8(NEEDLE as i8) };
    let cmp = unsafe { _mm256_cmpeq_epi8(haystack_vec, needle_vec) };

    let mask = unsafe { _mm256_movemask_epi8(cmp) } as u32;

    mask.trailing_zeros() as usize
}

/// Looks for NEEDLE in the first 64 bytes of haystack.
///
/// This will panic if the character is not present.
/// This will may read 64 bytes, even if the slice is less than 64 bytes.
#[cfg_attr(feature = "profiled", inline(never))]
pub unsafe fn memchr64_unchecked<const NEEDLE: u8>(haystack: &[u8]) -> usize {
    unsafe {
        let r = memchr32_unchecked::<NEEDLE>(haystack);
        if r < 32 {
            r
        } else {
            32 + memchr32_unchecked::<NEEDLE>(haystack.get_unchecked(32..))
        }
    }
}

/// Checks that up to the first 32 bytes of a and b are equal.
///
/// If the provided slice is <32 bytes, this will read past the end.
#[cfg_attr(feature = "profiled", inline(never))]
pub unsafe fn memeq32_unchecked(a: &[u8], b: &[u8]) -> bool {
    let a_vec = unsafe { _mm256_loadu_si256(a.as_ptr() as *const __m256i) };
    let b_vec = unsafe { _mm256_loadu_si256(b.as_ptr() as *const __m256i) };
    let cmp = unsafe { _mm256_cmpeq_epi8(a_vec, b_vec) };
    let mask = unsafe { _mm256_movemask_epi8(cmp) } as u32;
    a.len() == b.len() && mask.trailing_ones() >= a.len().min(32) as u32
}

/// Checks that up to the first 64 bytes of a and b are equal.
///
/// If the provided slice is <64 bytes, this may read past the end.
#[cfg_attr(feature = "profiled", inline(never))]
pub unsafe fn memeq64_unchecked(a: &[u8], b: &[u8]) -> bool {
    let res = unsafe { memeq32_unchecked(a, b) };
    if a.len() <= 32 {
        res
    } else {
        res && unsafe { memeq32_unchecked(a.get_unchecked(32..), b.get_unchecked(32..)) }
    }
}

#[cfg(test)]
mod test {
    use crate::memops::{memchr64_unchecked, memeq32_unchecked};

    fn pad64(s: &str) -> String {
        if s.len() >= 64 {
            s.to_owned()
        } else {
            let mut res = [0u8; 64];
            (&mut res[..s.len()]).copy_from_slice(s.as_bytes());
            std::str::from_utf8(&res).unwrap().to_owned()
        }
    }

    fn safe_memeq32(a: &str, b: &str) -> bool {
        unsafe { memeq32_unchecked(pad64(a).as_bytes(), pad64(b).as_bytes()) }
    }

    #[test]
    fn test_memeq32() {
        assert!(safe_memeq32("abcd", "abcd"));
        assert!(!safe_memeq32("abcd", "abc"));
        assert!(!safe_memeq32("aaa", "bbb"));
        assert!(safe_memeq32(
            "aaaaaaaabbbbbbbbccccccccdddddddd_AAA",
            "aaaaaaaabbbbbbbbccccccccdddddddd_BBB"
        ));
    }

    fn safe_memchr64<const NEEDLE: u8>(haystack: &str) -> usize {
        unsafe { memchr64_unchecked::<NEEDLE>(pad64(haystack).as_bytes()) }
    }

    #[test]
    fn test_memchr64() {
        assert_eq!(safe_memchr64::<b'A'>("aaaAaaa"), 3);
        assert_eq!(safe_memchr64::<b'A'>("aaaAaaaAaaa"), 3);
    }
}
