# Pak probe

Black box fixture for `pak_test.rs`. `probe.pak` was packed by Arma Reforger Workbench from the
sources written by `make_probe.sh`. It covers:

- directory tree ordering (mixed case, `_`, non-ASCII UTF-8 names, directories between files)
- stored and zlib compressed entries, an empty file, a file larger than 64 KiB
- timestamps from the source file modification times, including the 2063 upper bound

## Regenerating

1. Create an addon in Workbench (or reuse one) and write the sources into it:
   `bash make_probe.sh <addon>`
2. Pack the addon from the `Workbench` directory of Arma Reforger Tools. Workbench resolves
   `./addons` against the working directory, so run it from there or the base game addon is not
   found:

   ```sh
   ArmaReforgerWorkbenchSteamDiag.exe -wbModule=ResourceManager \
       -addonsDir "<Arma Reforger>/addons,<addons dir>" -gproj "<addon>/<addon>.gproj" \
       -packAddon -packAddonDir "<out>" -packAddonCacheDir "<cache>" -exitAfterInit
   ```

3. Copy `<out>/data.pak` here as `probe.pak`. Packing is deterministic, the same sources produce
   an identical pak.

## Game paks

Game paks can't be committed, so two tests only run when an environment variable points at them.
Use `--release`, both read gigabytes.

- `AFF_PAK_SCAN_DIR=<dir>`: `pak_scan_dir_test` reads every `.pak` below the directory, e.g.
  `<Arma Reforger>/addons`. It checks the tree order and data layout and decompresses every zlib
  entry.
- `AFF_PAK_REPACK=<pak>`: `pak_repack_test` streams the raw entries of the pak into a new pak in
  the temp directory and checks that it is byte identical. This needs free space for one copy of
  the pak.

```sh
AFF_PAK_SCAN_DIR="<Arma Reforger>/addons" cargo nextest run --release --test tests pak_scan_dir_test
AFF_PAK_REPACK="<Arma Reforger>/addons/data/data.pak" cargo nextest run --release --test tests pak_repack_test
```

Last run against Arma Reforger (2026-10-30): all 16 shipped paks pass the scan, about 223k entries.
Repacking `data/data.pak` (1.86 GiB) took 6 s with a peak working set of 11 MiB, and
`data/worlds001.pak` (51k entries) peaked at 38 MiB.
