use ring::digest::{Context, SHA256};

use crate::storage::Task;

const POW_VERSION: &str = "v1";
const POW_DIVIDER: &str = "|";

pub fn verify_pow(task: &Task, nonce: &str) -> bool {
    let mut ctx = Context::new(&SHA256);
    append_digest_field(&mut ctx, POW_VERSION, false);
    append_digest_field(&mut ctx, task.seed.0.as_str(), false);
    append_digest_field(&mut ctx, task.exp.to_string().as_str(), false);
    append_digest_field(&mut ctx, task.bits.to_string().as_str(), false);
    append_digest_field(&mut ctx, task.scope.0.as_str(), false);
    append_digest_field(&mut ctx, task.ua_hash.0.as_str(), false);
    append_digest_field(&mut ctx, nonce, true);
    let digest = ctx.finish();
    let leading = count_leading_zero_bits(&digest.as_ref());
    leading >= task.bits as i32
}

fn append_digest_field(ctx: &mut Context, param: &str, is_last: bool) {
    ctx.update(param.as_bytes());
    if !is_last {
        ctx.update(POW_DIVIDER.as_bytes());
    }
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
