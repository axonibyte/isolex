//! The built module itself: it imports nothing, so a host grants it
//! nothing, and it exports exactly what `src/lib.rs` documents.
//!
//! It reads the module CI built, named by `ISOLEX_WASM`, so it is ignored
//! in a plain `cargo test`; CI builds the module and runs it with
//! `--ignored`.

/// A LEB128 unsigned integer at `*at`, which moves past it.
fn leb(bytes: &[u8], at: &mut usize) -> u32 {
    let (mut value, mut shift) = (0u32, 0);
    loop {
        let b = bytes[*at];
        *at += 1;
        value |= u32::from(b & 0x7f) << shift;
        if b & 0x80 == 0 {
            return value;
        }
        shift += 7;
    }
}

fn name(bytes: &[u8], at: &mut usize) -> String {
    let n = leb(bytes, at) as usize;
    let s = String::from_utf8(bytes[*at..*at + n].to_vec()).unwrap();
    *at += n;
    s
}

#[test]
#[ignore = "needs the built module: ISOLEX_WASM=<path> cargo test -p isolex-wasm -- --ignored"]
fn imports_nothing_and_exports_the_documented_abi() {
    let path = std::env::var("ISOLEX_WASM").expect("ISOLEX_WASM names the built module");
    let bytes = std::fs::read(&path).unwrap();
    assert_eq!(
        &bytes[..8],
        b"\0asm\x01\0\0\0",
        "{path} is not a wasm 1.0 module"
    );

    let (mut imports, mut exports) = (0, Vec::new());
    let mut at = 8;
    while at < bytes.len() {
        let id = bytes[at];
        at += 1;
        let size = leb(&bytes, &mut at) as usize;
        let end = at + size;
        let mut p = at;
        match id {
            2 => imports = leb(&bytes, &mut p),
            7 => {
                for _ in 0..leb(&bytes, &mut p) {
                    let n = name(&bytes, &mut p);
                    let kind = bytes[p];
                    p += 1;
                    leb(&bytes, &mut p);
                    exports.push((n, kind));
                }
            }
            _ => {}
        }
        at = end;
    }

    assert_eq!(imports, 0, "the module imports something");
    exports.sort();
    let want: Vec<(String, u8)> = [
        ("isolex_alloc", 0),
        ("isolex_compile", 0),
        ("isolex_free", 0),
        ("isolex_version", 0),
        ("memory", 2),
    ]
    .iter()
    .map(|(n, k)| (n.to_string(), *k))
    .collect();
    assert_eq!(exports, want, "exports (name, 0 = function, 2 = memory)");
}
