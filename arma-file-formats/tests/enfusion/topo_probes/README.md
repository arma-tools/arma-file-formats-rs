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
- Hills: a second export, after writing two hills into the probe's `.ttile`s, was identical. The
  headless Workbench loads the terrain from `.EditorData`, so hill heights were checked against the
  game's terrains instead, see `topo::Hill`.

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
