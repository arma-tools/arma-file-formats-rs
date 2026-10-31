# Shore map probe

Black box fixtures for `smd_test.rs`. `probe.smd` and `shoreDFNearAtlas.edds` are what Arma Reforger
Workbench (1.8.0.13) built with `WorldEditorAPI.BuildShoreMap()` for a probe world, headless from a
Workbench plugin. It writes `<world>.smd`, `<world>_SMD.conf` with the default settings and the
atlases in `.Shore/` next to the world.

The world is the 8 tiles of `terr_probes` with an 8 m grid instead of 1.5 m (2048 x 1024 m) and
the "zero" height at -204.75 m. The heights were written into the `.ttile`s with `Ttile::write`
(the headless Workbench samples them with `GetSurfaceY`, it ignores `.EditorData`): an L-shaped
island at +6 m in a sea at -6 m, sloping 1 m per 4 m at the coast:

| Part | x       | z       |
| ---- | ------- | ------- |
| Base | 300..1000 | 200..400 |
| Arm  | 300..500  | 200..900 |

The `GenericWorldEntity` has the ocean of the game's worlds: `OceanMaterial`, `OceanSimulation`,
`WaterSimulation` and `ShoreSimulation` (without it the build fails with "there is no shore
simulation in the world").

Workbench logged:

```
numTiles: 5 x 5
tileSize (meters): 257.6
validTiles: 23 of 25 (92 percent)
one tile texture: 132 x 132 (17424 pixels, 1.98154 m/px)
tile atlas texture: 23 x 1 tiles, 3036 x 132 pixels (400752 total pixels, usage 100 %)
Shore map bounding box: (4, -92) -> (1292. 1196)
```

Findings:

- The bounding box is the coast's (about 300..1000, 208..900) grown by `DistanceFieldMaxValue`
  (300 m), extended to whole tiles: `max = min + tiles * tile_size`.
- Slots are 132 px with 130 px for the tile, also on a 384 x 192 m terrain with 253 m tiles. The
  atlas shape varies, here 23 x 1, it is always `atlas_slots * 132` pixels.
- The tile map is row by row along x, the first row at the lowest z: the two tiles without
  shore are the last two rows of the last column, the corner at high x and z far from both parts
  of the island. Mirrored in z they would touch the base, mirrored in x the arm.
- Slots are numbered in tile map order.
