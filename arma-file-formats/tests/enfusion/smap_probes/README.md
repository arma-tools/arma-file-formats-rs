# Sound map probe

Black box fixture for `smap_test.rs`. `probe.smap` is what Arma Reforger Workbench (1.8.0.13)
exported with `MapDataExporter.ExportData(EMapDataType.Soundmap, dir, world)`, headless from a
Workbench plugin. The exporter needs a `SoundWorld` entity in the world, otherwise it fails with
`DataExportErrorNoSoundWorld`. It is deterministic: SF-Tutorial-Empty exports byte for byte as
shipped.

The world is the island of `smd_probes` (the 8 tiles of `terr_probes` with an 8 m grid, 2048 x
1024 m) with an ocean, written into the `.ttile`s with `Ttile::write`: land at +6 m in a sea at
-6 m, sloping 1 m per 4 m at the coast, except that the arm rises north of z 450:

| Part | x         | z        | Height                                 |
| ---- | --------- | -------- | -------------------------------------- |
| Base | 300..1000 | 200..400 | 6 m                                    |
| Arm  | 300..500  | 200..900 | `6 + 0.6 * (z - 450)` m, sides 3:1    |

Its entities: a `GenericWorldEntity` with `boundMins 200 -100 100` and `boundMaxs 1100 100 1000`,
the terrain, and `SoundWorld_Base.et` (`TreesNum 30`, `HousesNum 2`, `QueryMult 5`, `FilterForest
Average3`, `FilterCity Gauss7`, `FilterSea Gauss3`, `FilterCoast Gauss7`, `LakeMinArea 600`).

Findings:

- The grid starts at `boundMins` of the world entity and covers it in whole cells of 10 m (here
  90 x 90). Without bounds it is -1000..1000.
- Cells are stored row by row along x, starting at the lowest z.
- `sea` is 255 over the ocean and 0 on land, blurred: 255, 191, 63, 0 across the west coast
  (`[1, 2, 1] / 4` of 255, 255, 0, 0).
- `coast` is a band on both sides of the coastline: 4, 28, 83, 139 | 139, 83, 27, 3.
- `height` is the terrain height at the cell center in meters: 15 at z 465 and 225 at z 815 on
  the arm, 0 in the sea.
- `forest` and `city` stay 0: the headless export counted neither 2775 trees nor 100 garages on
  the island in an earlier version of the world. On Arland `forest` rises with the trees of the
  map's `.topo` per cell (mean 7 without trees, 140 with one, 228 with four).
- The freshwater layer is empty. Exports of the world with a lake (`LakeGeneratorEntity`) or a
  river hang, headless as well as with the World Editor's "Export Map data" tool, also with the
  lake in a hollow of the terrain. On the game's maps, against the water triangles of their
  `.topo`s, the first byte is set in rivers only, the second in rivers only and rising with the
  slope of the terrain, the third in lakes only and the fourth is always 0. Of four sub-cell
  orders, row by row along x from the lowest z matches the triangles best.
