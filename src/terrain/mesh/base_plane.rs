use crate::terrain::chunk::Chunk;
use crate::terrain::{Face, CHUNK_SIZE};
use bevy::math::DVec3;
use bevy::prelude::*;

pub struct CurvedBasePlane {
    pub(crate) vertices: Vec<[f32; 3]>,
    colors: Vec<[f32; 4]>,
}

impl CurvedBasePlane {
    pub fn compute_for_chunk(Chunk { center, face, length, scale, .. }: &Chunk) -> Self {
        let step_size = *length as f64 / CHUNK_SIZE as f64;
        let offset = *length as f64 / 2.0;
        let (axis_a, axis_b) = face.perpendicular_vectors();
        let axis_a = DVec3::from(axis_a);
        let axis_b = DVec3::from(axis_b);
        let (offset_a, offset_b) = (axis_a * offset, axis_b * offset);

        let mut vertices: Vec<[f32; 3]> = Vec::with_capacity((CHUNK_SIZE + 1) * (CHUNK_SIZE + 1));
        let mut colors: Vec<[f32; 4]> = Vec::with_capacity((CHUNK_SIZE + 1) * (CHUNK_SIZE + 1));
        let plane_center = DVec3::from(*center) - DVec3::from(center.normalize()) * offset;

        let channel = color_channel(face);
        for y in 0..CHUNK_SIZE + 1 {
            for x in 0..CHUNK_SIZE + 1 {
                let vertex = axis_a * (x as f64 * step_size) + axis_b * (y as f64 * step_size) - offset_a - offset_b + plane_center;
                let vertex = vertex.normalize() * *scale as f64;
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