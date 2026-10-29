//! 只写 PNG 编码器（零依赖）：8 位灰度或 8 位 RGB，IDAT 用 zlib 的 stored（不压缩）deflate 块。

use std::io;
use std::path::Path;

const SIGNATURE: [u8; 8] = [0x89, b'P', b'N', b'G', b'\r', b'\n', 0x1A, b'\n'];

/// CRC-32（ISO-HDLC，多项式 0xEDB88320），PNG 块校验用。
pub fn crc32(data: &[u8]) -> u32 {
    crc32_update(0xFFFF_FFFF, data) ^ 0xFFFF_FFFF
}

fn crc32_update(mut crc: u32, data: &[u8]) -> u32 {
    for &b in data {
        crc ^= b as u32;
        for _ in 0..8 {
            let mask = (crc & 1).wrapping_neg();
            crc = (crc >> 1) ^ (0xEDB8_8320 & mask);
        }
    }
    crc
}

/// Adler-32（zlib 尾部校验）。
pub fn adler32(data: &[u8]) -> u32 {
    let (mut a, mut b) = (1u32, 0u32);
    for chunk in data.chunks(5552) {
        for &x in chunk {
            a += x as u32;
            b += a;
        }
        a %= 65521;
        b %= 65521;
    }
    (b << 16) | a
}

/// zlib 流：CMF/FLG + 若干 stored 块 + Adler-32（大端）。
pub fn zlib_stored(data: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(data.len() + data.len() / 65535 * 5 + 16);
    // CMF=0x78（deflate，32K 窗口），FLG=0x01：(0x78*256+0x01) % 31 == 0，FCHECK 合法。
    out.extend_from_slice(&[0x78, 0x01]);
    if data.is_empty() {
        out.extend_from_slice(&[0x01, 0x00, 0x00, 0xFF, 0xFF]);
    } else {
        let mut chunks = data.chunks(65535).peekable();
        while let Some(c) = chunks.next() {
            let last = chunks.peek().is_none();
            out.push(last as u8); // BFINAL 位 + BTYPE=00（stored）
            let len = c.len() as u16;
            out.extend_from_slice(&len.to_le_bytes());
            out.extend_from_slice(&(!len).to_le_bytes());
            out.extend_from_slice(c);
        }
    }
    out.extend_from_slice(&adler32(data).to_be_bytes());
    out
}

fn chunk(out: &mut Vec<u8>, kind: &[u8; 4], data: &[u8]) {
    out.extend_from_slice(&(data.len() as u32).to_be_bytes());
    out.extend_from_slice(kind);
    out.extend_from_slice(data);
    let crc = crc32_update(crc32_update(0xFFFF_FFFF, kind), data) ^ 0xFFFF_FFFF;
    out.extend_from_slice(&crc.to_be_bytes());
}

/// `channels` 为 1（灰度）或 3（RGB）。`px` 行优先，长度必须为 w*h*channels。
fn encode(w: u32, h: u32, channels: usize, px: &[u8]) -> Vec<u8> {
    assert!(channels == 1 || channels == 3, "channels must be 1 or 3");
    let row = w as usize * channels;
    assert_eq!(px.len(), row * h as usize, "pixel buffer size mismatch");
    let mut raw = Vec::with_capacity((row + 1) * h as usize);
    for line in px.chunks(row.max(1)).take(h as usize) {
        raw.push(0); // 过滤类型 0（None）
        raw.extend_from_slice(line);
    }
    let mut ihdr = Vec::with_capacity(13);
    ihdr.extend_from_slice(&w.to_be_bytes());
    ihdr.extend_from_slice(&h.to_be_bytes());
    let color_type = if channels == 1 { 0 } else { 2 };
    ihdr.extend_from_slice(&[8, color_type, 0, 0, 0]);

    let mut out = Vec::new();
    out.extend_from_slice(&SIGNATURE);
    chunk(&mut out, b"IHDR", &ihdr);
    chunk(&mut out, b"IDAT", &zlib_stored(&raw));
    chunk(&mut out, b"IEND", &[]);
    out
}

/// 8 位灰度 PNG。
pub fn encode_gray(w: u32, h: u32, px: &[u8]) -> Vec<u8> {
    encode(w, h, 1, px)
}

/// 8 位 RGB PNG（`px` 为 R,G,B 交错）。
pub fn encode_rgb(w: u32, h: u32, px: &[u8]) -> Vec<u8> {
    encode(w, h, 3, px)
}

fn write_file(path: &Path, data: &[u8]) -> io::Result<()> {
    if let Some(dir) = path.parent() {
        if !dir.as_os_str().is_empty() {
            std::fs::create_dir_all(dir)?;
        }
    }
    std::fs::write(path, data)
}

pub fn write_gray(path: &Path, w: u32, h: u32, px: &[u8]) -> io::Result<()> {
    write_file(path, &encode_gray(w, h, px))
}

pub fn write_rgb(path: &Path, w: u32, h: u32, px: &[u8]) -> io::Result<()> {
    write_file(path, &encode_rgb(w, h, px))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn crc32_check_value() {
        assert_eq!(crc32(b"123456789"), 0xCBF4_3926);
        assert_eq!(crc32(b"IEND"), 0xAE42_6082);
    }

    #[test]
    fn adler32_known() {
        assert_eq!(adler32(b"Wikipedia"), 0x11E6_0398);
        assert_eq!(adler32(b""), 1);
    }

    #[test]
    fn stored_blocks_split_at_65535() {
        let data = vec![7u8; 70000];
        let z = zlib_stored(&data);
        // 2 字节头 + 2 个块头（各 5 字节）+ 数据 + 4 字节 Adler
        assert_eq!(z.len(), 2 + 5 + 5 + 70000 + 4);
        assert_eq!(z[2], 0x00);
        assert_eq!(z[2 + 5 + 65535], 0x01);
    }
}
