use crate::terrain::chunk::Chunk;
use crate::terrain::mesh::mesh_generator::MESH_SIZE;
use crate::terrain::Face;
use bevy::math::DVec3;
use bevy::prelude::*;

pub struct CurvedBasePlane {
    pub(crate) vertices: Vec<[f32; 3]>,
    colors: Vec<[f32; 4]>,
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
        let mut colors: Vec<[f32; 4]> = Vec::with_capacity((MESH_SIZE + 1) * (MESH_SIZE + 1));
        let normal = DVec3::from(face.direction_vector());
        let center = DVec3::from(*center);
        // Build the grid on the surface plane (normal·p = scale) rather than at the
        // chunk's actual depth. The normalize()*scale projection's angular size depends
        // on |center|; pulling the grid onto the surface plane makes every surface-layer
        // chunk produce a patch of identical angular size that fits its neighbours,
        // independent of octree depth (restoring the quadtree behaviour).
        let surface_center = center + normal * (*scale as f64 - center.dot(normal));
        let channel = color_channel(face);
        for y in 0..MESH_SIZE + 1 {
            for x in 0..MESH_SIZE + 1 {
                let vertex = axis_a * (x as f64 * step_size) + axis_b * (y as f64 * step_size) - offset_a - offset_b + surface_center;
                let vertex = vertex.normalize() * *scale as f64 - center;
                let vertex = Vec3::new(vertex.x as f32, vertex.y as f32, vertex.z as f32);
                let t = if x == 0 && y == 0 { 0.0 } else { 1.0 };
                let mut color = [0.0, 0.0, 0.0, 1.0];
                color[channel] = t;

                vertices.push(vertex.into());
                colors.push(color);
            }
        }

        Self {
            vertices,
            colors,
        }
    }

    pub fn data(self) -> (Vec<[f32; 3]>, Vec<[f32; 4]>) {
        (self.vertices, self.colors)
    }

    pub fn vertex_at(&self, x: usize, y: usize) -> DVec3 {
        let values = self.vertices[y * (MESH_SIZE + 1) + x];
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