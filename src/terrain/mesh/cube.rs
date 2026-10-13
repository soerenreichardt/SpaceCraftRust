use std::collections::HashMap;
use bevy::math::DVec3;
use bevy::prelude::*;
use crate::terrain::{Face, CHUNK_SIZE};

const MAX_DEPTH: usize = 20;

#[derive(Resource)]
pub(crate) struct SurfacePatches {
    patches: HashMap<Face, [SurfacePatch; MAX_DEPTH]>,
}

#[derive(Debug)]
pub(crate) struct SurfacePatch {
    vertices: [DVec3; (CHUNK_SIZE + 1) * (CHUNK_SIZE + 1)],
}

const FACES: [Face; 6] = [Face::Back, Face::Front, Face::Left, Face::Right, Face::Top, Face::Bottom];

impl Default for SurfacePatches {
    fn default() -> Self {
        let patches = FACES.iter().map(|&face| {
            let patches: Vec<SurfacePatch> = (0..MAX_DEPTH).map(|level| SurfacePatch::generate(level as u8, face)).collect();
            let patches: [SurfacePatch; MAX_DEPTH] = patches.try_into().unwrap();
            (face, patches)
        }).collect();

        Self { patches }
    }
}

impl SurfacePatches {
    pub fn get_patch(&self, level: u8, face: Face) -> &SurfacePatch {
        &self.patches.get(&face).unwrap()[level as usize]
    }
}

impl SurfacePatch {
    pub fn generate(level: u8, face: Face) -> Self {
        let chunk_span = 2.0f64.powi(-(level as i32));
        let step_size = chunk_span * 2.0 / CHUNK_SIZE as f64;

        let (axis_a, axis_b) = face.perpendicular_vectors();
        let axis_a = DVec3::from(axis_a);
        let axis_b = DVec3::from(axis_b);

        let mut vertices: Vec<DVec3> = Vec::with_capacity((CHUNK_SIZE + 1) * (CHUNK_SIZE + 1));
        for y in 0..CHUNK_SIZE + 1 {
            for x in 0..CHUNK_SIZE + 1 {
                let position_a = (x as f64 * step_size) - chunk_span;
                let position_b = (y as f64 * step_size) - chunk_span;
                let vertex = axis_a * position_a + axis_b * position_b;
                vertices.push(vertex);
            }
        }

        Self {
            vertices: vertices.try_into().unwrap(),
        }
    }

    pub fn get_vertex(&self, x: usize, y: usize) -> &DVec3 {
        &self.vertices[y * (CHUNK_SIZE + 1) + x]
    }
}