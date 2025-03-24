use crate::security::{
    crypto::{decrypt_string_aes256_gcm, encrypt_string_aes256_gcm},
    secrets::create_service_secret,
};

#[test]
fn test_round_trip_encryption() {
    let secret = create_service_secret().unwrap().value;

    let encrypted = encrypt_string_aes256_gcm("test plaintext", &secret).unwrap();
    assert_ne!(encrypted, "test plaintext");

    let decrypted = decrypt_string_aes256_gcm(&encrypted, &secret).unwrap();
    assert_eq!(decrypted, "test plaintext");
}
