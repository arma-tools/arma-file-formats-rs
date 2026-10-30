# Terr probe

Black box fixture for `terr_test.rs`. `probe.terr` was created by Arma Reforger Workbench
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

## Regenerating

1. Create an addon in Workbench and a new world (Base scene) in it.
2. Add a `GenericTerrainEntity` at `0 0 0` and save the world.
3. Right-click the entity, `Create new terrain...`, enter the values above with Sync off and
   name `probe`, create and save the world.
4. Copy `Worlds/probe/probe.terr` here.

## Game terrains

Game files can't be committed. `terr_scan_dir_test` reads every `.terr` in the paks below
`AFF_TERR_SCAN_DIR`, checks that it writes back identically and that its tile count matches the
`.ttile` files next to it.

```sh
AFF_TERR_SCAN_DIR="<Arma Reforger>/addons" cargo nextest run --release --test tests terr_scan_dir_test
```

Last run against Arma Reforger 1.8 (2026-10-30): all 11 shipped terrains pass, versions 5, 7 and
9.
