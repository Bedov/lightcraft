//! Bedov fork: Sony crop tags (`0x74c7/0x74c8`) that describe an APS-C window on a full-frame image.
//!
//! A genuine APS-C / Super 35 ARW stores only that part of the sensor: on the ILCE-7M4 the raw IFD is 4736 × 3132,
//! and Sony's crop tags agree with the DNG-style default crop (44, 30, 4608 × 3072). Its Exif `FocalLengthIn35mm` is
//! about 1.5 × `FocalLength`.
//!
//! Some full-frame ILCE-7M4 files (firmware 6.01; 12 of 276 full-frame ARWs from one body, all
//! with a Sigma 24-70 DG DN Art) store the whole 7040 × 4688 sensor and a default crop of
//! (12, 8, 7008 × 4672), but Sony's crop tags say (1212, 808, 4608 × 3072): the centred APS-C
//! window. Exif `PixelXDimension`, the maker note's `FullImageSize` and the embedded preview's
//! size say 4608 × 3072 too, yet that preview shows the full field, downscaled. The camera JPEG
//! shows the full field, and both files give `FocalLengthIn35mm` equal to `FocalLength` (36 for
//! 36.3 mm). Apple ImageIO decodes 7008 × 4672.
//!
//! Such a crop drops about half of the frame's area. So when Sony's crop is a centred APS-C-scale
//! window inside the default crop, and the Exif focal lengths say the field is full frame, the
//! image uses the default crop instead. Every other file keeps Sony's crop tags as upstream reads
//! them: in-camera aspect ratios crop one direction only, and an off-centre window or a missing
//! default crop or focal length keeps them too.

use crate::Rect;
use lightcraft_tiff::Ifd;
use lightcraft_tiff::tags as t;
use std::ops::RangeInclusive;

/// How much larger the full frame is than Sony's APS-C / Super 35 window, in each direction: 1.52–1.53 on the
/// ILCE-7M4, ILCE-7RM5 and ILCE-7RM6.
const APSC_SCALE: RangeInclusive<f64> = 1.4..=1.65;
/// Largest `FocalLengthIn35mm / FocalLength` of a full-frame field. The camera rounds down (36 for 36.3 mm); APS-C mode
/// records about 1.5.
const FULL_FRAME_FIELD: f64 = 1.2;
/// Allowed difference between the two centres, in pixels, measured on twice the centre (even offsets keep the CFA phase).
const CENTRE_TOLERANCE: usize = 4;

/// The image crop for Sony's crop tags `sony` (clipped to the frame), the raw IFD's DNG-style default crop `default`
/// and the Exif IFD `exif`: `default` when `sony` is a stale APS-C window on a full-frame image, else `sony`.
pub(super) fn image_crop(sony: Rect, default: Option<Rect>, exif: Option<&Ifd>) -> Rect {
    match default {
        Some(frame) if full_frame_field(exif) && is_centred_apsc_window(sony, frame) => frame,
        _ => sony,
    }
}

/// Whether the Exif focal lengths describe a full-frame field: both present, positive and finite, the 35 mm
/// equivalent less than [`FULL_FRAME_FIELD`] times the actual focal length.
fn full_frame_field(exif: Option<&Ifd>) -> bool {
    let Some(exif) = exif else { return false };
    let (Some(focal), Some(equivalent)) = (exif.f64(t::FOCAL_LENGTH), exif.f64(t::FOCAL_LENGTH_35MM)) else { return false };
    focal.is_finite() && equivalent.is_finite() && focal > 0.0 && equivalent > 0.0 && equivalent / focal < FULL_FRAME_FIELD
}

