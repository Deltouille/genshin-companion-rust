//! HoYoLAB's "dynamic secret" (`DS` header): the time, six random letters
//! and an MD5 of both with a salt the overseas apps use. Not a secret of the
//! player's: it proves nothing but that the client knows the salt.

use md5::{Digest, Md5};

/// The overseas salt, as genshin.py has it (`constants.DS_SALT`). HoYoverse
/// changes it now and then; a request then fails with "invalid request".
const OVERSEAS_SALT: &str = "6s25p5ox5y14umn1p61aqyyvbvvl3lrt";

/// The `DS` header for a request sent at `time` (seconds since the epoch).
pub(crate) fn dynamic_secret(time: u64) -> String {
    let random: String = (0..6).map(|_| fastrand::alphabetic()).collect();
    sign(time, &random)
}

fn sign(time: u64, random: &str) -> String {
    let digest = Md5::digest(format!("salt={OVERSEAS_SALT}&t={time}&r={random}"));
    let hex: String = digest.iter().map(|byte| format!("{byte:02x}")).collect();
    format!("{time},{random},{hex}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_signature_is_genshin_pys() {
        // printf 'salt=…&t=1700000000&r=AbCdEf' | md5sum
        assert_eq!(
            sign(1_700_000_000, "AbCdEf"),
            "1700000000,AbCdEf,52762c606b53f6830e4692f53664105e"
        );
        let ds = dynamic_secret(1_700_000_000);
        let parts: Vec<&str> = ds.split(',').collect();
        assert_eq!(parts.len(), 3);
        assert_eq!(parts[1].len(), 6);
        assert!(parts[1].chars().all(|c| c.is_ascii_alphabetic()));
    }
}
