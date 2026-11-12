# EFF Large Wordlist — Provenance and License

**File:** `crates/hidlins-genpw/src/assets/eff_large_wordlist.txt`
**Source:** https://www.eff.org/files/2016/07/18/eff_large_wordlist.txt
**Article:** https://www.eff.org/deeplinks/2016/07/new-wordlists-random-passphrases
**Downloaded:** 2026-05-21
**SHA-256:** `addd35536511597a02fa0a9ff1e5284677b8883b83e986e43f15a3db996b903e`
**Word count:** 7,776 (= 6⁵, five dice rolls per word)
**Format:** UTF-8 (ASCII-only in practice), LF line endings, no BOM. Each line is
`<5-digit-dice-prefix>\t<word>\n`. First line: `11111\tabacus`. Last line:
`66666\tzoom`.

## License status

**Retained under:** Creative Commons Attribution 4.0 International
([CC-BY-4.0](https://creativecommons.org/licenses/by/4.0/)).
**Terms checked:** 2026-11-03.

[EFF's copyright policy](https://www.eff.org/copyright) licenses original EFF
website material under CC-BY-4.0 unless otherwise noted. We apply that general
grant to this original EFF wordlist; its plain-text download has no separate
license header. The linked source and article did not establish a separate
CC0 or public-domain grant. Reuse intent alone does not establish such a grant.

The operator approved an exception on 2026-11-03 solely for the exact embedded
non-code asset identified by the SHA-256 above. All code dependency license
rules remain unchanged. The repository's MIT license does not relicense this
asset. This exception permits no replacement bytes or additional assets.

[EFF-wordlist-NOTICE.txt](EFF-wordlist-NOTICE.txt) preserves attribution to
Joseph Bonneau and the Electronic Frontier Foundation, the source and license
links, and the fact that our copy is unmodified. CLI and TUI binaries include
this notice in the console executable help output.

The wordlist is an embedded asset outside Cargo's dependency graph;
`cargo deny` does not establish its license or check its notices.

## Verification

Run `make asset-license-check` to verify the full canonical wordlist digest,
exact exception, attribution, compiled console notice, and
exclusion of CC-BY-4.0 from Cargo license allow-lists, including negative controls. `make verify` and
`make release` include this check. The generator's unit tests also preserve
word count, ordering endpoints, uniqueness and the supported character profile.

Any wordlist change requires new review and authorization; updating the hash
to accept replacement bytes does not fall under this exact-asset exception.
