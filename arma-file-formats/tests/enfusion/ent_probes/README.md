# Ent probe

Black box fixture for `ent_test.rs`. `probe.ent` was written by `Ent::write` from `probe_world` in
`ent_test.rs` (no game data, plain `GenericEntity`s):

```text
GenericEntity EntProbe_Single { coords 10 20 30  angles 0 90 0  scale 2.5 }
$grp GenericEntity {
 EntProbe_GroupA { coords 11 21 31 }
 { coords 12 22 32 }
}
GenericEntity EntProbe_Parent {
 coords 100 0 100
 {
  GenericEntity EntProbe_Child { coords 1 2 3 }
  $grp GenericEntity { EntProbe_GroupChild { coords 4 5 6 } }
 }
}
```

Arma Reforger Workbench (1.8.0.13) loaded it as `Worlds/EntProbe.ent` of an addon (with a `.meta`
copied from another world and a new GUID). A Workbench plugin listed every entity through
`WorldEditorAPI.GetEditorEntity` with `IEntitySource` `GetClassName`, `GetName`, `Get("coords")`,
`Get("angles")`, `Get("scale")` and `GetParent`:

| Entity                | coords          | angles   | scale | parent          |
| --------------------- | --------------- | -------- | ----- | --------------- |
| `EntProbe_Single`     | `10, 20, 30`    | `0 90 0` | 2.5   |                 |
| `EntProbe_GroupA`     | `11, 21, 31`    | `0 0 0`  | 1     |                 |
| (unnamed)             | `12, 22, 32`    | `0 0 0`  | 1     |                 |
| `EntProbe_Parent`     | `100, 0, 100`   | `0 0 0`  | 1     |                 |
| `EntProbe_Child`      | `1, 2, 3`       | `0 0 0`  | 1     | `EntProbe_Parent` |
| `EntProbe_GroupChild` | `4, 5, 6`       | `0 0 0`  | 1     | `EntProbe_Parent` |

That is the order and hierarchy of `Ent::entities`: groups expand to their entries and the keyless
block in an entity body holds its children, groups included.

The same listing for the game's `worlds/Editor/PrefabEditing/PrefabEditingTool.ent` (21 entities,
16 children) and `worlds/Eden/EmptyEden.ent` (14 entities, a group of 6 children) matched
`Ent::entities` in count, order, class, name, parent and every `coords` stored in the file.
`worlds/CaptureAndHold/CAH_BaseWorld_Arland.ent`, a sub scene of Arland, listed 173233 entities:
the 7 of `Ent::entities` plus the 173226 of Arland, so the `SubScene` object is no entity.

## Regenerating

```sh
AFF_ENT_PROBE_OUT=tests/enfusion/ent_probes/probe.ent cargo test --test tests probe_test
```

## Game worlds

Game files can't be committed. `ent_scan_dir_test` reads every `.ent` in the paks below
`AFF_ENT_SCAN_DIR`, checks that it writes back identically and that every entity is an object
instance listed after its parent.

```sh
AFF_ENT_SCAN_DIR="<Arma Reforger>/addons" cargo nextest run --release --test tests ent_scan_dir_test
```

Last run against Arma Reforger 1.8 (2026-10-31): all 103 shipped worlds pass.
