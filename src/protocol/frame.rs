use std::collections::HashMap;

use crate::storage::Task;

pub const FRAME_MAGIC0: u8 = b'C';
pub const FRAME_MAGIC1: u8 = b'W';
pub const FRAME_VERSION: u8 = 1;

pub const FRAME_TYPE_TASK_REQUEST: u8 = 1;
pub const FRAME_TYPE_TASK_RESPONSE: u8 = 2;
pub const FRAME_TYPE_VERIFY_REQUEST: u8 = 3;
pub const FRAME_TYPE_VERIFY_RESPONSE: u8 = 4;
pub const FRAME_TYPE_ERROR: u8 = 5;

pub const XOR_KEY: &[u8] = b"cowcatwaflibwafcatcow";

pub const TLV_REDIRECT: u8 = 0x01;
pub const TLV_TASK_ID: u8 = 0x02;
pub const TLV_SEED: u8 = 0x03;
pub const TLV_EXP: u8 = 0x04;
pub const TLV_BITS: u8 = 0x05;
pub const TLV_SCOPE: u8 = 0x06;
pub const TLV_UA_HASH: u8 = 0x07;
pub const TLV_IP_HASH: u8 = 0x08;
pub const TLV_WORKERS: u8 = 0x09;
pub const TLV_NONCE: u8 = 0x0a;
pub const TLV_WORKER_TYPE: u8 = 0x0b;
pub const TLV_ERROR: u8 = 0x0f;

#[derive(Debug, Clone)]
pub struct BinaryTaskRequest {
    #[allow(dead_code)]
    pub redirect: String,
}

#[derive(Debug, Clone)]
pub struct BinaryTaskResponse {
    pub task_id: String,
    pub seed: String,
    pub bits: i32,
    pub exp: i64,
    pub scope: String,
    pub ua_hash: String,
    pub ip_hash: String,
    pub workers: i32,
    pub worker_type: String,
}

#[derive(Debug, Clone)]
pub struct BinaryVerifyRequest {
    pub task_id: String,
    pub nonce: String,
    pub redirect: String,
}

#[derive(Debug, Clone)]
pub struct BinaryVerifyResponse {
    pub redirect: String,
}

pub fn encode_frame(frame_type: u8, payload: Vec<u8>) -> Vec<u8> {
    let mut buf = Vec::with_capacity(8 + payload.len());
    buf.push(FRAME_MAGIC0);
    buf.push(FRAME_MAGIC1);
    buf.push(FRAME_VERSION);
    buf.push(frame_type);
    buf.extend_from_slice(&(payload.len() as u32).to_be_bytes());
    buf.extend_from_slice(&payload);
    buf
}

pub fn decode_frame(data: &[u8]) -> anyhow::Result<(u8, &[u8])> {
    if data.len() < 8 {
        anyhow::bail!("frame too short");
    }
    if data[0] != FRAME_MAGIC0 || data[1] != FRAME_MAGIC1 {
        anyhow::bail!("bad magic");
    }
    if data[2] != FRAME_VERSION {
        anyhow::bail!("unsupported version");
    }
    let frame_type = data[3];
    let payload_len = u32::from_be_bytes([data[4], data[5], data[6], data[7]]) as usize;
    if payload_len != data.len() - 8 {
        anyhow::bail!("length mismatch");
    }
    Ok((frame_type, &data[8..]))
}

pub fn deobfuscate_frame(data: &mut [u8], key: &[u8]) {
    for (idx, byte) in data.iter_mut().enumerate() {
        *byte ^= key[idx % key.len()];
    }
}