/// Whether `window` lies inside `frame`, smaller by an APS-C factor in both directions and centred on it.
fn is_centred_apsc_window(window: Rect, frame: Rect) -> bool {
    if window.width == 0 || window.height == 0 {
        return false;
    }
    let inside =
        |start: usize, len: usize, outer: usize, outer_len: usize| start >= outer && start.saturating_add(len) <= outer.saturating_add(outer_len);
    let scaled = |outer_len: usize, len: usize| APSC_SCALE.contains(&(outer_len as f64 / len as f64));
    let centred = |start: usize, len: usize, outer: usize, outer_len: usize| {
        start.saturating_mul(2).saturating_add(len).abs_diff(outer.saturating_mul(2).saturating_add(outer_len)) <= CENTRE_TOLERANCE
    };
    inside(window.x, window.width, frame.x, frame.width)
        && inside(window.y, window.height, frame.y, frame.height)
        && scaled(frame.width, window.width)
        && scaled(frame.height, window.height)
        && centred(window.x, window.width, frame.x, frame.width)
        && centred(window.y, window.height, frame.y, frame.height)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Mode;
    use lightcraft_tiff::{IfdBuilder, ImageData, TiffWriter, Value};

    /// The ILCE-7M4's geometry at 1/100 scale (rounded to even): a 72 × 48 sensor, the default crop (2, 2, 66 × 44)
    /// and the centred APS-C window (13, 9, 44 × 30), 1.5 × smaller in each direction.
    const SIZE: (u32, u32) = (72, 48);
    const FRAME: [u32; 4] = [2, 2, 66, 44];
    const APSC: [u32; 4] = [13, 9, 44, 30];

    /// A small uncompressed Sony CFA file with Sony's crop tags `sony`, the DNG-style default crop `default` and, when
    /// `focal` is set, an Exif IFD with `FocalLength` and `FocalLengthIn35mm`, in the style of `arw_lens`'s fixture.
    struct Arw {
        size: (u32, u32),
        sony: Option<[u32; 4]>,
        default: Option<[u32; 4]>,
        focal: Option<((u32, u32), u16)>,
    }

    impl Arw {
        /// The stale case: full storage and default crop, Sony's crop the APS-C window, 36.3 mm recorded as 36 mm.
        fn stale() -> Arw {
            Arw { size: SIZE, sony: Some(APSC), default: Some(FRAME), focal: Some(((363, 10), 36)) }
        }

        fn build(&self) -> Vec<u8> {
            let (w, h) = self.size;
            let mut raw = IfdBuilder::new();
            raw.set(t::MAKE, Value::Ascii("SONY".into()));
            raw.set(t::MODEL, Value::Ascii("ILCE-7M4".into()));
            raw.set(t::IMAGE_WIDTH, Value::Long(vec![w]));
            raw.set(t::IMAGE_LENGTH, Value::Long(vec![h]));
            raw.set(t::BITS_PER_SAMPLE, Value::Short(vec![16]));
            raw.set(t::SAMPLES_PER_PIXEL, Value::Short(vec![1]));
            raw.set(t::PHOTOMETRIC, Value::Short(vec![t::photometric::CFA]));
            raw.set(t::COMPRESSION, Value::Short(vec![1]));
            raw.set(t::WHITE_LEVEL, Value::Long(vec![16383]));
            // the distortion-correction setting (off), so the file has a Sony lens record
            raw.set(0x7036, Value::Short(vec![0]));
            if let Some([x, y, cw, ch]) = self.sony {
                raw.set(0x74c7, Value::Long(vec![x, y]));
                raw.set(0x74c8, Value::Long(vec![cw, ch]));
            }
            if let Some([x, y, cw, ch]) = self.default {
                raw.set(t::DEFAULT_CROP_ORIGIN, Value::Long(vec![x, y]));
                raw.set(t::DEFAULT_CROP_SIZE, Value::Long(vec![cw, ch]));
            }
            if let Some((focal, equivalent)) = self.focal {
                let mut exif = IfdBuilder::new();
                exif.set(t::FOCAL_LENGTH, Value::Rational(vec![focal]));
                exif.set(t::FOCAL_LENGTH_35MM, Value::Short(vec![equivalent]));
                raw.set_child(t::EXIF_IFD, exif);
            }
            raw.set_image(ImageData::Strips { rows_per_strip: h, strips: vec![vec![0; (w * h * 2) as usize]] });
            TiffWriter::default().write(&[raw]).unwrap()
        }

        /// The crop of the header probe and of the full decode, which must agree.
        fn crop(&self) -> Rect {
            let file = self.build();
            let header = super::super::arw::decode(&file, Mode::Header).unwrap();
            let full = super::super::arw::decode(&file, Mode::Full).unwrap();
            assert_eq!(header.info(), full.info());
            full.crop
        }
    }

    fn rect([x, y, w, h]: [u32; 4]) -> Rect {
        Rect::new(x as usize, y as usize, w as usize, h as usize)
    }

    #[test]
    fn stale_apsc_crop_tags_on_a_full_frame_image_use_the_default_crop() {
        let file = Arw::stale().build();
        for mode in [Mode::Header, Mode::Full] {
            let img = super::super::arw::decode(&file, mode).unwrap();
            assert_eq!(img.crop, rect(FRAME), "{mode:?}");
            assert_eq!((img.metadata.width, img.metadata.height), (Some(66), Some(44)), "{mode:?}");
            // the lens record keeps the tags as written, and frames its tables on the image crop
            let g = &img.lens_metadata.sony.as_ref().expect("a Sony lens record").geometry;
            assert_eq!((g.sony_crop, g.default_crop, g.image_crop, g.reference), (Some(rect(APSC)), Some(rect(FRAME)), rect(FRAME), rect(FRAME)));
            assert!(g.native_3_2, "{mode:?}");
        }
        // the ILCE-7M4's own numbers, as the decoder sees them
        let (frame, window) = (Rect::new(12, 8, 7008, 4672), Rect::new(1212, 808, 4608, 3072));
        assert!(is_centred_apsc_window(window, frame));
        // 24 mm and 70 mm recorded as themselves are a full-frame field too
        for focal in [((240, 10), 24), ((700, 10), 70)] {
            assert_eq!(Arw { focal: Some(focal), ..Arw::stale() }.crop(), rect(FRAME), "{focal:?}");
        }
    }

    #[test]
    fn genuine_crops_keep_sonys_tags() {
        let cases = [
            // a genuine APS-C file: only the window is stored, and both crops agree
            ("APS-C storage", Arw { size: (48, 32), sony: Some([2, 2, 44, 30]), default: Some([2, 2, 44, 30]), ..Arw::stale() }, [2, 2, 44, 30]),
            // the field is APS-C (35 mm equivalent 1.5 x the focal length): Sony's window stands
            ("APS-C field", Arw { focal: Some(((363, 10), 54)), ..Arw::stale() }, APSC),
            // no evidence about the field: no Exif IFD, a missing, zero or undefined focal length
            ("no Exif", Arw { focal: None, ..Arw::stale() }, APSC),
            ("zero focal length", Arw { focal: Some(((0, 10), 0)), ..Arw::stale() }, APSC),
            ("undefined focal length", Arw { focal: Some(((363, 0), 36)), ..Arw::stale() }, APSC),
            // no default crop to fall back on
            ("no default crop", Arw { default: None, ..Arw::stale() }, APSC),
            // in-camera aspect ratios crop one direction only: 16:9 and 1:1
            ("16:9", Arw { sony: Some([2, 6, 66, 36]), ..Arw::stale() }, [2, 6, 66, 36]),
            ("1:1", Arw { sony: Some([13, 2, 44, 44]), ..Arw::stale() }, [13, 2, 44, 44]),
            // an APS-C-sized window away from the centre, or one partly outside the default crop
            ("off centre", Arw { sony: Some([2, 2, 44, 30]), ..Arw::stale() }, [2, 2, 44, 30]),
            ("outside", Arw { sony: Some([0, 9, 44, 30]), default: Some([4, 2, 64, 44]), ..Arw::stale() }, [0, 9, 44, 30]),
            // much smaller than APS-C, and Sony's crop wider than the default crop (the ILCE-7RM4A's x = 0 versus 32)
            ("small", Arw { sony: Some([24, 16, 24, 16]), ..Arw::stale() }, [24, 16, 24, 16]),
            ("wider", Arw { sony: Some([0, 2, 70, 44]), ..Arw::stale() }, [0, 2, 70, 44]),
            // both crops agree on the full frame (every other full-frame file)
            ("full frame", Arw { sony: Some(FRAME), ..Arw::stale() }, FRAME),
        ];
        for (name, arw, crop) in cases {
            assert_eq!(arw.crop(), rect(crop), "{name}");
        }
    }

    #[test]
    fn hostile_rectangles_and_focal_lengths_are_rejected() {
        let frame = Rect::new(12, 8, 7008, 4672);
        for window in
            [Rect::new(1212, 808, 0, 3072), Rect::new(usize::MAX, usize::MAX, usize::MAX, usize::MAX), Rect::new(usize::MAX - 1, 808, 4608, 3072)]
        {
            assert!(!is_centred_apsc_window(window, frame), "{window:?}");
        }
        assert!(!is_centred_apsc_window(Rect::new(1212, 808, 4608, 3072), Rect::new(usize::MAX, usize::MAX, usize::MAX, usize::MAX)));
        assert!(!full_frame_field(None));
    }
}
