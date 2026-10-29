use std::io::Cursor;

use binrw::{BinRead, binread};

use crate::errors::{AffError, XobError};

const COLLIDER_NO_BONE: u8 = u8::MAX;

/// One collider of the `COLL` chunk (`USP_`, `UCS_`, `UBX_`, `UCX_`, `UCL_` or `UTM_` objects).
#[binread]
#[derive(Debug, Clone, PartialEq)]
#[br(little, import(strings: &[String]))]
pub struct XobCollider {
    #[br(temp, assert((1..=7).contains(&shape_type), XobError::UnknownColliderShape(shape_type)))]
    shape_type: u8,
    /// Index into [`super::XobHead::bones`] the collider is attached to.
    #[br(map = |bone: u8| (bone != COLLIDER_NO_BONE).then_some(bone))]
    pub bone: Option<u8>,
    /// Index into [`super::XobHead::strings`].
    pub layer_index: u16,
    /// Layer preset, empty if none is set.
    #[br(calc = string_at(strings, layer_index))]
    pub layer: String,
    /// Local x, y and z axis of the collider, one per row.
    pub rotation: [[f32; 3]; 3],
    pub position: [f32; 3],
    pub unknown_52: u32,
    /// Index into [`super::XobHead::strings`].
    pub name_index: u16,
    #[br(calc = string_at(strings, name_index))]
    pub name: String,
    /// Index into [`super::XobHead::strings`].
    pub material_index: u16,
    /// Surface material, usually a `.gamemat` resource; may be empty or a `.physmat`.
    #[br(calc = string_at(strings, material_index))]
    pub material: String,
    pub margin: f32,
    #[br(args(shape_type, strings))]
    pub shape: XobColliderShape,
}

#[binread]
#[derive(Debug, Clone, PartialEq)]
#[br(little, import(shape_type: u8, strings: &[String]))]
pub enum XobColliderShape {
    #[br(pre_assert(shape_type == 1))]
    Sphere { radius: f32 },
    /// Aligned with the local y axis, `half_length` excludes the hemispheres.
    #[br(pre_assert(shape_type == 2))]
    Capsule { radius: f32, half_length: f32 },
    #[br(pre_assert(shape_type == 3))]
    Box { half_extents: [f32; 3] },
    #[br(pre_assert(shape_type == 4))]
    Convex(XobConvexHull),
    /// Aligned with the local y axis.
    #[br(pre_assert(shape_type == 7))]
    Cylinder { radius: f32, half_length: f32 },
    /// Type 6 carries per triangle range materials, type 5 does not.
    #[br(pre_assert(shape_type == 5 || shape_type == 6))]
    TriMesh(#[br(args(shape_type == 6, strings))] XobTriMesh),
}

/// Convex hull with its topology, vertices are relative to the collider position.
#[binread]
#[derive(Debug, Clone, Default, PartialEq)]
#[br(little)]
pub struct XobConvexHull {
    #[br(temp)]
    vertex_count: u16,
    #[br(temp)]
    face_count: u16,
    #[br(temp)]
    edge_count: u16,
    #[br(temp)]
    face_edge_count: u16,
    #[br(count = vertex_count)]
    pub vertices: Vec<[f32; 3]>,
    #[br(count = edge_count)]
    pub edges: Vec<XobHullEdge>,
    /// Edge indices, every face owns a range of them.
    #[br(count = face_edge_count)]
    pub face_edges: Vec<u16>,
    #[br(count = face_count)]
    pub faces: Vec<XobHullFace>,
}

#[derive(BinRead, Debug, Clone, Copy, PartialEq, Eq)]
#[br(little)]
pub struct XobHullEdge {
    pub vertices: [u16; 2],
    pub faces: [u16; 2],
}

#[derive(BinRead, Debug, Clone, Copy, PartialEq, Eq)]
#[br(little)]
pub struct XobHullFace {
    pub first_edge: u16,
    pub edge_count: u16,
}

#[binread]
#[derive(Debug, Clone, Default, PartialEq)]
#[br(little, import(has_material_ranges: bool, strings: &[String]))]
pub struct XobTriMesh {
    #[br(temp)]
    vertex_count: u16,
    #[br(temp)]
    triangle_count: u16,
    #[br(temp, if(has_material_ranges))]
    material_range_count: u32,
    /// Game materials per triangle range, empty if [`XobCollider::material`] applies to all.
    #[br(args { count: material_range_count as usize, inner: (strings,) })]
    pub material_ranges: Vec<XobMaterialRange>,
    #[br(count = vertex_count)]
    pub vertices: Vec<[f32; 3]>,
    #[br(count = triangle_count)]
    pub triangles: Vec<[u16; 3]>,
}

/// Triangles up to and including `last_triangle` (and after the previous range) use `material`.
#[binread]
#[derive(Debug, Clone, PartialEq, Eq)]
#[br(little, import(strings: &[String]))]
pub struct XobMaterialRange {
    /// Index into [`super::XobHead::strings`].
    pub material_index: u16,
    #[br(calc = string_at(strings, material_index))]
    pub material: String,
    pub last_triangle: u16,
}

impl XobCollider {
    pub(crate) fn read_all(data: &[u8], strings: &[String]) -> Result<Vec<Self>, AffError> {
        let mut input = Cursor::new(data);
        let mut colliders = Vec::new();
        while (input.position() as usize) < data.len() {
            colliders.push(Self::read_args(&mut input, (strings,))?);
        }
        Ok(colliders)
    }
}

fn string_at(strings: &[String], index: u16) -> String {
    strings.get(usize::from(index)).cloned().unwrap_or_default()
}
