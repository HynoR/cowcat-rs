use ring::digest::SHA256;

use crate::storage::Task;

pub fn verify_pow(task: &Task, nonce: &str) -> bool {
    let msg = format!(
        "v1|{}|{}|{}|{}|{}|{}",
        task.seed.0, task.exp, task.bits, task.scope.0, task.ua_hash.0, nonce
    );
    let digest = ring::digest::digest(&SHA256, msg.as_bytes());
    let leading = count_leading_zero_bits(digest.as_ref());
    leading >= task.bits as i32
}

fn count_leading_zero_bits(hash: &[u8]) -> i32 {
    let mut count = 0;
    for &byte in hash {
        if byte == 0 {
            count += 8;
        } else {
            for i in (0..8).rev() {
                if (byte >> i) & 1 == 0 {
                    count += 1;
                } else {
                    return count;
                }
            }
        }
    }
    count
}
