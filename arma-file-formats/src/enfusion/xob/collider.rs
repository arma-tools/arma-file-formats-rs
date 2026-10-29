use std::io::{Cursor, Read, Seek};

use crate::{
    core::read::ReadExtTrait,
    errors::{AffError, XobError},
};

const COLLIDER_NO_BONE: u8 = u8::MAX;

/// One collider of the `COLL` chunk (`USP_`, `UCS_`, `UBX_`, `UCX_`, `UCL_` or `UTM_` objects).
#[derive(Debug, Clone, PartialEq)]
pub struct XobCollider {
    pub name: String,
    /// Index into [`super::XobHead::bones`] the collider is attached to.
    pub bone: Option<u8>,
    /// Layer preset, empty if none is set.
    pub layer: String,
    /// Surface material, usually a `.gamemat` resource; may be empty or a `.physmat`.
    pub material: String,
    /// Local x, y and z axis of the collider, one per row.
    pub rotation: [[f32; 3]; 3],
    pub position: [f32; 3],
    pub unknown_52: u32,
    pub margin: f32,
    pub shape: XobColliderShape,
}

#[derive(Debug, Clone, PartialEq)]
pub enum XobColliderShape {
    Sphere {
        radius: f32,
    },
    /// Aligned with the local y axis, `half_length` excludes the hemispheres.
    Capsule {
        radius: f32,
        half_length: f32,
    },
    Box {
        half_extents: [f32; 3],
    },
    Convex(XobConvexHull),
    /// Aligned with the local y axis.
    Cylinder {
        radius: f32,
        half_length: f32,
    },
    TriMesh(XobTriMesh),
}

/// Convex hull with its topology, vertices are relative to the collider position.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct XobConvexHull {
    pub vertices: Vec<[f32; 3]>,
    pub edges: Vec<XobHullEdge>,
    /// Edge indices, every face owns a range of them.
    pub face_edges: Vec<u16>,
    pub faces: Vec<XobHullFace>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct XobHullEdge {
    pub vertices: [u16; 2],
    pub faces: [u16; 2],
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct XobHullFace {
    pub first_edge: u16,
    pub edge_count: u16,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct XobTriMesh {
    pub vertices: Vec<[f32; 3]>,
    pub triangles: Vec<[u16; 3]>,
    /// Game materials per triangle range, empty if [`XobCollider::material`] applies to all.
    pub material_ranges: Vec<XobMaterialRange>,
}

/// Triangles up to and including `last_triangle` (and after the previous range) use `material`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct XobMaterialRange {
    pub material: String,
    pub last_triangle: u16,
}

impl XobCollider {
    pub(crate) fn read_all(data: &[u8], strings: &[String]) -> Result<Vec<Self>, AffError> {
        let mut input = Cursor::new(data);
        let mut colliders = Vec::new();
        while (input.position() as usize) < data.len() {
            colliders.push(Self::read(&mut input, strings)?);
        }
        Ok(colliders)
    }

    fn read<I: Read + Seek>(input: &mut I, strings: &[String]) -> Result<Self, AffError> {
        let string = |index: u16| strings.get(usize::from(index)).cloned().unwrap_or_default();

        let shape_type = input.read_u8()?;
        let bone = input.read_u8()?;
        let layer = string(input.read_u16()?);
        let rotation = [read_vec3(input)?, read_vec3(input)?, read_vec3(input)?];
        let position = read_vec3(input)?;
        let unknown_52 = input.read_u32()?;
        let name = string(input.read_u16()?);
        let material = string(input.read_u16()?);
        let margin = input.read_f32()?;

        let shape = match shape_type {
            1 => XobColliderShape::Sphere {
                radius: input.read_f32()?,
            },
            2 => XobColliderShape::Capsule {
                radius: input.read_f32()?,
                half_length: input.read_f32()?,
            },
            3 => XobColliderShape::Box {
                half_extents: read_vec3(input)?,
            },
            4 => XobColliderShape::Convex(read_convex(input)?),
            5 | 6 => XobColliderShape::TriMesh(read_tri_mesh(input, shape_type == 6, &string)?),
            7 => XobColliderShape::Cylinder {
                radius: input.read_f32()?,
                half_length: input.read_f32()?,
            },
            unknown => return Err(XobError::UnknownColliderShape(unknown).into()),
        };

        Ok(Self {
            name,
            bone: (bone != COLLIDER_NO_BONE).then_some(bone),
            layer,
            material,
            rotation,
            position,
            unknown_52,
            margin,
            shape,
        })
    }
}

fn read_convex<I: Read + Seek>(input: &mut I) -> Result<XobConvexHull, AffError> {
    let vertex_count = input.read_u16()?;
    let face_count = input.read_u16()?;
    let edge_count = input.read_u16()?;
    let face_edge_count = input.read_u16()?;

    Ok(XobConvexHull {
        vertices: (0..vertex_count)
            .map(|_| read_vec3(input))
            .collect::<Result<_, _>>()?,
        edges: (0..edge_count)
            .map(|_| {
                Ok(XobHullEdge {
                    vertices: [input.read_u16()?, input.read_u16()?],
                    faces: [input.read_u16()?, input.read_u16()?],
                })
            })
            .collect::<Result<_, AffError>>()?,
        face_edges: (0..face_edge_count)
            .map(|_| input.read_u16())
            .collect::<Result<_, _>>()?,
        faces: (0..face_count)
            .map(|_| {
                Ok(XobHullFace {
                    first_edge: input.read_u16()?,
                    edge_count: input.read_u16()?,
                })
            })
            .collect::<Result<_, AffError>>()?,
    })
}

fn read_tri_mesh<I: Read + Seek>(
    input: &mut I,
    has_material_ranges: bool,
    string: &impl Fn(u16) -> String,
) -> Result<XobTriMesh, AffError> {
    let vertex_count = input.read_u16()?;
    let triangle_count = input.read_u16()?;

    let material_ranges = if has_material_ranges {
        let count = input.read_u32()?;
        (0..count)
            .map(|_| {
                Ok(XobMaterialRange {
                    material: string(input.read_u16()?),
                    last_triangle: input.read_u16()?,
                })
            })
            .collect::<Result<_, AffError>>()?
    } else {
        Vec::new()
    };

    Ok(XobTriMesh {
        vertices: (0..vertex_count)
            .map(|_| read_vec3(input))
            .collect::<Result<_, _>>()?,
        triangles: (0..triangle_count)
            .map(|_| Ok([input.read_u16()?, input.read_u16()?, input.read_u16()?]))
            .collect::<Result<_, AffError>>()?,
        material_ranges,
    })
}

fn read_vec3<I: Read + Seek>(input: &mut I) -> Result<[f32; 3], AffError> {
    Ok([input.read_f32()?, input.read_f32()?, input.read_f32()?])
}
