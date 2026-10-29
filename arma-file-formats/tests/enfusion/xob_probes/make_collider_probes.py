"""Blender (headless): collider probe with one collider of every kind, known transforms.

run: blender -b --factory-startup --python make_collider_probes.py -- <out_dir>
"""
import math
import os
import sys

import bpy

OUT = sys.argv[sys.argv.index("--") + 1]
os.makedirs(OUT, exist_ok=True)


def material(name):
    return bpy.data.materials.get(name) or bpy.data.materials.new(name)


def export(name):
    bpy.ops.export_scene.fbx(
        filepath=os.path.join(OUT, name + ".fbx"),
        apply_unit_scale=True,
        apply_scale_options="FBX_SCALE_ALL",
        axis_forward="-Z",
        axis_up="Y",
        mesh_smooth_type="FACE",
        add_leaf_bones=False,
        bake_anim=False,
    )


bpy.ops.wm.read_factory_settings(use_empty=True)

# visual mesh, so the model has a LOD
bpy.ops.mesh.primitive_cube_add(size=1, location=(0, 0, 0))
body = bpy.context.active_object
body.name = "body"
body.data.materials.append(material("BodyMat"))

# box 1 x 2 x 3 (Blender axes), rotated 30 degrees about Blender z, moved to x = 2
bpy.ops.mesh.primitive_cube_add(size=1, location=(2, 0, 0), rotation=(0, 0, math.radians(30)))
box = bpy.context.active_object
box.name = "UBX_box"
box.scale = (1, 2, 3)

# sphere, radius 0.5 at Blender y = 3
bpy.ops.mesh.primitive_uv_sphere_add(radius=0.5, location=(0, 3, 0), segments=32, ring_count=16)
bpy.context.active_object.name = "USP_sphere"

# cylinder, radius 0.3, length 1.2 along Blender z, at x = -2
bpy.ops.mesh.primitive_cylinder_add(radius=0.3, depth=1.2, location=(-2, 0, 0), vertices=32)
bpy.context.active_object.name = "UCL_cylinder"

# capsule, radius 0.25, total length 1.5 along Blender x, at Blender y = -3
bpy.ops.mesh.primitive_uv_sphere_add(radius=0.25, location=(0, -3, 0), segments=24, ring_count=12)
capsule = bpy.context.active_object
capsule.name = "UCS_capsule"
bpy.ops.object.mode_set(mode="EDIT")
bpy.ops.object.mode_set(mode="OBJECT")
for v in capsule.data.vertices:  # stretch the hemispheres apart: half segment length 0.5
    v.co.z += 0.5 if v.co.z > 0 else -0.5 if v.co.z < 0 else 0
capsule.rotation_euler = (0, math.radians(90), 0)

# convex hull: square pyramid, base 1 x 1 at Blender z = 2, apex at z = 3
pyramid = bpy.data.meshes.new("UCX_pyramid")
pyramid.from_pydata(
    [(-0.5, -0.5, 2), (0.5, -0.5, 2), (0.5, 0.5, 2), (-0.5, 0.5, 2), (0, 0, 3)],
    [],
    [(3, 2, 1, 0), (0, 1, 4), (1, 2, 4), (2, 3, 4), (3, 0, 4)],
)
bpy.context.scene.collection.objects.link(bpy.data.objects.new("UCX_pyramid", pyramid))

# triangle mesh: one quad (two triangles) at Blender z = -2
quad = bpy.data.meshes.new("UTM_quad")
quad.from_pydata([(-1, -1, -2), (1, -1, -2), (1, 1, -2), (-1, 1, -2)], [], [(0, 1, 2, 3)])
bpy.context.scene.collection.objects.link(bpy.data.objects.new("UTM_quad", quad))

# triangle mesh with two materials, 2 + 4 triangles
strip = bpy.data.meshes.new("UTM_twomat")
strip.from_pydata(
    [(x, 0, -4 + z) for z in (0, 1) for x in (0, 1, 2, 3)],
    [],
    [(0, 1, 5, 4), (1, 2, 6, 5), (2, 3, 7, 6)],
)
strip.materials.append(material("ColA"))
strip.materials.append(material("ColB"))
for poly, mi in zip(strip.polygons, (0, 1, 1)):
    poly.material_index = mi
bpy.context.scene.collection.objects.link(bpy.data.objects.new("UTM_twomat", strip))

export("colliders")
print("done")
