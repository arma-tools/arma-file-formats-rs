# Animation probes

Black box fixtures for `anm_test.rs`. Each `.anm` is what Arma Reforger Workbench (1.8.0.13)
built from the `.txa` next to it, a text animation as the Blender tools export it. The `.txa`
gives the exact input values.

| Probe      | Covers                                                                 |
| ---------- | ---------------------------------------------------------------------- |
| `p_t`      | translation track, range and step, default rotation and scale         |
| `p_tneg`   | negative minimum                                                       |
| `p_full5`  | a key on every frame, component order, rounding                        |
| `p_q1`     | constant rotation reduced to one key, range of a constant track        |
| `p_tqs`    | all three tracks, data order translation, scale, rotation; full flags  |
| `p_two`    | two nested bones: no hierarchy in the file                             |
| `p_qs`     | `$keys q s` without `t`: translation track without keys                |
| `p_diff`   | `$nodeDiff`: additive flag                                             |
| `p_ev1`    | `$events`: `EVN3` with an EBIN document of two `AnimSrcEventGeneric`   |
| `p_cp`     | `$custProps`: `CPRP`, also an empty value                              |
| `p_linear` | linear motion over 9 frames reduced to keys on frames 0 and 8          |
| `p_hold`   | a step reduced to keys on frames 0, 3 and 4, held after the last key   |
| `p_slerp`  | a uniform 90 degree turn reduced to two rotation keys                  |
| `p_shortarc` | a -160 degree turn reduced to two keys whose dot product is negative: slerp takes the shorter arc |
| `p_one`    | a one frame animation: one key per track, "key on every frame" flags  |

Probes not kept here showed that a turn sampled with nlerp is not reduced (90 and 160
degrees, all 9 keys kept), that a bump of `1e-3` on a linear move is kept and one of `1e-4` is
dropped, and that constant tracks get a step of `0`.

## Regenerating

1. Write the sources: `python make_anm_probes.py <addon>/Anims` (any Python 3, e.g. Blender's
   bundled one).
2. Build them headless with the plugin `AP_Build.c` in `<addon>/Scripts/WorkbenchGame/`. It reads
   one absolute `.txa` path per line from the `AP_ARGS` file named in the plugin and calls
   `ResourceManager.RegisterResourceFile(path, true)` for new files and `RebuildResourceFile` for
   registered ones:

   ```sh
   ArmaReforgerWorkbenchSteamDiag.exe -wbModule=ResourceManager -plugin=AP_Build \
     -addonsDir "<game>/addons,<my addons>" -gproj <addon>/<addon>.gproj
   ```

   Run it from the Tools' `Workbench` directory. Of several newly registered files only the first
   is built, a second run builds the rest. Packing (`-packAddon`) does not build `.txa` files.
3. Copy the `.anm` files here.
