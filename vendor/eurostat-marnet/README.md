# Vendored Eurostat SeaRoute / MARNET GeoPackages

This directory contains five maritime network GeoPackage files
vendored byte-for-byte from the Eurostat SeaRoute project so that
`cargo build` is self-contained and requires no runtime downloads.

The files are **unmodified copies** of the upstream resources. They
have not been recompressed, re-encoded, or otherwise transformed.

## Provenance chain

1. Oak Ridge National Labs — global commercial shipping lanes
   (foundation dataset).
2. Eurostat / SeaRoute — enriched with AIS data, processed into a
   maritime network graph, and simplified into five resolutions
   (5, 10, 20, 50, 100 km).
3. `eurostat/searoute` GitHub repository — published as classpath
   resources under `modules/core/src/main/resources/marnet/`.
4. **`rustyroute`** — vendored into `vendor/eurostat-marnet/`.

## Source

- Upstream repository: <https://github.com/eurostat/searoute>
- Upstream commit (pinned):
  `88a2e568a8e0144d1f5a81c3931a7bc2bcce6901`
  (default branch `master`, dated 2022-01-10)
- Upstream directory: `modules/core/src/main/resources/marnet/`
- Downloaded on (UTC): `2026-05-16`
- Download method: `curl` against
  `https://raw.githubusercontent.com/eurostat/searoute/88a2e568a8e0144d1f5a81c3931a7bc2bcce6901/modules/core/src/main/resources/marnet/marnet_plus_<RES>km.gpkg`

## Files

| File | Size (bytes) | SHA-256 |
| --- | ---: | --- |
| `marnet_plus_5km.gpkg`   | 6963200 | `75ca6cc130b3748f568196a9caf62889e9f096ec358f65223b4095ac070be0e7`   |
| `marnet_plus_10km.gpkg`  | 4661248 | `738cf880623dee8be3616a3ca46b565cc47a6548003ebd4438fea309dc7fa4f9`  |
| `marnet_plus_20km.gpkg`  | 2891776 | `8d909ba49d5062e1bdd7b305ee768e3366f1eab2d04b4ac448dfbdf9bf71a915`  |
| `marnet_plus_50km.gpkg`  | 1552384 | `44a309ffa0fbdc02e00616ed6a9e20ad92c773994b8f4502995cee9ffdf96acf`  |
| `marnet_plus_100km.gpkg` | 1024000 | `d0cd431c40efc2aa2cb1c218e0bf0590ef8dd0ba30eee157abe848900b16f3af` |

Total: ~17.1 MiB across five files.

## License

These GeoPackages are published by Eurostat under the European Union
Public Licence v. 1.2 (EUPL-1.2), the same license under which
`rustyroute` is distributed. See the top-level `LICENSE` file for
the full text and `NOTICE` for attribution.

EUPL-1.2 Article 5 obliges any redistribution to retain copyright
notices and indicate the original source. Those obligations are
satisfied by this README, the top-level `NOTICE`, and the unchanged
`LICENSE` file in the crate root.

## How to verify

Re-fetching the same URLs from the pinned commit and re-computing
SHA-256 checksums must produce the values in the table above. If
they do not, upstream has changed (unexpected — the pinned commit
is from 2022-01-10) or the local copies are damaged.

    rev=88a2e568a8e0144d1f5a81c3931a7bc2bcce6901
    base=https://raw.githubusercontent.com/eurostat/searoute/${rev}/modules/core/src/main/resources/marnet
    for res in 5 10 20 50 100; do
      curl -fL "${base}/marnet_plus_${res}km.gpkg" | sha256sum
    done

## Updating

To re-vendor against a newer upstream commit, replace the commit
SHA above, re-run the fetch script in this repository's `ENG-4677`
plan, re-record sizes and checksums in this README, and update the
top-level `NOTICE` to reflect the new commit SHA and date.
