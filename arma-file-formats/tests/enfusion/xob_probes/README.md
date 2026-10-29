# XOB probes

Black box fixtures for `xob_test.rs`. Each probe isolates one feature of the format:

| Probe     | Covers                                                        |
| --------- | ------------------------------------------------------------- |
| `tri`     | single triangle, axis mapping, uv flip                        |
| `quad`    | uv bounds (uvs only use part of the texture)                  |
| `colors`  | vertex colors                                                 |
| `uv2`     | second uv set                                                 |
| `box`     | flat shaded 1 x 2 x 3 box, bounding box and radius           |
| `box2mat` | two materials, two sections                                   |
| `lods`    | three LODs with LOD factors                                   |
| `skinned` | two bone armature, bone hierarchy and skin weights            |
| `biggrid` | 257 x 257 vertex grid: section split, >65535 triangles, multi block LZ4 |
| `colliders` | one collider of every shape with known transforms, margin and layer |

## Regenerating

1. Build the FBX files with Blender:
   `blender -b --factory-startup --python make_probes.py -- <addon>/Assets/Probes`
   `blender -b --factory-startup --python make_collider_probes.py -- <addon>/Assets/Probes`
2. Register them in Arma Reforger Workbench (Resource Browser, select the FBX files, Register).
   For `skinned` set `ExportSkinning 1` in `skinned.xob.meta` and reimport.
   For `colliders` set `Mass 7.5` / `Margin 0.25` on `UBX_box` and `LayerPreset "Prop"` /
   `Mass 3` / `Margin 0.04` on `USP_sphere` in `colliders.xob.meta` and reimport.
3. Copy the resulting `.xob` and `.txo` files here. The `.txo` is Workbench's text dump of the
   imported mesh and is used as the expected result. `biggrid.txo` is left out because of its size.
