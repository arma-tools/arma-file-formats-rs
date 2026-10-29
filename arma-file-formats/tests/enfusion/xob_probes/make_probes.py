"""Blender (headless) script: build probe meshes with known geometry and export FBX.

run: blender -b --factory-startup --python make_probes.py -- <out_dir>
Writes <name>.fbx plus expected.json describing each probe.
"""
import json
import math
import os
import sys

import bmesh
import bpy

OUT = sys.argv[sys.argv.index("--") + 1]
os.makedirs(OUT, exist_ok=True)
expected = {}


def reset():
    bpy.ops.wm.read_factory_settings(use_empty=True)


def material(name, color=(0.8, 0.8, 0.8, 1)):
    m = bpy.data.materials.get(name) or bpy.data.materials.new(name)
    m.diffuse_color = color
    return m


def mesh_object(name, verts, faces, uvs=None, mats=None, face_mats=None):
    me = bpy.data.meshes.new(name)
    me.from_pydata(verts, [], faces)
    if uvs is not None:
        layer = me.uv_layers.new(name="UVMap")
        for loop in me.loops:
            layer.data[loop.index].uv = uvs[loop.vertex_index]
    for m in mats or []:
        me.materials.append(m)
    if face_mats:
        for poly, mi in zip(me.polygons, face_mats):
            poly.material_index = mi
    me.update()
    ob = bpy.data.objects.new(name, me)
    bpy.context.scene.collection.objects.link(ob)
    return ob


def export(name, **extra):
    path = os.path.join(OUT, name + ".fbx")
    bpy.ops.export_scene.fbx(
        filepath=path,
        use_selection=False,
        apply_unit_scale=True,
        apply_scale_options="FBX_SCALE_ALL",
        axis_forward="-Z",
        axis_up="Y",
        mesh_smooth_type="FACE",
        use_tspace=True,
        add_leaf_bones=False,
        bake_anim=False,
    )
    info = {"objects": {}}
    for ob in bpy.context.scene.objects:
        if ob.type != "MESH":
            continue
        me = ob.data
        info["objects"][ob.name] = {
            "verts": [list(ob.matrix_world @ v.co) for v in me.vertices],
            "faces": [list(p.vertices) for p in me.polygons],
            "materials": [m.name for m in me.materials],
            "face_materials": [p.material_index for p in me.polygons],
        }
    info.update(extra)
    expected[name] = info


# 1. single triangle, asymmetric so every axis is identifiable
reset()
mesh_object(
    "tri",
    [(0.0, 0.0, 0.0), (1.0, 0.0, 0.0), (0.0, 2.0, 3.0)],
    [(0, 1, 2)],
    uvs=[(0.0, 0.0), (1.0, 0.0), (0.0, 1.0)],
    mats=[material("TriMat")],
)
export("tri")

# 2. quad 2 x 1 with a sub-range UV mapping (tests the UV bounds fields)
reset()
mesh_object(
    "quad",
    [(-1.0, 0.0, -0.5), (1.0, 0.0, -0.5), (1.0, 0.0, 0.5), (-1.0, 0.0, 0.5)],
    [(0, 1, 2, 3)],
    uvs=[(0.25, 0.25), (0.75, 0.25), (0.75, 0.5), (0.25, 0.5)],
    mats=[material("QuadMat")],
)
export("quad")

# 3. 1 x 2 x 3 box, flat shaded (24 render verts expected)
reset()
bpy.ops.mesh.primitive_cube_add(size=1)
box = bpy.context.active_object
box.name = "box"
box.scale = (1, 2, 3)
bpy.ops.object.transform_apply(scale=True)
box.data.materials.append(material("BoxMat"))
export("box")

# 4. box with two materials (3 faces each)
reset()
bpy.ops.mesh.primitive_cube_add(size=2)
b2 = bpy.context.active_object
b2.name = "box2mat"
b2.data.materials.append(material("MatA", (1, 0, 0, 1)))
b2.data.materials.append(material("MatB", (0, 0, 1, 1)))
for p in b2.data.polygons:
    p.material_index = 0 if p.index < 3 else 1
export("box2mat")

# 5. LODs: icosphere at decreasing subdivision, Enfusion _LODn naming
reset()
mat = material("LodMat")
for lod, sub in enumerate([3, 2, 1]):
    bpy.ops.mesh.primitive_ico_sphere_add(subdivisions=sub, radius=1)
    ob = bpy.context.active_object
    ob.name = f"sphere_LOD{lod}"
    ob.data.materials.append(mat)
export("lods")

# 6. vertex colours on a quad
reset()
q = mesh_object(
    "colors",
    [(-1.0, 0.0, -1.0), (1.0, 0.0, -1.0), (1.0, 0.0, 1.0), (-1.0, 0.0, 1.0)],
    [(0, 1, 2, 3)],
    uvs=[(0, 0), (1, 0), (1, 1), (0, 1)],
    mats=[material("ColMat")],
)
cols = [(1, 0, 0, 1), (0, 1, 0, 1), (0, 0, 1, 1), (1, 1, 1, 0.5)]
attr = q.data.color_attributes.new("Col", "BYTE_COLOR", "POINT")
for i, c in enumerate(cols):
    attr.data[i].color = c
export("colors", colors=cols)

# 7. second UV channel
reset()
q = mesh_object(
    "uv2",
    [(-1.0, 0.0, -1.0), (1.0, 0.0, -1.0), (1.0, 0.0, 1.0), (-1.0, 0.0, 1.0)],
    [(0, 1, 2, 3)],
    uvs=[(0, 0), (1, 0), (1, 1), (0, 1)],
    mats=[material("Uv2Mat")],
)
uv2 = q.data.uv_layers.new(name="UVMap2")
uv2_vals = [(0.5, 0.5), (0.75, 0.5), (0.75, 0.75), (0.5, 0.75)]
for loop in q.data.loops:
    uv2.data[loop.index].uv = uv2_vals[loop.vertex_index]
export("uv2", uv2=uv2_vals)

# 8. big grid: > 65535 verts forces 32-bit indices and multi-block LZ4
reset()
bpy.ops.mesh.primitive_grid_add(x_subdivisions=256, y_subdivisions=256, size=10)
g = bpy.context.active_object
g.name = "biggrid"
g.data.materials.append(material("GridMat"))
export("biggrid")

# 9. skinned: 2-bone armature, cylinder weighted top/bottom
reset()
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
cyl.name = "skinned"
cyl.data.materials.append(material("SkinMat"))
g0 = cyl.vertex_groups.new(name="bone_root")
g1 = cyl.vertex_groups.new(name="bone_tip")
for v in cyl.data.vertices:
    t = v.co.z / 2.0 + 0.5  # local z in [-1, 1]
    g0.add([v.index], 1.0 - t, "REPLACE")
    g1.add([v.index], t, "REPLACE")
mod = cyl.modifiers.new("Armature", "ARMATURE")
mod.object = arm
cyl.parent = arm
export("skinned", bones=["bone_root", "bone_tip"])

with open(os.path.join(OUT, "expected.json"), "w") as f:
    json.dump(expected, f, indent=1)
print("done", list(expected))
