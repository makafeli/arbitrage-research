//! Dependency-level compatibility checks for the STANDARD account-data codec.
//! These synthetic byte fixtures do not qualify RPC data or any live pool.
use base64::{Engine, engine::general_purpose::STANDARD};

#[test]
fn canonical_fixed_vectors_remain_byte_for_byte_stable() {
    for (plain, encoded) in [
        ("", ""),
        ("f", "Zg=="),
        ("fo", "Zm8="),
        ("foo", "Zm9v"),
        ("foob", "Zm9vYg=="),
        ("fooba", "Zm9vYmE="),
        ("foobar", "Zm9vYmFy"),
    ] {
        assert_eq!(STANDARD.encode(plain.as_bytes()), encoded);
        assert_eq!(STANDARD.decode(encoded).unwrap(), plain.as_bytes());
    }
    let binary = [0_u8, 1, 2, 254, 255];
    assert_eq!(STANDARD.encode(binary), "AAEC/v8=");
    assert_eq!(STANDARD.decode("AAEC/v8=").unwrap(), binary);
}

#[test]
fn noncanonical_padding_and_nonzero_trailing_bits_are_rejected() {
    for encoded in "Zg Zg= Zg=== Zm8 Zm8== Zm9v= Zh== Zm9=".split(' ') {
        assert!(STANDARD.decode(encoded).is_err(), "accepted {encoded:?}");
    }
}

#[test]
fn whitespace_url_alphabet_and_embedded_control_bytes_are_rejected() {
    for encoded in [" Zg==", "Zg==\n", "____", "----", "AA\0=", "!!!!"] {
        assert!(STANDARD.decode(encoded).is_err(), "accepted {encoded:?}");
    }
}

#[test]
fn boundary_lengths_and_all_byte_values_round_trip_without_truncation() {
    let all_bytes: Vec<u8> = (0..=u8::MAX).collect();
    let encoded_all = STANDARD.encode(&all_bytes);
    assert_eq!(STANDARD.decode(encoded_all).unwrap(), all_bytes);
    for len in (0..=1024_usize).chain([9988]) {
        let bytes: Vec<u8> = (0..len).map(|i| (i % 256) as u8).collect();
        let encoded = STANDARD.encode(&bytes);
        assert_eq!(encoded.len(), len.div_ceil(3) * 4);
        let decoded = STANDARD.decode(encoded).unwrap();
        assert_eq!(decoded.len(), len);
        assert_eq!(decoded, bytes);
    }
}
