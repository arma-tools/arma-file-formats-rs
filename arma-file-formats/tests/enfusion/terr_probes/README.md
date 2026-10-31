# Terr probe

Black box fixtures for `terr_test.rs` and `ttile_test.rs`. `probe.terr` was created by Arma Reforger Workbench
(1.8.0.13). Every setting differs from the defaults so each field of the file can be identified:

| New Terrain dialog         | Value             | Stored as                                    |
| -------------------------- | ----------------- | -------------------------------------------- |
| Width x Height             | 256 x 128         | `HEAD` vertices `257`, `129`                 |
| Blocks per tile            | 2                 | `HEAD` blocks per tile, 4 x 2 `.ttile` files |
| Grid cell size (meters)    | 1.5               | `HEAD`                                       |
| Height scale (meters)      | 0.05              | `HEAD`                                       |
| "Zero" height              | 100 (%)           | `HEAD` height offset `-65535 * 0.05`         |
| Surface layer weight bits  | 5 bits (7 surfaces) | `MATS` surface bits `5`                    |

Workbench reports the terrain bounds as `0 0 0` to `384 0 192`, so the first vertex count is
along x. It also loads the probe after `Terr::write` changed its grass cluster size.

`probe_0.ttile` and `probe_7.ttile` are the first and last of its tiles. Workbench numbers them
row by row along x and addresses blocks by their coordinates in the whole terrain: tile 7 holds
blocks `6, 2` to `7, 3`.

The height map layout was checked by writing the slope `-(0.1 x + 0.5 z)` meters into all eight
tiles with `Ttile::write`, reopening the world and sampling it with the Workbench NET API
(`wb_terrain getHeight`): `300, 30` gave `-45`, `30, 150` gave `-78` and `150, 30` gave `-30`, so
heights are stored row by row along x, starting at the lowest z.

The material map layout was checked on the game's Arland, as the probe has a single material:
400 texels inside uniformly painted areas, chosen so that mirroring or swapping the axes of the
map would change their material, were traced in Workbench with `TraceParam.TraceMaterial`
straight down at their center. All 400 matched `TileBlockMaterials::material_at`. Any mirroring
or swapping matched at most 251.

## Regenerating

1. Create an addon in Workbench and a new world (Base scene) in it.
2. Add a `GenericTerrainEntity` at `0 0 0` and save the world.
3. Right-click the entity, `Create new terrain...`, enter the values above with Sync off and
   name `probe`, create and save the world.
4. Copy `Worlds/probe/probe.terr` and `Worlds/probe/.Data/probe_0.ttile` and `probe_7.ttile`
   here.

## Game terrains

Game files can't be committed. `terr_scan_dir_test` reads every `.terr` in the paks below
`AFF_TERR_SCAN_DIR`, checks that it writes back identically and that its tile count matches the
`.ttile` files next to it. `ttile_scan_dir_test` does the same for every `.ttile` and checks it
against its terr: grid and block counts, block coordinates, that every texel of the material
maps resolves to a material, that the layers of a block include its materials and that the level
of detail errors are whole height map values that never decrease.

```sh
AFF_TERR_SCAN_DIR="<Arma Reforger>/addons" cargo nextest run --release --test tests scan_dir_test
```

Last run against Arma Reforger 1.8 (2026-10-30): all 11 shipped terrains and their 5282 tiles
pass, versions 5, 7 and 9.
