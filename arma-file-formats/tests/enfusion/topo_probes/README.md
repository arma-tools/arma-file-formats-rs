# Topo probe

Black box fixtures for `topo_test.rs`. `probe.ent` is a binary world written with
`enfusion::ent`. It references game prefabs and materials by GUID only. `probe.topo` is what Arma
Reforger Workbench (1.8.0.13) exported from it with
`MapDataExporter.ExportData(EMapDataType.Geometry2D, dir, world, 50, true)`.

The world lies on the 384 x 192 m terrain of `terr_probes` (the `GenericTerrainEntity` named
`probe`, data in `probe/probe.terr` next to the world), which is flat at 0 m. The exporter needs an
`SCR_MapEntity`, otherwise it fails with `DataExportErrorNoMapEntity`.

| Entity | World position | Setup | In `probe.topo` |
| ------ | -------------- | ----- | --------------- |
| `RoadEntity` Road_Minor | 20, 20 | 2 spline points to 120, 20, `Type 2`, `Width 8` | road type 2, 4 vertices 8 m apart |
| `RoadEntity` Road_Trail | 20, 60 | 4 spline points, `Type 3`, `Width 4` | road type 3, 88 vertices |
| `RoadEntity` Road_PathA, Road_PathB | 200, 20 and 250, 20 | `Type 5`, `Width 2`, B starts where A ends | two roads of type 5, not merged |
| `RoadEntity` Road_Bridge | 200, 60 | `Type 6`, `Width 6` | road type 6 |
| `RoadEntity` Road_Line | 300, 100 | `Type 255`, `Width 1` | left out |
| `RoadEntity` Road_Runway | 20, 150 | `Type 0`, `Width 20` | road type 0 |
| `PowerlineEntity` | 200, 120 | one `Cable` to 240, 140 | left out, no power lines |
| `SCR_DestructibleBuildingEntity` | 320, 40 | yaw 30 | one footprint in category 0, building point at 320.29, 40.48 with direction (0.5, 0.866) |
| `SCR_IndestructibleEnvironmentalEntity` | 330, 130 | a prefab common in Arland | left out |
| `PolylineShapeEntity` + `LakeGeneratorEntity` Pond | 60, 100 | 20 m square, `m_bIsLake 0` | left out |
| `PolylineShapeEntity` + `LakeGeneratorEntity` Lake | 110, 100 | 30 m square, `m_bIsLake 1` | water type 3, 2 triangles |
| `SplineShapeEntity` + `RiverEntity` | 160, 100 | 3 points to 290, 170 | water type 2, 16 triangles |
| `Tree` x 3 | 300, 170; 305, 170; 310, 175 | a prefab whose trees are on Arland's map | 3 area points |

Findings:

- Roads, footprints, water and trees are in map coordinates: `z` is mirrored to
  `192 - z`. A road at world `z = 20` lies at `z = 172`. Building points are world coordinates.
  The game's files agree: all 65909 area points of Arland are trees at `4096 - z`.
- Roads are sorted by type, from the highest. The quads go from the first spline point to the
  last, as wide as `Width`. The straight roads have 4 vertices in the first level of detail and
  12 in the others.
- The building point is the entity's origin (within a meter) and its forward vector.
- No hills: the terrain is flat. Hills written into its `.ttile`s were lost, as its "zero" height
  of 100 % puts the whole 16 bit range below 0 m. See the hill probe below.

## Hill probe

`hills.topo` and `mesas.topo` were exported the same way, with a minimum hill height of 30, from
a world holding just the terrain and an `SCR_MapEntity`. The terrain is the 8 tiles of
`terr_probes` with an 8 m grid instead of 1.5 m (2048 x 1024 m) and the "zero" height at
-204.75 m, flat at 10 m, with the hills written into the `.ttile`s with `Ttile::write`. Workbench
samples these heights (`GetSurfaceY`), it ignores `.EditorData`. Heights below are as stored,
in steps of 0.05 m.

`hills.topo`, peaks in world coordinates on vertices:

| Hill | Peak       | Height | Shape                             | In `hills.topo`       |
| ---- | ---------- | ------ | --------------------------------- | --------------------- |
| A    | 296, 248   | 120.35 | cone of radius 200                | 113 at 74, 194        |
| B    | 800, 704   | 75.6   | Gaussian, sigma 80                | 78 at 200, 80         |
| C    | 1496, 296  | 90.5   | flat top of radius 60             | 93 at 374, 182        |
| D    | 1752, 800  | 45.3   | Gaussian, sigma 60                | 43 at 438, 56, left out with a minimum of 50 |
| E    | 1104, 400  | 100.8  | Gaussian, sigma 50                | 103 at 276, 156       |
| E2   | 1192, 400  | 96.45  | Gaussian, sigma 50, next to E     | 93 at 298, 156        |

`mesas.topo`: ten flat tops of radius 60, sloping down to 10 m at radius 180, at x 200, 600,
..., 1800 and z 256 (the first five) and 768, with tops of 100, 100.5, ..., 104.5 m.

Findings:

- Positions are map coordinates (`z` mirrored) in units of half a grid cell: A at 74, 194 is
  296, 1024 - 4 * 194. A second export with a 4 m grid put A at 148, 132. With the game's 2 m
  grids the units are meters.
- Heights are a 5 m contour plus 3 m. A top of 100.0 m gives 98, from 100.5 m it gives 103, so
  isolated round and flat tops get `5 * floor((top - 0.5) / 5) + 3`, as do B, C, D and E. E2,
  next to the higher E, gets a level less. Sharp peaks differ: A gets 113 with the 8 m grid and 123 with the 4 m grid, cones of radius 150 with peaks
  of 100 to 105 m all get 98.
- With a minimum hill height of 50 D is left out. On a 60 m plateau with small bumps, minimums of
  40 and 55 keep the bumps (63 and 68), 60 keeps only the 68 and 65 none, so the contour has to
  be above the minimum. A minimum of 30 left out all of them, for reasons unknown.
- All hills have whole number heights and zeros in the other fields. None of the probes gave a
  hill with a fractional height like those of the game's files, nor did small bumps of 0.5 to
  8 m. Arland's fractional hills have no common entity at their positions.

## Regenerating

1. Write the world with `enfusion::ent` as `Worlds/TopoProbe.ent` of an addon holding the
   `terr_probes` terrain as `Worlds/probe/`, with a `.meta` copied from another world and a new
   GUID.
2. Run a Workbench plugin that calls `MapDataExporter.ExportData` on it.

## Game files

`topo_scan_dir_test` reads every `.topo` in the paks below `AFF_TOPO_SCAN_DIR`. It checks that
each one writes back identically and has the structure the docs describe.

```sh
AFF_TOPO_SCAN_DIR="<Arma Reforger>/addons" cargo nextest run --release --test tests topo_scan_dir_test
```

Last run against Arma Reforger 1.8 (2026-10-31): all 4 shipped topos pass.
