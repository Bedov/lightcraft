//! Nikon's observed component geometry, using the MIT OxideAV lifting kernel.
use oxideav_jpegxs::{Error, Result, colour_transform};

fn invalid(message: &str) -> Error {
    Error::InvalidData(message.into())
}

/// Empirical same-exposure reference calibration, not a Nikon specification.
/// Positive branch fitted on MST_3508; independently checked on MST_4562/4722.
/// All 73,495,680 sensor samples differ by at most 2 DN from reference, with
/// More than 99.996% within 1 DN. The negative branch preserves samples below black.
/// Only used with the verified PIH extension and 1008-DN black level.
pub fn linear_sample(value: i32) -> u16 {
    let t = f64::from(value) + 16_512.881_889_384_47;
    let squared = t * t;
    let output = if t >= 0.0 { 1_008.136_489_489_676_9 + squared / 157_948.145_899_102 } else { 1_008.136_489_489_676_9 - squared / 262_144.0 };
    output.round().clamp(0.0, 16_383.0) as u16
}

/// Return the signed companded RGGB mosaic, before Nikon output mapping.
pub fn companded_bayer(w: usize, h: usize, planes: &[Vec<i32>]) -> Result<Vec<i32>> {
    if (w, h) != (3032, 2020) || planes.len() != 4 {
        return Err(invalid("unverified Nikon component dimensions"));
    }
    let n = w.checked_mul(h).ok_or_else(|| invalid("component size overflow"))?;
    let mut transposed = Vec::with_capacity(4);
    // Nikon order: Y (G1 position), Cr, green difference (G2), Cb.
    // The standard kernel uses Y (G2 position), Cb, Cr, green difference.
    // Swapping the spatial axes and then the two green outputs reconciles them.
    for c in [0usize, 3, 1, 2] {
        let source = planes.get(c).filter(|p| p.len() == n).ok_or_else(|| invalid("component size mismatch"))?;
        let mut target = vec![0; n];
        for (y, row) in source.chunks_exact(w).enumerate() {
            for (x, value) in row.iter().enumerate() {
                *target.get_mut(x * h + y).ok_or_else(|| invalid("transpose bounds"))? = *value;
            }
        }
        transposed.push(target);
    }
    let mut refs: Vec<&mut [i32]> = transposed.iter_mut().map(Vec::as_mut_slice).collect();
    // Parameters established by comparing the same-exposure reference pixels.
    colour_transform::inverse_star_tetrix(&mut refs, h, w, 0, 0, 0, 0)?;
    let size = n.checked_mul(4).ok_or_else(|| invalid("mosaic size overflow"))?;
    let mut mosaic = vec![0; size];
    for y in 0..h {
        for x in 0..w {
            for (slot, c) in [0usize, 2, 1, 3].iter().enumerate() {
                let value = transposed.get(*c).and_then(|p| p.get(x * h + y)).ok_or_else(|| invalid("sample bounds"))?;
                let at = (2 * y + slot / 2) * (2 * w) + 2 * x + slot % 2;
                *mosaic.get_mut(at).ok_or_else(|| invalid("mosaic bounds"))? = *value;
            }
        }
    }
    Ok(mosaic)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mapping_is_monotone_and_preserves_below_black() {
        assert!(linear_sample(-18_162) < 1008);
        assert_eq!(linear_sample(-16_513), 1008);
        assert_eq!(linear_sample(i32::MIN), 0);
        assert_eq!(linear_sample(i32::MAX), 16_383);
        for v in -40_000..50_000 {
            assert!(linear_sample(v) <= linear_sample(v + 1));
        }
    }

    #[test]
    fn component_geometry_is_checked_before_allocation() {
        assert!(companded_bayer(0, 0, &[]).is_err());
        assert!(companded_bayer(3032, 2020, &[vec![], vec![], vec![], vec![]]).is_err());
    }
}
