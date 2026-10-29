//! Native Nikon Z6 III HE reconstruction for the measured full-frame profile.
//! Transport and output calibration have separate error and validation boundaries.
#[path = "nefh_reconstruct.rs"]
mod reconstruct;
#[path = "nefh_transport.rs"]
mod transport;

use crate::{RawError, Result};

pub(super) fn decode_rows(src: &[u8], width: usize, height: usize, rows: usize) -> Result<Vec<u16>> {
    if (width, height) != (6064, 4040) || src.len() > 32 * 1024 * 1024 {
        return Err(RawError::Unsupported("Nikon HE geometry outside the verified Z6 III profile".into()));
    }
    if src.len() != 9_186_816 {
        return Err(RawError::Unsupported("Nikon HE rate/strip size outside the verified profile".into()));
    }
    transport::normalized_headers(src).map_err(|e| RawError::Unsupported(format!("Nikon HE profile not verified ({e})")))?;
    if rows < height {
        // Header mode needs dimensions/levels only, not a full sensor allocation.
        return Ok(Vec::new());
    }
    let (w, h, planes) = transport::decode_components(src).map_err(|e| RawError::Corrupt(format!("Nikon HE entropy decode: {e}")))?;
    let values = reconstruct::companded_bayer(w, h, &planes).map_err(|e| RawError::Corrupt(format!("Nikon HE reconstruction: {e}")))?;
    Ok(values.into_iter().map(reconstruct::linear_sample).collect())
}
