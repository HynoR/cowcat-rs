use std::alloc::{alloc as global_alloc, dealloc as global_dealloc, Layout};
use std::slice;

#[no_mangle]
pub extern "C" fn alloc(len: usize) -> *mut u8 {
	if len == 0 {
		return std::ptr::null_mut();
	}
	let layout = match Layout::from_size_align(len, 1) {
		Ok(l) => l,
		Err(_) => return std::ptr::null_mut(),
	};
	unsafe { global_alloc(layout) }
}

#[no_mangle]
pub extern "C" fn dealloc(ptr: *mut u8, len: usize) {
	if ptr.is_null() || len == 0 {
		return;
	}
	let layout = match Layout::from_size_align(len, 1) {
		Ok(l) => l,
		Err(_) => return,
	};
	unsafe {
		global_dealloc(ptr, layout);
	}
}

#[no_mangle]
pub extern "C" fn pow_search(
	prefix_ptr: *const u8,
	prefix_len: usize,
	bits: u32,
	start: u32,
	step: u32,
	max_iters: u32,
) -> u32 {
	if step == 0 {
		return u32::MAX;
	}

	let prefix = unsafe { std::slice::from_raw_parts(prefix_ptr, prefix_len) };
	let mut nonce = start;
	let mut iter = 0u32;

	loop {
		if max_iters != 0 && iter >= max_iters {
			return u32::MAX;
		}

		let mut hasher = Sha256::new();
		hasher.update(prefix);

		let mut nonce_buf = [0u8; 10];
		let nonce_bytes = write_u32_decimal(nonce, &mut nonce_buf);
		hasher.update(nonce_bytes);

		let hash = hasher.finalize();
		if has_leading_zero_bits(&hash, bits) {
			return nonce;
		}

		nonce = nonce.wrapping_add(step);
		iter = iter.wrapping_add(1);
	}
}

fn write_u32_decimal(mut n: u32, out: &mut [u8; 10]) -> &[u8] {
	let mut i = out.len();
	if n == 0 {
		i -= 1;
		out[i] = b'0';
		return &out[i..];
	}
	while n > 0 {
		let digit = (n % 10) as u8;
		i -= 1;
		out[i] = b'0' + digit;
		n /= 10;
	}
	&out[i..]
}

fn has_leading_zero_bits(hash: &[u8; 32], bits: u32) -> bool {
	if bits == 0 {
		return true;
	}

	let mut remaining = bits;
	for &b in hash.iter() {
		if remaining == 0 {
			return true;
		}
		let lz = b.leading_zeros() as u32;
		if lz >= remaining {
			return true;
		}
		if lz != 8 {
			return false;
		}
		remaining -= 8;
	}

	remaining == 0
}

const FRAME_MAGIC0: u8 = b'C';
const FRAME_MAGIC1: u8 = b'W';
const FRAME_VERSION: u8 = 1;

const FRAME_TASK_REQUEST: u8 = 1;
const FRAME_TASK_RESPONSE: u8 = 2;
const FRAME_VERIFY_REQUEST: u8 = 3;
const FRAME_VERIFY_RESPONSE: u8 = 4;
const FRAME_ERROR: u8 = 5;

const TLV_REDIRECT: u8 = 0x01;
const TLV_TASK_ID: u8 = 0x02;
const TLV_SEED: u8 = 0x03;
const TLV_EXP: u8 = 0x04;
const TLV_BITS: u8 = 0x05;
const TLV_SCOPE: u8 = 0x06;
const TLV_UA_HASH: u8 = 0x07;
const TLV_IP_HASH: u8 = 0x08;
const TLV_WORKERS: u8 = 0x09;
const TLV_NONCE: u8 = 0x0a;
const TLV_WORKER_TYPE: u8 = 0x0b;
const TLV_ERROR: u8 = 0x0f;

// XOR 混淆密钥（用于 verify request）
const XOR_KEY: &[u8] = b"cowcatwaflibwafcatcow";

