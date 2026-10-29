# Nikon Z6 III High Efficiency: local native decoder

This local build decodes the supplied full-frame Nikon Z6 III HE NEFs directly.
The original NEF stays the library source; adjustments use LightCraft's normal
non-destructive develop settings. No converter is invoked during import,
development, or export.

## Verified profile

Support is deliberately restricted to model `NIKON Z6_3`, 6064 × 4040, 14-bit
RGGB, a single 9,186,816-byte JPEG XS strip, black level 1008, and the measured
PIH extension, decomposition and weight table. Other HE variants retain the
unsupported/embedded-preview path. HE*, cropped modes and other Nikon models
are not claimed as supported.

The active sensor is 6064 × 4040; the default image crop is 6048 × 4032 at
(8, 4), with white level 15892. These match the three reference DNGs.

## Reconstruction and evidence

The independently written transport adapter handles Nikon's component-major
bands, packet traversal, long headers and suppressed component 2. Standard
entropy and wavelet kernels come from MIT-licensed OxideAV JPEG XS, pinned at
`e99ae89540e771994f4d17e91ef0440fc15948d2`, with default features disabled.
No GPL decoder implementation or Adobe application internals were used.

Same-exposure sensor references established the component order as
Y/Cr/green-difference/Cb, with transposed Star-Tetrix geometry and exponents
e1 = e2 = 0. There is no additional chroma gain. Output mapping is empirical,
not a published Nikon specification: a positive quadratic and a negative
quadratic that preserves noise below black.

Three original NEFs were converted once with Adobe DNG Converter 18.7 for
validation. Only decoded sensor values, geometry and black/white levels were
used; no Adobe colour profile or camera matrix is bundled. The positive
mapping was fitted on MST_3508 and checked on MST_4562 and MST_4722. Across
73,495,680 samples, every result is within 2 DN of the reference and more than
99.996% are within 1 DN. Mean error is below 0.005 DN in each frame. This is
not a bit-exact decoder claim, and it does not establish Lightroom colour
rendering parity.

Private photographs and references remain outside the source tree. Run
`cargo test -p lightcraft-raw --release --test nikon_he_reference` with
`LIGHTCRAFT_NIKON_HE_NEF_DIR` pointing to the originals and
`LIGHTCRAFT_NIKON_HE_REFERENCE_DIR` to the decoded `.raw` reference files.
The test checks full sensor arrays, header/full geometry agreement, all NEFs
in the supplied folder, truncated inputs and selected corrupt headers/packets.
