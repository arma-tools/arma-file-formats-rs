"""Blender (headless) script: build a mesh with shape keys of known offsets and export FBX.

run: blender -b --factory-startup --python make_morph_probes.py -- <out_dir>
Writes morphs.fbx: a flat 2 x 2 m grid of 3 x 3 vertices (8 triangles) with three shape keys:
- `up`: the center vertex 0.5 m up (Blender z).
- `skew`: vertex 0 by +0.25 m and vertex 2 by -0.125 m along x. Blender builds it from the
  current mix, so it also holds the offset of `up`.
- `scale`: every vertex 1.5 times as far from the center.
"""
import os
import sys

import bpy

OUT = sys.argv[sys.argv.index("--") + 1]
os.makedirs(OUT, exist_ok=True)

bpy.ops.wm.read_factory_settings(use_empty=True)

verts = [(x - 1.0, y - 1.0, 0.0) for y in range(3) for x in range(3)]
faces = []
for y in range(2):
    for x in range(2):
        a = y * 3 + x
        faces += [(a, a + 1, a + 4), (a, a + 4, a + 3)]
me = bpy.data.meshes.new("morphs")
me.from_pydata(verts, [], faces)
layer = me.uv_layers.new(name="UVMap")
for loop in me.loops:
    x, y, _ = verts[loop.vertex_index]
    layer.data[loop.index].uv = ((x + 1.0) / 2.0, (y + 1.0) / 2.0)
mat = bpy.data.materials.new("MorphMat")
me.materials.append(mat)
me.update()
ob = bpy.data.objects.new("morphs", me)
bpy.context.scene.collection.objects.link(ob)

ob.shape_key_add(name="Basis")
up = ob.shape_key_add(name="up")
up.data[4].co.z += 0.5
skew = ob.shape_key_add(name="skew")
skew.data[0].co.x += 0.25
skew.data[2].co.x -= 0.125
scale = ob.shape_key_add(name="scale")
for i, v in enumerate(verts):
    scale.data[i].co = (v[0] * 1.5, v[1] * 1.5, v[2])

def export(name, smooth_type="FACE"):
    bpy.ops.export_scene.fbx(
        filepath=os.path.join(OUT, name + ".fbx"),
        use_selection=False,
        apply_unit_scale=True,
        apply_scale_options="FBX_SCALE_ALL",
        axis_forward="-Z",
        axis_up="Y",
        mesh_smooth_type=smooth_type,
        use_tspace=True,
        use_mesh_modifiers=False,
        add_leaf_bones=False,
        bake_anim=False,
    )


export("morphs")

# morphs_smooth (and morphs_dyn, imported with DynamicMorph): a smooth icosphere of radius 1,
# upper half one material, lower half another. `bulge` lifts the top vertex by 0.3 m, only in
# the upper section. `squash` halves every z.
for name in ("morphs_smooth", "morphs_dyn"):
    bpy.ops.wm.read_factory_settings(use_empty=True)
    bpy.ops.mesh.primitive_ico_sphere_add(subdivisions=1, radius=1.0)
    ob = bpy.context.active_object
    ob.name = name
    me = ob.data
    me.materials.append(bpy.data.materials.new("TopMat"))
    me.materials.append(bpy.data.materials.new("BottomMat"))
    for poly in me.polygons:
        poly.use_smooth = True
        poly.material_index = 0 if poly.center.z >= 0 else 1
    top = max(range(len(me.vertices)), key=lambda i: me.vertices[i].co.z)
    ob.shape_key_add(name="Basis", from_mix=False)
    bulge = ob.shape_key_add(name="bulge", from_mix=False)
    bulge.data[top].co.z += 0.3
    squash = ob.shape_key_add(name="squash", from_mix=False)
    for i, v in enumerate(me.vertices):
        squash.data[i].co.z = v.co.z * 0.5
    export(name, "OFF")

# morphs_skinned: the two bone cylinder of `skinned` with a morph `wide` that moves the top ring
# out by 0.1 m.
bpy.ops.wm.read_factory_settings(use_empty=True)
bpy.ops.object.armature_add(location=(0, 0, 0))
arm = bpy.context.active_object
arm.name = "Armature"
bpy.ops.object.mode_set(mode="EDIT")
b0 = arm.data.edit_bones[0]
b0.name = "bone_root"
b0.head, b0.tail = (0, 0, 0), (0, 0, 1)
b1 = arm.data.edit_bones.new("bone_tip")
b1.head, b1.tail, b1.parent = (0, 0, 1), (0, 0, 2), b0
bpy.ops.object.mode_set(mode="OBJECT")
bpy.ops.mesh.primitive_cylinder_add(vertices=8, radius=0.25, depth=2, location=(0, 0, 1))
cyl = bpy.context.active_object
cyl.name = "morphs_skinned"
cyl.data.materials.append(bpy.data.materials.new("SkinMat"))
g0 = cyl.vertex_groups.new(name="bone_root")
g1 = cyl.vertex_groups.new(name="bone_tip")
for v in cyl.data.vertices:
    t = v.co.z / 2.0 + 0.5
    g0.add([v.index], 1.0 - t, "REPLACE")
    g1.add([v.index], t, "REPLACE")
cyl.shape_key_add(name="Basis", from_mix=False)
wide = cyl.shape_key_add(name="wide", from_mix=False)
for i, v in enumerate(cyl.data.vertices):
    if v.co.z > 0:
        wide.data[i].co.x = v.co.x * 1.4
        wide.data[i].co.y = v.co.y * 1.4
mod = cyl.modifiers.new("Armature", "ARMATURE")
mod.object = arm
cyl.parent = arm
export("morphs_skinned")
print("done morphs")