#[no_mangle]
pub extern "C" fn encode_task_request(
	redirect_ptr: *const u8,
	redirect_len: usize,
	out_len_ptr: *mut u32,
) -> *mut u8 {
	let redirect = unsafe { slice::from_raw_parts(redirect_ptr, redirect_len) };
	let mut payload = Vec::new();
	append_tlv(&mut payload, TLV_REDIRECT, redirect);
	let frame = build_frame(FRAME_TASK_REQUEST, &payload);
	write_output(frame, out_len_ptr)
}

#[no_mangle]
pub extern "C" fn encode_verify_request(
	task_id_ptr: *const u8,
	task_id_len: usize,
	nonce_ptr: *const u8,
	nonce_len: usize,
	redirect_ptr: *const u8,
	redirect_len: usize,
	out_len_ptr: *mut u32,
) -> *mut u8 {
	let task_id = unsafe { slice::from_raw_parts(task_id_ptr, task_id_len) };
	let nonce = unsafe { slice::from_raw_parts(nonce_ptr, nonce_len) };
	let redirect = unsafe { slice::from_raw_parts(redirect_ptr, redirect_len) };
	let mut payload = Vec::new();
	append_tlv(&mut payload, TLV_TASK_ID, task_id);
	append_tlv(&mut payload, TLV_NONCE, nonce);
	append_tlv(&mut payload, TLV_REDIRECT, redirect);
	let mut frame = build_frame(FRAME_VERIFY_REQUEST, &payload);
	obfuscate_frame(&mut frame);
	write_output(frame, out_len_ptr)
}

#[no_mangle]
pub extern "C" fn decode_task_response(
	frame_ptr: *const u8,
	frame_len: usize,
	out_len_ptr: *mut u32,
) -> *mut u8 {
	let frame = unsafe { slice::from_raw_parts(frame_ptr, frame_len) };
	// 对 task response 进行解混淆
	let mut deobfuscated = frame.to_vec();
	obfuscate_frame(&mut deobfuscated);
	let json = decode_task_response_json(&deobfuscated);
	write_output(json.into_bytes(), out_len_ptr)
}

#[no_mangle]
pub extern "C" fn decode_verify_response(
	frame_ptr: *const u8,
	frame_len: usize,
	out_len_ptr: *mut u32,
) -> *mut u8 {
	let frame = unsafe { slice::from_raw_parts(frame_ptr, frame_len) };
	let json = decode_verify_response_json(frame);
	write_output(json.into_bytes(), out_len_ptr)
}

fn build_frame(frame_type: u8, payload: &[u8]) -> Vec<u8> {
	let mut frame = Vec::with_capacity(8 + payload.len());
	frame.push(FRAME_MAGIC0);
	frame.push(FRAME_MAGIC1);
	frame.push(FRAME_VERSION);
	frame.push(frame_type);
	frame.extend_from_slice(&(payload.len() as u32).to_be_bytes());
	frame.extend_from_slice(payload);
	frame
}

fn obfuscate_frame(frame: &mut [u8]) {
	let key_len = XOR_KEY.len();
	for (i, byte) in frame.iter_mut().enumerate() {
		*byte ^= XOR_KEY[i % key_len];
	}
}

fn parse_frame(data: &[u8]) -> Result<(u8, &[u8]), &'static str> {
	if data.len() < 8 {
		return Err("frame too short");
	}
	if data[0] != FRAME_MAGIC0 || data[1] != FRAME_MAGIC1 {
		return Err("bad magic");
	}
	if data[2] != FRAME_VERSION {
		return Err("bad version");
	}
	let frame_type = data[3];
	let len = u32::from_be_bytes([data[4], data[5], data[6], data[7]]) as usize;
	if data.len() - 8 != len {
		return Err("length mismatch");
	}
	Ok((frame_type, &data[8..]))
}

fn append_tlv(buf: &mut Vec<u8>, t: u8, v: &[u8]) {
	if v.len() > u16::MAX as usize {
		return;
	}
	buf.push(t);
	buf.extend_from_slice(&(v.len() as u16).to_be_bytes());
	buf.extend_from_slice(v);
}

