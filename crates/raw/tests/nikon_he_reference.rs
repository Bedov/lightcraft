//! Private same-exposure reference check. Media stays outside the repository.
use lightcraft_raw::{RawData, Rect, decode, probe_info};
use std::path::PathBuf;

#[test]
fn all_private_z6iii_he_files_decode() {
    let Some(directory) = std::env::var_os("LIGHTCRAFT_NIKON_HE_NEF_DIR") else {
        lightcraft_testkit::skip!(Input, "set Nikon HE NEF directory");
        return;
    };
    let mut files: Vec<_> = std::fs::read_dir(directory)
        .unwrap()
        .map(|p| p.unwrap().path())
        .filter(|p| p.extension().is_some_and(|e| e.eq_ignore_ascii_case("nef")))
        .collect();
    files.sort();
    assert!(!files.is_empty());
    for (i, path) in files.iter().enumerate() {
        let start = std::time::Instant::now();
        let raw = decode(&std::fs::read(path).unwrap()).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
        assert_eq!((raw.width, raw.height, raw.data.len()), (6064, 4040, 24_498_560));
        assert_eq!(raw.crop, Rect::new(8, 4, 6048, 4032));
        eprintln!("NATIVE_PASS {}/{} {} {:.3}s", i + 1, files.len(), path.file_name().unwrap().to_string_lossy(), start.elapsed().as_secs_f64());
    }
}

#[test]
fn z6iii_he_matches_independent_sensor_references() {
    let (Some(nefs), Some(references)) = (std::env::var_os("LIGHTCRAFT_NIKON_HE_NEF_DIR"), std::env::var_os("LIGHTCRAFT_NIKON_HE_REFERENCE_DIR"))
    else {
        lightcraft_testkit::skip!(Input, "set Nikon HE NEF and reference directories");
        return;
    };
    let (nefs, references) = (PathBuf::from(nefs), PathBuf::from(references));
    for stem in ["MST_3508", "MST_4562", "MST_4722"] {
        let bytes = std::fs::read(nefs.join(format!("{stem}.NEF"))).unwrap();
        let raw = decode(&bytes).unwrap();
        let info = probe_info(&bytes).unwrap();
        assert_eq!((raw.width, raw.height, raw.bits), (6064, 4040, 14));
        assert_eq!((info.width, info.height), (raw.width, raw.height));
        assert_eq!(raw.crop, Rect::new(8, 4, 6048, 4032));
        assert_eq!(info.crop, raw.crop);
        assert_eq!(raw.black.mean(), 1008.0);
        assert_eq!(raw.white, vec![15892.0]);
        let RawData::U16(samples) = &raw.data else { panic!("expected integer sensor samples") };
        let reference = std::fs::read(references.join(format!("{stem}.raw"))).unwrap();
        assert_eq!(&reference[..4], &6064u32.to_le_bytes());
        assert_eq!(&reference[4..8], &4040u32.to_le_bytes());
        assert_eq!(reference.len(), samples.len() * 2 + 8);
        let (mut maximum, mut within_one, mut bias, mut below_black) = (0i32, 0usize, 0i64, 0usize);
        for (&value, pair) in samples.iter().zip(reference[8..].chunks_exact(2)) {
            let expected = u16::from_le_bytes([pair[0], pair[1]]);
            let difference = i32::from(value) - i32::from(expected);
            maximum = maximum.max(difference.abs());
            within_one += usize::from(difference.abs() <= 1);
            bias += i64::from(difference);
            below_black += usize::from(value < 1008);
        }
        assert!(maximum <= 2, "{stem}: max error {maximum}");
        assert!(within_one as f64 / samples.len() as f64 >= 0.9999, "{stem}: too many >1-DN differences");
        assert!((bias as f64 / samples.len() as f64).abs() < 0.01, "{stem}: sensor bias");
        if stem == "MST_4722" {
            assert!(below_black > 1000, "must preserve shadow noise below black");
        }
        eprintln!("{stem}: {} sensor samples, max error {maximum} DN, bias {}", samples.len(), bias as f64 / samples.len() as f64);
    }
}

#[test]
fn malformed_private_he_inputs_do_not_panic() {
    let Some(directory) = std::env::var_os("LIGHTCRAFT_NIKON_HE_NEF_DIR") else {
        lightcraft_testkit::skip!(Input, "set Nikon HE NEF directory");
        return;
    };
    let bytes = std::fs::read(PathBuf::from(directory).join("MST_3508.NEF")).unwrap();
    for size in [0, 8, 1024, bytes.len() / 2, bytes.len() - 1] {
        assert!(std::panic::catch_unwind(|| decode(&bytes[..size])).unwrap().is_err());
    }
    let start = bytes.windows(4).position(|b| b == [0xff, 0x10, 0xff, 0x50]).unwrap();
    // Header mutations and packet-boundary corruption must return safely; a
    // valid mutation need not be rejected, but must never panic.
    for offset in [4, 40, 55, 60, 66, 100, 159, 160, 161, 164, 170, 180, 200, 500, 1024] {
        let mut corrupt = bytes.clone();
        corrupt[start + offset] ^= 0xff;
        assert!(std::panic::catch_unwind(|| decode(&corrupt)).is_ok(), "mutation at {offset}");
    }
}
