use crate::terrain::chunk::Chunk;
use crate::terrain::{Face, CHUNK_SIZE};
use bevy::math::DVec3;
use bevy::prelude::*;
use crate::terrain::mesh::cube::SurfacePatch;

pub struct CurvedBasePlane {
    pub(crate) vertices: Vec<[f32; 3]>,
}

#[inline]
pub fn project_to_cube_surface(position: Vec3, face: Face, scale: f32) -> Vec3 {
    match face {
        Face::Top => Vec3::new(position.x, scale, position.z),
        Face::Bottom => Vec3::new(position.x, -scale, position.z),
        Face::Left => Vec3::new(-scale, position.y, position.z),
        Face::Right => Vec3::new(scale, position.y, position.z),
        Face::Front => Vec3::new(position.x, position.y, scale),
        Face::Back => Vec3::new(position.x, position.y, -scale),
    }
}

#[inline]
pub fn expand_to_unit_sphere(position_on_unit_cube: DVec3) -> DVec3 {
    let x = position_on_unit_cube.x;
    let y = position_on_unit_cube.y;
    let z = position_on_unit_cube.z;
    let (x2, y2, z2) = (x * x, y * y, z * z);

    DVec3::new(
        x * (1.0 - y2 / 2.0 - z2 / 2.0 + y2 * z2 / 3.0).sqrt(),
        y * (1.0 - z2 / 2.0 - x2 / 2.0 + z2 * x2 / 3.0).sqrt(),
        z * (1.0 - x2 / 2.0 - y2 / 2.0 + x2 * y2 / 3.0).sqrt(),
    )
}

#[inline]
pub fn project_to_sphere(center_on_cube: Vec3, face: Face, scale: f32) -> Vec3 {
    let center_on_cube_surface = project_to_cube_surface(center_on_cube, face, scale);
    let on_unit_cube = DVec3::from(center_on_cube_surface) / scale as f64;
    let radius = (center_on_cube - center_on_cube_surface).dot(face.direction_vector()) + scale;
    (expand_to_unit_sphere(on_unit_cube) * radius as f64).as_vec3()
}

impl CurvedBasePlane {
    pub fn compute_for_chunk(Chunk { center_on_cube, face, scale, length, .. }: &Chunk, surface_patch: &SurfacePatch) -> Self {
        let center_on_cube_surface = DVec3::from(project_to_cube_surface(*center_on_cube, *face, *scale));
        let center_on_unit_cube = center_on_cube_surface / *scale as f64;
        let radius = (DVec3::from(*center_on_cube) - center_on_cube_surface).dot(face.direction_vector().into()) + *scale as f64;
        let radius = radius - *length as f64 / 2.0;

        let mut vertices: Vec<[f32; 3]> = Vec::with_capacity((CHUNK_SIZE + 1) * (CHUNK_SIZE + 1));

        for y in 0..CHUNK_SIZE + 1 {
            for x in 0..CHUNK_SIZE + 1 {
                let vertex_on_unit_cube = *surface_patch.get_vertex(x, y) + center_on_unit_cube;
                let vertex_on_sphere = expand_to_unit_sphere(vertex_on_unit_cube) * radius;
                vertices.push(vertex_on_sphere.as_vec3().into());
            }
        }

        Self { vertices }
    }

    pub fn vertices(self) -> Vec<[f32; 3]> {
        self.vertices
    }

    #[inline]
    fn index(x: usize, y: usize) -> usize {
        x + y * (CHUNK_SIZE + 1)
    }

    /// Computes the middle point of the 4 adjacent vertices of this plane — the
    /// center of cell (x, y). For example given the input (0, 0) the middle point
    /// is the point in the middle of (0, 0), (1, 0), (0, 1), (1, 1). Like the
    /// vertices, it is relative to the chunk center.
    pub fn middle_point_at(&self, x: usize, y: usize) -> DVec3 {
        let p1 = self.vertices[Self::index(x, y)];
        let p2 = self.vertices[Self::index(x + 1, y)];
        let p3 = self.vertices[Self::index(x, y + 1)];
        let p4 = self.vertices[Self::index(x + 1, y + 1)];

        let middle_point = (Vec3::from(p1) + Vec3::from(p2) + Vec3::from(p3) + Vec3::from(p4)) / 4.0;
        middle_point.into()
    }

    pub fn vertex_at(&self, x: usize, y: usize) -> DVec3 {
        let values = self.vertices[y * (CHUNK_SIZE + 1) + x];
        Vec3::from(values).into()
    }
}

fn color_channel(face: &Face) -> usize {
    match face {
        Face::Left | Face::Right => 0,
        Face::Top | Face::Bottom => 1,
        Face::Front | Face::Back => 2,
    }
}