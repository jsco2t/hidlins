# Task 004: Integrate Hidlins brand assets and launcher icons

Delegation: main-only

## Goal

Turn the user-supplied logo package into one documented, deterministic application
brand across Linux, macOS, iOS, Android, and the relevant in-app surfaces.

## Context

`.ai/workflow/hidlins-logo-package.zip` contains three canonical 1024×1024 SVG/PDF
treatments and raster exports at 16–1024 px. The navy treatment is already the
package's favicon treatment and is the primary full-color launcher candidate;
mono is suitable for symbolic/monochrome contexts, while vintage-dark is an
optional dark-surface in-app treatment. The current platform projects still use
Flutter template icons and the golden suite does not exercise the brand.

## Scope

### In scope

- Inventory and checksum the supplied archive; record source/provenance and the
  user's authorization to use the artwork in Hidlins.
- Track the canonical SVG masters and only the raster outputs actually required
  by application/runtime/platform packaging; do not copy PDFs or redundant sizes
  into shipping assets without a documented consumer.
- Use navy as the primary launcher/window/application icon, mono for a platform
  symbolic or monochrome slot where appropriate, and vintage-dark only where a
  dark in-app surface materially benefits from it.
- Replace Flutter template icons in macOS and iOS asset catalogs, Linux packaging,
  and Android legacy/adaptive icon resources.
- Add the approved mark to the lock/first-run and About/license surfaces without
  turning decorative imagery into noisy accessibility output.
- Add deterministic asset validation and brand goldens before the full desktop
  golden matrix is captured.

### Out of scope

- Redesigning, tracing, or AI-regenerating the supplied logo.
- Adding `flutter_launcher_icons`, an SVG runtime package, or another dependency
  solely to copy/resize assets already present in the archive.
- Trademark registration, store marketing artwork, or a broader brand guide.

## Implementation requirements

- Treat the supplied SVG files as canonical masters and preserve their viewBox;
  platform rasters must match the approved source hashes or a documented,
  deterministic flatten/crop operation.
- iOS launcher PNGs must be opaque and satisfy the asset-catalog size matrix;
  Android adaptive foreground content must stay within the safe zone and have an
  explicit background color; Linux/macOS icons must remain legible at 16/32 px.
- Store a small machine-readable asset manifest mapping each product file to its
  source treatment, dimensions, expected hash, alpha policy, and platform use.
- Asset checks must validate dimensions, hashes, Xcode catalog completeness,
  Android resource references/ABI-independent packaging, Linux install paths, and
  absence of leftover Flutter template icons.
- Generate and compare deterministic in-app/window captures plus simulator/
  emulator launcher captures wherever the platform test APIs expose them. Do not
  turn routine icon inspection into a manual checklist.
- In-app images use localized semantic labels only when informative; decorative
  marks are excluded from semantics.
- Record whether artwork is MIT-covered project material or separately reserved
  brand artwork; do not leave asset licensing/provenance ambiguous.

## Acceptance criteria

- [ ] The archive inventory and provenance/license record are committed, and every
  shipped brand asset maps to a canonical supplied master or exact supplied raster.
- [ ] Linux, macOS, iOS, and Android artifacts display the approved Hidlins mark in
  their launcher/window icon slots with no Flutter template icon remaining.
- [ ] iOS icons are opaque and complete; Android adaptive/legacy icons pass safe-zone
  and resource checks; 16/32 px desktop renders remain recognizable.
- [ ] Automated platform-resource and available launcher-capture checks pass; only
  vendor mask rendering outside the app process remains for the registered
  representative OS-shell observation in Tasks 010 and 013.
- [ ] Lock/first-run and About surfaces use the brand consistently in light/dark
  themes without changing focus order or announcing decorative duplicates.
- [ ] Brand-specific widget/golden and deterministic asset-manifest tests pass.

## Validation

- `make app-brand-check`
- `make app-test`
- `make app-build-macos`

## Dependencies

- Task 003

## Expected areas of change

- `app/assets/branding/` and an asset provenance/manifest document
- `app/ios/Runner/Assets.xcassets/`
- `app/macos/Runner/Assets.xcassets/`
- `app/android/app/src/main/res/`
- Linux packaging/CMake resources
- Lock, first-run, About/settings widgets and goldens
- `Makefile`

## Risks / notes

The supplied PNGs contain an alpha channel even when they visually include a solid
background, so iOS opacity must be measured rather than assumed. Android launchers
mask icons into multiple shapes; use a proper adaptive composition rather than
embedding a pre-rounded square unchanged.