pub fn encode_task_response(resp: BinaryTaskResponse) -> Vec<u8> {
    let mut payload = Vec::new();
    payload = append_tlv(payload, TLV_TASK_ID, resp.task_id.as_bytes());
    payload = append_tlv(payload, TLV_SEED, resp.seed.as_bytes());
    payload = append_tlv(payload, TLV_EXP, &(resp.exp as u64).to_be_bytes());
    payload = append_tlv(payload, TLV_BITS, &(resp.bits as u16).to_be_bytes());
    payload = append_tlv(payload, TLV_SCOPE, resp.scope.as_bytes());
    payload = append_tlv(payload, TLV_UA_HASH, resp.ua_hash.as_bytes());
    payload = append_tlv(payload, TLV_IP_HASH, resp.ip_hash.as_bytes());
    payload = append_tlv(payload, TLV_WORKERS, &[resp.workers as u8]);
    if !resp.worker_type.is_empty() {
        payload = append_tlv(payload, TLV_WORKER_TYPE, resp.worker_type.as_bytes());
    }
    payload
}

pub fn decode_task_request(payload: &[u8]) -> anyhow::Result<BinaryTaskRequest> {
    let fields = parse_tlv(payload)?;
    let redirect = fields
        .get(&TLV_REDIRECT)
        .map(|v| String::from_utf8_lossy(v).to_string())
        .unwrap_or_default();
    Ok(BinaryTaskRequest { redirect })
}

pub fn decode_verify_request(payload: &[u8]) -> anyhow::Result<BinaryVerifyRequest> {
    let fields = parse_tlv(payload)?;
    let task_id = fields
        .get(&TLV_TASK_ID)
        .map(|v| String::from_utf8_lossy(v).to_string())
        .unwrap_or_default();
    let nonce = fields
        .get(&TLV_NONCE)
        .map(|v| String::from_utf8_lossy(v).to_string())
        .unwrap_or_default();
    let redirect = fields
        .get(&TLV_REDIRECT)
        .map(|v| String::from_utf8_lossy(v).to_string())
        .unwrap_or_default();
    if task_id.is_empty() || nonce.is_empty() {
        anyhow::bail!("missing fields");
    }
    Ok(BinaryVerifyRequest {
        task_id,
        nonce,
        redirect,
    })
}

pub fn encode_verify_response(resp: BinaryVerifyResponse) -> Vec<u8> {
    append_tlv(Vec::new(), TLV_REDIRECT, resp.redirect.as_bytes())
}

pub fn encode_error_frame(message: &str) -> Vec<u8> {
    let payload = append_tlv(Vec::new(), TLV_ERROR, message.as_bytes());
    encode_frame(FRAME_TYPE_ERROR, payload)
}

pub fn encode_task_response_frame(
    task: &Task,
    workers: i32,
    worker_type: &str,
) -> anyhow::Result<Vec<u8>> {
    let resp = BinaryTaskResponse {
        task_id: task.task_id.0.to_string(),
        seed: task.seed.0.clone(),
        bits: task.bits as i32,
        exp: task.exp,
        scope: task.scope.0.clone(),
        ua_hash: task.ua_hash.0.clone(),
        ip_hash: task.ip_hash.0.clone(),
        workers,
        worker_type: worker_type.to_string(),
    };
    let payload = encode_task_response(resp);
    let mut frame = encode_frame(FRAME_TYPE_TASK_RESPONSE, payload);
    deobfuscate_frame(&mut frame, XOR_KEY);
    Ok(frame)
}

fn append_tlv(mut buf: Vec<u8>, t: u8, v: &[u8]) -> Vec<u8> {
    if v.len() > u16::MAX as usize {
        return buf;
    }
    buf.push(t);
    buf.extend_from_slice(&(v.len() as u16).to_be_bytes());
    buf.extend_from_slice(v);
    buf
}

fn parse_tlv<'a>(payload: &'a [u8]) -> anyhow::Result<HashMap<u8, &'a [u8]>> {
    let mut fields = HashMap::new();
    let mut idx = 0usize;
    while idx < payload.len() {
        if payload.len() - idx < 3 {
            anyhow::bail!("invalid tlv header");
        }
        let t = payload[idx];
        let len = u16::from_be_bytes([payload[idx + 1], payload[idx + 2]]) as usize;
        idx += 3;
        if payload.len() - idx < len {
            anyhow::bail!("invalid tlv length");
        }
        fields.insert(t, &payload[idx..idx + len]);
        idx += len;
    }
    Ok(fields)
}
