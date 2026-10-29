//! Nikon Z6 III HE transport adapter, restricted to the measured full-frame profile.
//! Sources: observed Z6 III stream headers and public format descriptions in
//! https://github.com/dnglab/dnglab/pull/835 (prose only).
//! Standard JPEG XS kernels: OxideAV, MIT, pinned in vendor/.
//! Component reconstruction is separate from the calibrated sensor output mapping.
use oxideav_jpegxs::{
    Error, Result, codestream, dequant, dwt,
    entropy::{self, PacketEntry, PacketLayout, PrecinctTop, packet_body::PrecinctState},
    picture_header, slice_walker,
};

fn invalid(message: &str) -> Error {
    Error::InvalidData(message.into())
}

/// Normalize only the container headers for reuse of the standard slice parser.
/// Entropy data is copied byte-for-byte. Original files are never changed.
pub fn normalized_headers(input: &[u8]) -> Result<Vec<u8>> {
    if input.get(..4) != Some(&[0xff, 0x10, 0xff, 0x50]) {
        return Err(invalid("not a Nikon JPEG XS stream"));
    }
    let mut out = vec![0xff, 0x10];
    let mut cursor = 2usize;
    let mut saw_pih = false;
    let mut saw_cdt = false;
    loop {
        let h = input.get(cursor..cursor + 4).ok_or_else(|| invalid("truncated marker"))?;
        let marker = u16::from_be_bytes([h[0], h[1]]);
        let len = u16::from_be_bytes([h[2], h[3]]) as usize;
        if len < 2 {
            return Err(invalid("invalid marker length"));
        }
        let end = cursor.checked_add(len + 2).ok_or_else(|| invalid("marker overflow"))?;
        let body = input.get(cursor + 4..end).ok_or_else(|| invalid("truncated marker body"))?;
        let replacement = match marker {
            0xff50 if cursor == 2 => {
                if !body.starts_with(b"CONTACT_INTOPIX_") {
                    return Err(invalid("unrecognized Nikon HE capabilities signature"));
                }
                vec![]
            }
            0xff12 if !saw_pih => {
                if body.len() != 37 {
                    return Err(invalid("unsupported Nikon PIH length"));
                }
                if body.get(24..) != Some(&[0x50, 0x88, 0x70, 0x83, 0xf0, 0x15, 0x23, 0xd1, 0x49, 0xcd, 0x3f, 0x7f, 7]) {
                    return Err(invalid("unverified Nikon output mapping"));
                }
                let mut p = body.get(..24).ok_or_else(|| invalid("short PIH"))?.to_vec();
                // Nikon's Ppoc=1 extension uses the layout handled below.
                let ppoc = p.get_mut(21).ok_or_else(|| invalid("short PIH"))?;
                if *ppoc != 0x10 {
                    return Err(invalid("unsupported Nikon progression"));
                }
                *ppoc = 0;
                let mut parsed = picture_header::parse(&p)?;
                if parsed.lcod != 9_186_816
                    || parsed.wf != 6064
                    || parsed.hf != 4040
                    || parsed.nc != 4
                    || parsed.nlx != 5
                    || parsed.nly != 1
                    || parsed.cw != 0
                    || parsed.hsl != 16
                    || parsed.bw != 18
                    || parsed.fq != 4
                    || parsed.qpih != 1
                    || parsed.fs != 1
                    || parsed.rm != 0
                    || parsed.ng != 4
                    || parsed.ss != 8
                    || parsed.br != 4
                    || parsed.cpih != 0
                    || parsed.fslc != 0
                    || parsed.rl != 0
                {
                    return Err(invalid("unsupported Nikon geometry; probe is Z6 III full-frame only"));
                }
                // PIH describes the Bayer mosaic, while each coded plane is half-sized.
                parsed.wf /= 2;
                parsed.hf /= 2;
                p.get_mut(..4).ok_or_else(|| invalid("PIH"))?.fill(0); // changed header size
                p.get_mut(8..10).ok_or_else(|| invalid("PIH"))?.copy_from_slice(&parsed.wf.to_be_bytes());
                p.get_mut(10..12).ok_or_else(|| invalid("PIH"))?.copy_from_slice(&parsed.hf.to_be_bytes());
                // Nikon always carries the long packet header.
                *p.get_mut(23).ok_or_else(|| invalid("PIH"))? |= 0x80;
                saw_pih = true;
                p
            }
            0xff13 if saw_pih && !saw_cdt => {
                if body != [14, 17, 81, 14, 17, 81, 14, 17, 0, 14, 17, 81] {
                    return Err(invalid("unsupported Nikon component decomposition"));
                }
                saw_cdt = true;
                vec![14, 17, 14, 17, 14, 17, 14, 17]
            }
            0xff14 if saw_cdt => {
                if body
                    != [
                        3, 2, 3, 19, 2, 9, 2, 22, 1, 10, 1, 21, 2, 5, 1, 3, 1, 17, 0, 1, 0, 7, 0, 13, 0, 11, 2, 6, 1, 4, 1, 16, 0, 0, 0, 8, 0, 14, 1,
                        20, 0, 18, 0, 15, 0, 23, 0, 12, 0, 24,
                    ]
                {
                    return Err(invalid("unverified Nikon weights"));
                }
                body.to_vec()
            }
            0xff20 if saw_pih && saw_cdt => {
                // Model logical components as [Nikon 0, 1, 3, 2], last one suppressed.
                out.extend_from_slice(&[0xff, 0x17, 0, 3, 1]); // CWD
                out.extend_from_slice(input.get(cursor..).ok_or_else(|| invalid("slice offset"))?);
                return Ok(out);
            }
            _ => return Err(invalid("unexpected Nikon header marker")),
        };
        out.extend_from_slice(&marker.to_be_bytes());
        let length = u16::try_from(replacement.len() + 2).map_err(|_| invalid("segment too long"))?;
        out.extend_from_slice(&length.to_be_bytes());
        out.extend_from_slice(&replacement);
        cursor = end;
        if cursor > 1024 {
            return Err(invalid("oversized Nikon header"));
        }
    }
}

