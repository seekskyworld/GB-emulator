//! T05b：手写 PNG 能被 Pillow 打开，且像素与原始数据一致。
//! Python 解释器依次尝试 `$GBEMU_PYTHON`、`<workspace>/.venv/bin/python`、`$GBEMU_REFS/../.venv/bin/python`；
//! 都没有 Pillow 时打印 SKIP 并通过。

use std::path::{Path, PathBuf};
use std::process::Command;

use gb_cli::png;
use gb_cli::report::diff_image_rgb;
use gb_core::FRAME_PIXELS;

const CHECK: &str = r#"
import sys
from PIL import Image
png, raw, mode, w, h = sys.argv[1], sys.argv[2], sys.argv[3], int(sys.argv[4]), int(sys.argv[5])
im = Image.open(png)
im.load()
assert im.mode == mode, (im.mode, mode)
assert im.size == (w, h), im.size
want = open(raw, "rb").read()
got = im.tobytes()
assert got == want, "pixel mismatch: %d bytes vs %d" % (len(got), len(want))
print("OK")
"#;

fn python() -> Option<PathBuf> {
    let mut cands: Vec<PathBuf> = Vec::new();
    if let Some(p) = std::env::var_os("GBEMU_PYTHON") {
        cands.push(p.into());
    }
    let ws = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    cands.push(ws.join(".venv/bin/python"));
    if let Some(r) = std::env::var_os("GBEMU_REFS") {
        cands.push(Path::new(&r).join("../.venv/bin/python"));
    }
    cands.into_iter().find(|p| {
        p.exists()
            && Command::new(p)
                .args(["-I", "-c", "import PIL"])
                .output()
                .is_ok_and(|o| o.status.success())
    })
}

fn tmpdir(tag: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("gbcli-png-{tag}-{}", std::process::id()));
    std::fs::create_dir_all(&d).unwrap();
    d
}

#[allow(clippy::too_many_arguments)]
fn check(py: &Path, dir: &Path, name: &str, data: &[u8], mode: &str, w: u32, h: u32, raw: &[u8]) {
    let p = dir.join(format!("{name}.png"));
    let r = dir.join(format!("{name}.raw"));
    std::fs::write(&p, data).unwrap();
    std::fs::write(&r, raw).unwrap();
    let out = Command::new(py)
        .arg("-I")
        .arg("-c")
        .arg(CHECK)
        .arg(&p)
        .arg(&r)
        .arg(mode)
        .arg(w.to_string())
        .arg(h.to_string())
        .output()
        .unwrap();
    assert!(
        out.status.success() && String::from_utf8_lossy(&out.stdout).trim() == "OK",
        "{name}: Pillow rejected PNG\nstdout: {}\nstderr: {}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
}

#[test]
fn pillow_reads_gray_and_rgb() {
    let Some(py) = python() else {
        println!("SKIP png_pillow: no Python with Pillow (set GBEMU_PYTHON)");
        return;
    };
    let dir = tmpdir("ok");

    // 160×144 灰度：DMG 的 4 个灰度值交错
    let gray: Vec<u8> = (0..FRAME_PIXELS).map(|i| [0xFF, 0xAA, 0x55, 0x00][(i / 3 + i / 160) % 4]).collect();
    check(&py, &dir, "gray", &png::encode_gray(160, 144, &gray), "L", 160, 144, &gray);

    // RGB 差异图
    let mut actual = gray.clone();
    actual[12345] ^= 0xFF;
    let rgb = diff_image_rgb(&actual, &gray);
    assert_eq!(&rgb[12345 * 3..12345 * 3 + 3], &[0xFF, 0, 0]);
    check(&py, &dir, "rgb", &png::encode_rgb(160, 144, &rgb), "RGB", 160, 144, &rgb);

    // 跨多个 stored 块（> 65535 字节的原始数据）且宽高不同、不对齐
    let (w, h) = (301u32, 257u32);
    let big: Vec<u8> = (0..(w * h * 3) as usize).map(|i| (i * 31 % 251) as u8).collect();
    check(&py, &dir, "big", &png::encode_rgb(w, h, &big), "RGB", w, h, &big);

    // 1×1
    check(&py, &dir, "tiny", &png::encode_gray(1, 1, &[0x7F]), "L", 1, 1, &[0x7F]);

    // 写文件的封装（含自动建目录）
    let nested = dir.join("a/b/c.png");
    png::write_gray(&nested, 160, 144, &gray).unwrap();
    let bytes = std::fs::read(&nested).unwrap();
    check(&py, &dir, "nested", &bytes, "L", 160, 144, &gray);

    let _ = std::fs::remove_dir_all(&dir);
}