fn parse_tlv(payload: &[u8]) -> Result<Vec<Option<Vec<u8>>>, &'static str> {
	let mut fields = vec![None; 256];
	let mut i = 0usize;
	while i < payload.len() {
		if payload.len() - i < 3 {
			return Err("invalid tlv header");
		}
		let t = payload[i] as usize;
		let len = u16::from_be_bytes([payload[i + 1], payload[i + 2]]) as usize;
		i += 3;
		if payload.len() - i < len {
			return Err("invalid tlv length");
		}
		fields[t] = Some(payload[i..i + len].to_vec());
		i += len;
	}
	Ok(fields)
}

fn decode_task_response_json(frame: &[u8]) -> String {
	let (frame_type, payload) = match parse_frame(frame) {
		Ok(v) => v,
		Err(e) => return error_json(e),
	};

	if frame_type == FRAME_ERROR {
		return decode_error_json(payload);
	}
	if frame_type != FRAME_TASK_RESPONSE {
		return error_json("unexpected frame type");
	}

	let fields = match parse_tlv(payload) {
		Ok(v) => v,
		Err(e) => return error_json(e),
	};

	let task_id = match field_string(&fields, TLV_TASK_ID) {
		Some(v) => v,
		None => return error_json("missing task_id"),
	};
	let seed = match field_string(&fields, TLV_SEED) {
		Some(v) => v,
		None => return error_json("missing seed"),
	};
	let scope = match field_string(&fields, TLV_SCOPE) {
		Some(v) => v,
		None => return error_json("missing scope"),
	};
	let ua_hash = match field_string(&fields, TLV_UA_HASH) {
		Some(v) => v,
		None => return error_json("missing ua_hash"),
	};
	let ip_hash = field_string(&fields, TLV_IP_HASH).unwrap_or_default();
	let exp = match field_i64(&fields, TLV_EXP) {
		Some(v) => v,
		None => return error_json("missing exp"),
	};
	let bits = match field_u16(&fields, TLV_BITS) {
		Some(v) => v,
		None => return error_json("missing bits"),
	};
	let workers = match field_u8(&fields, TLV_WORKERS) {
		Some(v) => v,
		None => return error_json("missing workers"),
	};
	let worker_type = field_string(&fields, TLV_WORKER_TYPE).unwrap_or_else(|| "wasm".to_string());

	format!(
		"{{\"task_id\":\"{}\",\"seed\":\"{}\",\"bits\":{},\"exp\":{},\"scope\":\"{}\",\"ua_hash\":\"{}\",\"ip_hash\":\"{}\",\"workers_n\":{},\"worker_type\":\"{}\"}}",
		json_escape(&task_id),
		json_escape(&seed),
		bits,
		exp,
		json_escape(&scope),
		json_escape(&ua_hash),
		json_escape(&ip_hash),
		workers,
		json_escape(&worker_type)
	)
}

fn decode_verify_response_json(frame: &[u8]) -> String {
	let (frame_type, payload) = match parse_frame(frame) {
		Ok(v) => v,
		Err(e) => return error_json(e),
	};

	if frame_type == FRAME_ERROR {
		return decode_error_json(payload);
	}
	if frame_type != FRAME_VERIFY_RESPONSE {
		return error_json("unexpected frame type");
	}

	let fields = match parse_tlv(payload) {
		Ok(v) => v,
		Err(e) => return error_json(e),
	};
	let redirect = match field_string(&fields, TLV_REDIRECT) {
		Some(v) => v,
		None => return error_json("missing redirect"),
	};

	format!("{{\"redirect\":\"{}\"}}", json_escape(&redirect))
}

fn decode_error_json(payload: &[u8]) -> String {
	let fields = match parse_tlv(payload) {
		Ok(v) => v,
		Err(_) => return error_json("invalid error"),
	};
	let message = field_string(&fields, TLV_ERROR).unwrap_or_else(|| "error".to_string());
	format!("{{\"error\":\"{}\"}}", json_escape(&message))
}

fn field_string(fields: &[Option<Vec<u8>>], t: u8) -> Option<String> {
	let value = fields.get(t as usize)?.as_ref()?;
	String::from_utf8(value.clone()).ok()
}

fn field_u16(fields: &[Option<Vec<u8>>], t: u8) -> Option<u16> {
	let value = fields.get(t as usize)?.as_ref()?;
	if value.len() != 2 {
		return None;
	}
	Some(u16::from_be_bytes([value[0], value[1]]))
}