// Standard interleaved band id -> Nikon component-major band id.
fn nikon_band(standard: usize) -> Result<usize> {
    if standard == 24 {
        return Ok(16);
    }
    if standard >= 24 {
        return Err(invalid("bad band id"));
    }
    let base = match standard % 3 {
        0 => 0,
        1 => 8,
        _ => 17,
    };
    Ok(base + standard / 3)
}

/// Reconstruct signed wavelet-domain component samples. No guessed black/white
/// level, chroma gain, inverse companding or display tone curve is applied.
pub fn decode_components(input: &[u8]) -> Result<(usize, usize, Vec<Vec<i32>>)> {
    if input.len() > 32 * 1024 * 1024 {
        return Err(invalid("oversized Nikon HE stream"));
    }
    let bytes = normalized_headers(input)?;
    let cs = codestream::parse(&bytes)?;
    let pih = cs.pih;
    let (plan, _) = slice_walker::build_plan_sd(&pih, &cs.cdt, &cs.wgt, 1)?;
    let width = usize::from(pih.wf);
    let height = usize::from(pih.hf);
    let pixels = width.checked_mul(height).ok_or_else(|| invalid("pixel count overflow"))?;
    if pixels > 6_500_000 {
        return Err(invalid("too many plane pixels"));
    }
    let mut planes = vec![vec![0i32; pixels]; 4];
    // Each row contributes sequentially to each band's picture-level buffer.
    let mut bands: Vec<Vec<Vec<i32>>> = (0..3).map(|_| (0..8).map(|_| Vec::new()).collect()).collect();
    for (si, slice_plan) in plan.slices.iter().enumerate() {
        let slice = cs.slices.get(si).ok_or_else(|| invalid("missing slice"))?;
        let mut cursor = slice.data_offset;
        let slice_end = cursor.checked_add(slice.data_length).ok_or_else(|| invalid("slice overflow"))?;
        let mut top: Option<PrecinctTop> = None;
        for precinct in &slice_plan.precincts {
            let mut geom = precinct.geometry.clone();
            for (standard, band) in precinct.geometry.bands.iter().enumerate() {
                *geom.bands.get_mut(nikon_band(standard)?).ok_or_else(|| invalid("band index"))? = *band;
            }
            // Nikon weights follow packet traversal rather than band-header order:
            // each component's six low-horizontal bands, then its LH/HH pair.
            let weight_order: Vec<usize> = (0..6).chain(8..14).chain([16]).chain(17..23).chain([6, 7, 14, 15, 23, 24]).collect();
            for (wire, id) in weight_order.iter().enumerate() {
                let pair = cs.wgt.get(wire * 2..wire * 2 + 2).ok_or_else(|| invalid("weight table"))?;
                let band = geom.bands.get_mut(*id).ok_or_else(|| invalid("weight band"))?;
                band.gain = *pair.first().ok_or_else(|| invalid("weight gain"))?;
                band.priority = *pair.get(1).ok_or_else(|| invalid("weight priority"))?;
            }
            geom.lh = 1;
            if top.is_none() {
                // Nikon emits D=3 even on the first precinct. Treat the missing
                // predecessor as zero; this hypothesis is checked by exact
                // packet consumption and will still require reference pixels.
                top = Some(PrecinctTop {
                    last_m: (0..geom.bands.len()).map(|b| vec![0; geom.ncg(b) as usize]).collect(),
                    t: vec![0; geom.bands.len()],
                });
            }
            let data = bytes.get(cursor..slice_end).ok_or_else(|| invalid("precinct range"))?;
            let ph = entropy::parse_precinct_header(data, &geom)?;
            let start = cursor.checked_add(ph.header_bytes).ok_or_else(|| invalid("precinct offset"))?;
            let end = start.checked_add(ph.lprc as usize).ok_or_else(|| invalid("precinct length"))?;
            if end > slice_end {
                return Err(invalid("precinct exceeds slice"));
            }
            cursor = start;
            let mut state = PrecinctState::default();
            let packet_bands: [Vec<usize>; 8] =
                [(0..6).collect(), (8..14).collect(), vec![16], (17..23).collect(), vec![6, 7], vec![14, 15], vec![16], vec![23, 24]];
            for (packet_index, ids) in packet_bands.iter().enumerate() {
                let mut layout = PacketLayout { entries: Vec::new() };
                for id in ids {
                    let band = geom.bands.get(*id).ok_or_else(|| invalid("packet band"))?;
                    let line = band.l0 + u16::from(packet_index == 6);
                    if line < band.l1 {
                        layout.entries.push(PacketEntry { band: u16::try_from(*id).map_err(|_| invalid("band overflow"))?, line });
                    }
                }
                if layout.entries.is_empty() {
                    continue;
                }
                let header = entropy::parse_packet_header(bytes.get(cursor..end).ok_or_else(|| invalid("packet range"))?, &geom)?;
                cursor += header.header_bytes;
                let packet = entropy::decode_packet_body(
                    bytes.get(cursor..end).ok_or_else(|| invalid("packet body range"))?,
                    &geom,
                    &ph,
                    &header,
                    &layout,
                    &mut state,
                    top.as_ref(),
                )
                .map_err(|e| Error::InvalidData(format!("slice={si} precinct={} packet={:?}: {e}", precinct.p, layout.entries)))?;
                cursor = cursor.checked_add(packet.bytes_consumed).ok_or_else(|| invalid("packet overflow"))?;
            }
            if cursor > end {
                return Err(invalid("packet data exceeds precinct"));
            }
            top = Some(PrecinctTop::capture(&geom, &ph, &state));
            let trunc = entropy::precinct_truncation(&geom, &ph);
            let values = dequant::dequantize_precinct(pih.qpih, &geom, &trunc, &state.coefficients, pih.fq);
            for standard in 0..25usize {
                let id = nikon_band(standard)?;
                let decoded = values.get(id).ok_or_else(|| invalid("missing decoded band"))?;
                if standard == 24 {
                    let row = usize::try_from(precinct.p).map_err(|_| invalid("row index"))? * 2;
                    let start = row * width;
                    let end = start.checked_add(decoded.len()).ok_or_else(|| invalid("plane overflow"))?;
                    planes.get_mut(3).and_then(|p| p.get_mut(start..end)).ok_or_else(|| invalid("plane bounds"))?.copy_from_slice(decoded);
                } else {
                    bands
                        .get_mut(standard % 3)
                        .and_then(|p| p.get_mut(standard / 3))
                        .ok_or_else(|| invalid("gather band index"))?
                        .extend_from_slice(decoded);
                }
            }
            cursor = end;
        }
        if cursor != slice_end {
            return Err(invalid("slice not fully consumed"));
        }
    }
    for (component, component_bands) in bands.iter().enumerate() {
        dwt::inverse_cascade_2d(width, height, pih.nlx, pih.nly, component_bands, planes.get_mut(component).ok_or_else(|| invalid("plane index"))?)?;
    }
    // Restore Nikon component numbering.
    planes.swap(2, 3);
    Ok((width, height, planes))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn header() -> Vec<u8> {
        let mut b = vec![0xff, 0x10];
        for (marker, body) in [
            (0xff50u16, b"CONTACT_INTOPIX_".to_vec()),
            (
                0xff12,
                vec![
                    0, 0x8c, 0x2e, 0, 0, 0, 0, 0, 0x17, 0xb0, 0x0f, 0xc8, 0, 0, 0, 16, 4, 4, 8, 18, 0x44, 0x10, 0x51, 0x14, 0x50, 0x88, 0x70, 0x83,
                    0xf0, 0x15, 0x23, 0xd1, 0x49, 0xcd, 0x3f, 0x7f, 7,
                ],
            ),
            (0xff13, vec![14, 17, 81, 14, 17, 81, 14, 17, 0, 14, 17, 81]),
            (
                0xff14,
                vec![
                    3, 2, 3, 19, 2, 9, 2, 22, 1, 10, 1, 21, 2, 5, 1, 3, 1, 17, 0, 1, 0, 7, 0, 13, 0, 11, 2, 6, 1, 4, 1, 16, 0, 0, 0, 8, 0, 14, 1, 20,
                    0, 18, 0, 15, 0, 23, 0, 12, 0, 24,
                ],
            ),
            (0xff20, vec![0, 0]),
        ] {
            b.extend_from_slice(&marker.to_be_bytes());
            b.extend_from_slice(&((body.len() + 2) as u16).to_be_bytes());
            b.extend_from_slice(&body);
        }
        b
    }

    #[test]
    fn rejects_every_truncated_header() {
        let input = header();
        for end in 0..input.len() {
            assert!(normalized_headers(&input[..end]).is_err(), "prefix {end}");
        }
        assert!(normalized_headers(&input).is_ok());
    }

    #[test]
    fn band_mapping_is_a_bijection() {
        let mut mapped: Vec<_> = (0..25).map(|i| nikon_band(i).unwrap()).collect();
        mapped.sort();
        assert_eq!(mapped, (0..25).collect::<Vec<_>>());
        assert!(nikon_band(25).is_err());
    }

    #[test]
    fn no_sample_decode_without_slice_payload() {
        assert!(decode_components(&header()).is_err());
    }
}
