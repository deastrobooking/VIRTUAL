//! SipHash-2-4 message authentication and show-key derivation.
//!
//! SipHash is a keyed pseudorandom function designed for authenticating short
//! messages. It needs no tables or big integers, so an ESP32 or an Arduino
//! computes it in microseconds; `clients/c/virtual_sync.h` has the same code.

/// A 128-bit show key.
pub type Key = [u64; 2];

/// SipHash-2-4 of `data` under `key`.
pub fn siphash24(key: Key, data: &[u8]) -> u64 {
    let [k0, k1] = key;
    let mut v0 = 0x736f_6d65_7073_6575 ^ k0;
    let mut v1 = 0x646f_7261_6e64_6f6d ^ k1;
    let mut v2 = 0x6c79_6765_6e65_7261 ^ k0;
    let mut v3 = 0x7465_6462_7974_6573 ^ k1;
    let round = |v0: &mut u64, v1: &mut u64, v2: &mut u64, v3: &mut u64| {
        *v0 = v0.wrapping_add(*v1);
        *v1 = v1.rotate_left(13) ^ *v0;
        *v0 = v0.rotate_left(32);
        *v2 = v2.wrapping_add(*v3);
        *v3 = v3.rotate_left(16) ^ *v2;
        *v0 = v0.wrapping_add(*v3);
        *v3 = v3.rotate_left(21) ^ *v0;
        *v2 = v2.wrapping_add(*v1);
        *v1 = v1.rotate_left(17) ^ *v2;
        *v2 = v2.rotate_left(32);
    };
    let mut chunks = data.chunks_exact(8);
    for chunk in &mut chunks {
        let m = u64::from_le_bytes(chunk.try_into().expect("8-byte chunk"));
        v3 ^= m;
        round(&mut v0, &mut v1, &mut v2, &mut v3);
        round(&mut v0, &mut v1, &mut v2, &mut v3);
        v0 ^= m;
    }
    let mut last = [0u8; 8];
    let tail = chunks.remainder();
    last[..tail.len()].copy_from_slice(tail);
    last[7] = data.len() as u8;
    let m = u64::from_le_bytes(last);
    v3 ^= m;
    round(&mut v0, &mut v1, &mut v2, &mut v3);
    round(&mut v0, &mut v1, &mut v2, &mut v3);
    v0 ^= m;
    v2 ^= 0xff;
    for _ in 0..4 {
        round(&mut v0, &mut v1, &mut v2, &mut v3);
    }
    v0 ^ v1 ^ v2 ^ v3
}

/// Derives the 128-bit show key from a passphrase typed into every device.
/// Two SipHash passes under fixed, published keys; an empty passphrase means
/// unsigned traffic.
pub fn derive_key(passphrase: &str) -> Option<Key> {
    let passphrase = passphrase.trim();
    if passphrase.is_empty() {
        return None;
    }
    const SALT_A: Key = [0x5649_5254_5541_4c31, 0x7379_6e63_6b65_7961]; // "VIRTUAL1" "synckeya"
    const SALT_B: Key = [0x5649_5254_5541_4c32, 0x7379_6e63_6b65_7962]; // "VIRTUAL2" "synckeyb"
    Some([
        siphash24(SALT_A, passphrase.as_bytes()),
        siphash24(SALT_B, passphrase.as_bytes()),
    ])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn matches_the_reference_test_vectors() {
        // Key 00 01 .. 0f; messages 00 01 .. (n-1). From the SipHash paper.
        let key = [0x0706_0504_0302_0100, 0x0f0e_0d0c_0b0a_0908];
        let message: Vec<u8> = (0..64).collect();
        assert_eq!(siphash24(key, &message[..0]), 0x726f_db47_dd0e_0e31);
        assert_eq!(siphash24(key, &message[..1]), 0x74f8_39c5_93dc_67fd);
        assert_eq!(siphash24(key, &message[..8]), 0x93f5_f579_9a93_2462);
        assert_eq!(siphash24(key, &message[..15]), 0xa129_ca61_49be_45e5);
        assert_eq!(siphash24(key, &message[..63]), 0x958a_324c_eb06_4572);
    }

    #[test]
    fn passphrases_derive_stable_distinct_keys() {
        assert_eq!(derive_key("  "), None);
        assert_eq!(derive_key("disco"), derive_key(" disco "));
        assert_ne!(derive_key("disco"), derive_key("Disco"));
    }
}