fn field_i64(fields: &[Option<Vec<u8>>], t: u8) -> Option<i64> {
	let value = fields.get(t as usize)?.as_ref()?;
	if value.len() != 8 {
		return None;
	}
	let mut buf = [0u8; 8];
	buf.copy_from_slice(value);
	Some(i64::from_be_bytes(buf))
}

fn field_u8(fields: &[Option<Vec<u8>>], t: u8) -> Option<u8> {
	let value = fields.get(t as usize)?.as_ref()?;
	if value.len() != 1 {
		return None;
	}
	Some(value[0])
}

fn json_escape(s: &str) -> String {
	let mut out = String::with_capacity(s.len() + 4);
	for ch in s.chars() {
		match ch {
			'"' => out.push_str("\\\""),
			'\\' => out.push_str("\\\\"),
			'\n' => out.push_str("\\n"),
			'\r' => out.push_str("\\r"),
			'\t' => out.push_str("\\t"),
			c if c.is_control() => {
				let code = c as u32;
				out.push_str(&format!("\\u{:04x}", code));
			}
			_ => out.push(ch),
		}
	}
	out
}

fn error_json(message: &str) -> String {
	format!("{{\"error\":\"{}\"}}", json_escape(message))
}

fn write_output(buf: Vec<u8>, out_len_ptr: *mut u32) -> *mut u8 {
	let len = buf.len();
	if len == 0 {
		unsafe {
			if !out_len_ptr.is_null() {
				*out_len_ptr = 0;
			}
		}
		return std::ptr::null_mut();
	}
	let out_ptr = alloc(len);
	if out_ptr.is_null() {
		unsafe {
			if !out_len_ptr.is_null() {
				*out_len_ptr = 0;
			}
		}
		return std::ptr::null_mut();
	}
	unsafe {
		std::ptr::copy_nonoverlapping(buf.as_ptr(), out_ptr, len);
		if !out_len_ptr.is_null() {
			*out_len_ptr = len as u32;
		}
	}
	out_ptr
}

struct Sha256 {
	state: [u32; 8],
	buf: [u8; 64],
	buf_len: usize,
	total_len: u64,
}

impl Sha256 {
	fn new() -> Self {
		Self {
			state: [
				0x6a09e667,
				0xbb67ae85,
				0x3c6ef372,
				0xa54ff53a,
				0x510e527f,
				0x9b05688c,
				0x1f83d9ab,
				0x5be0cd19,
			],
			buf: [0u8; 64],
			buf_len: 0,
			total_len: 0,
		}
	}

	fn update(&mut self, mut data: &[u8]) {
		self.total_len += data.len() as u64;

		if self.buf_len > 0 {
			let need = 64 - self.buf_len;
			if data.len() < need {
				self.buf[self.buf_len..self.buf_len + data.len()].copy_from_slice(data);
				self.buf_len += data.len();
				return;
			}

			self.buf[self.buf_len..].copy_from_slice(&data[..need]);
			let block = self.buf;
			self.compress(&block);
			self.buf_len = 0;
			data = &data[need..];
		}

		while data.len() >= 64 {
			let (block, rest) = data.split_at(64);
			self.compress(block.try_into().unwrap());
			data = rest;
		}

		if !data.is_empty() {
			self.buf[..data.len()].copy_from_slice(data);
			self.buf_len = data.len();
		}
	}

	fn finalize(mut self) -> [u8; 32] {
		let bit_len = self.total_len * 8;

		self.buf[self.buf_len] = 0x80;
		self.buf_len += 1;

		if self.buf_len > 56 {
			for b in self.buf[self.buf_len..].iter_mut() {
				*b = 0;
			}
			let block = self.buf;
			self.compress(&block);
			self.buf_len = 0;
		}

		for b in self.buf[self.buf_len..56].iter_mut() {
			*b = 0;
		}

		self.buf[56..64].copy_from_slice(&bit_len.to_be_bytes());
		let block = self.buf;
		self.compress(&block);

		let mut out = [0u8; 32];
		for (i, word) in self.state.iter().enumerate() {
			out[i * 4..i * 4 + 4].copy_from_slice(&word.to_be_bytes());
		}
		out
	}

