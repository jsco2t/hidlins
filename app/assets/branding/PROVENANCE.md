# Hidlins brand artwork

The project owner supplied `.ai/workflow/hidlins-logo-package.zip` on
2026-09-01 and explicitly authorized its use as the Hidlins application logo.
The archive SHA-256 is
`99818bc4dfd16b3ab876d58a7cd3c5edd7ae1274ab42cb9d1ce13f9ad79561d6`.

The package README describes the artwork as a clean vector redraw of selected
AI-generated raster concepts. The canonical masters are the supplied
`hidlins-mono.svg`, `hidlins-navy.svg`, and `hidlins-vintage-dark.svg`, each with
an unchanged `0 0 1024 1024` viewBox. The archive also contains matching PNGs at
1024, 512, 256, 192, 180, 152, 128, 64, 60, 48, 32, and 16 pixels for each
treatment, three PDF exports, the README, and a navy multi-size favicon: 44 files
in total. PDFs, the favicon, and unused raster sizes are intentionally not copied
into the product tree because no runtime or packaging consumer needs them.

`manifest.json` maps every retained or generated product image to its treatment,
dimensions, exact hash, alpha policy, consumer, and transform. Exact supplied
rasters are copied byte-for-byte. `tools/dev/generate-brand-assets.swift` performs
the only derived operations: CoreGraphics resizing, an opaque navy flatten for
iOS, and a centered 66% monochrome foreground for Android's adaptive safe zone.
No tracing, redesign, runtime SVG library, or launcher-icon dependency is used.

## License

The repository's MIT license covers source code, but does **not** relicense this
logo package. The Hidlins brand artwork is separately reserved by the project
owner and is included here with permission for use and redistribution as part of
Hidlins. Reuse of the mark outside Hidlins requires separate permission. This
notice is the authoritative asset-license exception.
