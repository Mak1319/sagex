//! E2EE placeholder (user plugs real crypto here later).
//!
//! Contract: `encrypt_outgoing` feeds `POST /messages {body}` and
//! `decrypt_incoming` renders what comes back. Both are raw passthrough
//! today — the server stores whatever it gets.

/// "Encrypt" for sending: currently the raw bytes, untouched.
pub fn encrypt_outgoing(plain: &str) -> String {
    // TODO(you): real E2EE — encrypt with the group's key, base64 the result.
    plain.to_string()
}

/// "Decrypt" for display: currently the stored bytes, untouched.
pub fn decrypt_incoming(stored: &str) -> String {
    // TODO(you): real E2EE — base64-decode and decrypt with the group's key.
    stored.to_string()
}
