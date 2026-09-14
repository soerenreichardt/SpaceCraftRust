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
pub fn project_to_sphere(center_on_cube: Vec3, face: Face, scale: f32) -> Vec3 {
    let center_on_cube_surface = project_to_cube_surface(center_on_cube, face, scale);
    center_on_cube_surface.normalize() * ((center_on_cube - center_on_cube_surface).length() + scale)
}

impl CurvedBasePlane {
    pub fn compute_for_chunk(Chunk { center_on_cube, face, scale, .. }: &Chunk, surface_patch: &SurfacePatch) -> Self {
        let center_on_cube_surface = project_to_cube_surface(*center_on_cube, *face, *scale);
        let length = (center_on_cube - center_on_cube_surface).length() + scale;

        let mut vertices: Vec<[f32; 3]> = Vec::with_capacity((CHUNK_SIZE + 1) * (CHUNK_SIZE + 1));
        let center_on_cube_surface = DVec3::from(center_on_cube_surface);

        for y in 0..CHUNK_SIZE + 1 {
            for x in 0..CHUNK_SIZE + 1 {
                let vertex_on_cube_surface = surface_patch.get_vertex(x, y) + center_on_cube_surface;
                let vertex_on_sphere = vertex_on_cube_surface.normalize().as_vec3() * length;
                vertices.push(vertex_on_sphere.into());
            }
        }

        Self {
            vertices
        }
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