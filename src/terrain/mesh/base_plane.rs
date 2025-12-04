use crate::terrain::chunk::Chunk;
use crate::terrain::mesh::mesh_generator::MESH_SIZE;
use bevy::math::DVec3;
use bevy::prelude::*;

pub struct CurvedBasePlane {
    vertices: Vec<[f32; 3]>,
}

impl CurvedBasePlane {
    pub fn compute_for_chunk(Chunk { center, face, length, scale, .. }: &Chunk) -> Self {
        let step_size = *length as f64 / MESH_SIZE as f64;
        let offset = *length as f64 / 2.0;
        let (axis_a, axis_b) = face.perpendicular_vectors();
        let axis_a = DVec3::from(axis_a);
        let axis_b = DVec3::from(axis_b);
        let (offset_a, offset_b) = (axis_a * offset, axis_b * offset);

        let mut vertices: Vec<[f32; 3]> = Vec::with_capacity((MESH_SIZE + 1) * (MESH_SIZE + 1));
        let center = DVec3::from(*center);
        for y in 0..MESH_SIZE + 1 {
            for x in 0..MESH_SIZE + 1 {
                let vertex = axis_a * (x as f64 * step_size) + axis_b * (y as f64 * step_size) - offset_a - offset_b + center;
                let vertex = vertex.normalize() * *scale as f64 - center;
                let vertex = Vec3::new(vertex.x as f32, vertex.y as f32, vertex.z as f32);
                vertices.push(vertex.into());
            }
        }

        Self {
            vertices
        }
    }

    pub fn vertices(self) -> Vec<[f32; 3]> {
        self.vertices
    }
}