	fn compress(&mut self, block: &[u8; 64]) {
		let mut w = [0u32; 64];
		for (i, chunk) in block.chunks_exact(4).take(16).enumerate() {
			w[i] = u32::from_be_bytes(chunk.try_into().unwrap());
		}
		for i in 16..64 {
			let s0 = w[i - 15].rotate_right(7) ^ w[i - 15].rotate_right(18) ^ (w[i - 15] >> 3);
			let s1 = w[i - 2].rotate_right(17) ^ w[i - 2].rotate_right(19) ^ (w[i - 2] >> 10);
			w[i] = w[i - 16]
				.wrapping_add(s0)
				.wrapping_add(w[i - 7])
				.wrapping_add(s1);
		}

		let mut a = self.state[0];
		let mut b = self.state[1];
		let mut c = self.state[2];
		let mut d = self.state[3];
		let mut e = self.state[4];
		let mut f = self.state[5];
		let mut g = self.state[6];
		let mut h = self.state[7];

		for i in 0..64 {
			let s1 = e.rotate_right(6) ^ e.rotate_right(11) ^ e.rotate_right(25);
			let ch = (e & f) ^ ((!e) & g);
			let temp1 = h
				.wrapping_add(s1)
				.wrapping_add(ch)
				.wrapping_add(K[i])
				.wrapping_add(w[i]);
			let s0 = a.rotate_right(2) ^ a.rotate_right(13) ^ a.rotate_right(22);
			let maj = (a & b) ^ (a & c) ^ (b & c);
			let temp2 = s0.wrapping_add(maj);

			h = g;
			g = f;
			f = e;
			e = d.wrapping_add(temp1);
			d = c;
			c = b;
			b = a;
			a = temp1.wrapping_add(temp2);
		}

		self.state[0] = self.state[0].wrapping_add(a);
		self.state[1] = self.state[1].wrapping_add(b);
		self.state[2] = self.state[2].wrapping_add(c);
		self.state[3] = self.state[3].wrapping_add(d);
		self.state[4] = self.state[4].wrapping_add(e);
		self.state[5] = self.state[5].wrapping_add(f);
		self.state[6] = self.state[6].wrapping_add(g);
		self.state[7] = self.state[7].wrapping_add(h);
	}
}

const K: [u32; 64] = [
	0x428a2f98,
	0x71374491,
	0xb5c0fbcf,
	0xe9b5dba5,
	0x3956c25b,
	0x59f111f1,
	0x923f82a4,
	0xab1c5ed5,
	0xd807aa98,
	0x12835b01,
	0x243185be,
	0x550c7dc3,
	0x72be5d74,
	0x80deb1fe,
	0x9bdc06a7,
	0xc19bf174,
	0xe49b69c1,
	0xefbe4786,
	0x0fc19dc6,
	0x240ca1cc,
	0x2de92c6f,
	0x4a7484aa,
	0x5cb0a9dc,
	0x76f988da,
	0x983e5152,
	0xa831c66d,
	0xb00327c8,
	0xbf597fc7,
	0xc6e00bf3,
	0xd5a79147,
	0x06ca6351,
	0x14292967,
	0x27b70a85,
	0x2e1b2138,
	0x4d2c6dfc,
	0x53380d13,
	0x650a7354,
	0x766a0abb,
	0x81c2c92e,
	0x92722c85,
	0xa2bfe8a1,
	0xa81a664b,
	0xc24b8b70,
	0xc76c51a3,
	0xd192e819,
	0xd6990624,
	0xf40e3585,
	0x106aa070,
	0x19a4c116,
	0x1e376c08,
	0x2748774c,
	0x34b0bcb5,
	0x391c0cb3,
	0x4ed8aa4a,
	0x5b9cca4f,
	0x682e6ff3,
	0x748f82ee,
	0x78a5636f,
	0x84c87814,
	0x8cc70208,
	0x90befffa,
	0xa4506ceb,
	0xbef9a3f7,
	0xc67178f2,
];
