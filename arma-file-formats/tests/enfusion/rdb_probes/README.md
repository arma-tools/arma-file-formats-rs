# Resource database probe

Black box fixture for `rdb_test.rs`. `probe.rdb` is the `resourceDatabase.rdb` that Arma Reforger
Workbench (1.8.0.13) saved for the addon that `make_rdb_probe.sh` builds, when loading it headless:

```
ArmaReforgerWorkbenchSteamDiag.exe -wbModule=ResourceManager -addonsDir "<game>/addons,<addons>" \
  -gproj <addons>/RdbProbe/RdbProbe.gproj -exitAfterInit
```

The run takes a few seconds. Without an rdb Workbench does a full scan ("ResourceDB: full scan"),
logs `resource not registered ... Setting null GUID` for the files without a `.meta` and saves
the cache ("ResourceDB: written 766/766 bytes").

The addon has `.conf` files with `.meta`s (GUIDs `A000...`) that set the Resource Browser tags in
different combinations, a file without a `.meta`, an empty directory, a directory registered with
its own `Registered.meta`, a string table and a file with a non-ASCII name. All `.meta`s have a
fixed modification time, `a.conf.meta` a different one.

Findings:

- Tags are the `Tags` property of the `.meta`, a single string of space separated tags. Further
  strings are dropped (`"tagD" "tagE"` gives only `tagD`), `Tags { "tagA" }` gives the tag `{`.
  The rdb lists the addon's tags in the order of the scan, and every entry has a bit mask of them:
  `ab` 3, `cba` 7, `d` 8.
- Meta times are the `.meta`'s modification time in UTC, packed like pak timestamps. The sizes
  are the `.meta` sizes.
- Directories without `.meta` have GUIDs too. They don't depend on the addon: `Configs` has the
  same GUID in the game's `core` rdb (as `configs`). Common 64 bit hashes (FNV, CRC64, MurmurHash64A,
  MD5, SHA1, djb2) of the path don't give them.
- A registered directory has flags `0x07` and the GUID and time of its `.meta`.
- Workbench leaves out `ünïcode.conf` (and its `.meta`).
- Entries are sorted by their lowercased path: `_under` before `a`, `Registered/inner.conf` before
  `Upper.conf`.
- Flag `0x10` is only set on `localization.st` in the game's `data` rdb and `ui/language/default.st`
  in `core`. It is not on other `.st` files, also not with `StringTableResourceClass` in the
  `.meta` or when the `.gproj` lists the file as `StringTableSource`.
