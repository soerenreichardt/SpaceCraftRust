use bevy::prelude::*;
use bevy::render::mesh::{Indices, PrimitiveTopology};
use bevy::render::render_asset::RenderAssetUsages;
use rand::random;
use crate::terrain::Face;

pub(crate) const MESH_SIZE: usize = 16;
const QUEUE_CAPACITY: usize = 10000;

#[derive(Resource)]
pub(crate) struct MeshGenerator2 {
    indices: Vec<u32>
}

#[derive(Debug)]
pub(crate) enum RequestKind2 {
    Create,
    Remove,
}

impl Default for MeshGenerator2 {
    fn default() -> Self {
        let indices = std::str::from_utf8(include_bytes!("../../resources/indices")).unwrap()
            .split(',')
            .map(|s| s.trim().parse().unwrap())
            .collect();
        MeshGenerator2 {
            indices
        }
    }
}

impl MeshGenerator2 {
    pub(crate) fn generate_mesh(&self, center: Vec3, length: f32, face: Face, scale: f32, meshes: &mut Assets<Mesh>, materials: &mut Assets<StandardMaterial>) -> PbrBundle {
        compute_mesh(center, length, face, scale, self.indices.clone(), meshes, materials)
    }
}

pub(crate) fn compute_mesh(
    center: Vec3,
    length: f32,
    face: Face,
    scale: f32,
    indices: Vec<u32>,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
) -> PbrBundle {
    let mut mesh = Mesh::new(PrimitiveTopology::TriangleList, RenderAssetUsages::all());
    let step_size = length / MESH_SIZE as f32;
    let offset = length / 2.0;
    let (axis_a, axis_b) = face.perpendicular_vectors();
    let (offset_a, offset_b) = (axis_a * offset, axis_b * offset);

    let mut vertices: Vec<[f32; 3]> = Vec::with_capacity((MESH_SIZE + 1) * (MESH_SIZE + 1));
    for y in 0..MESH_SIZE + 1 {
        for x in 0..MESH_SIZE + 1 {
            let vertex = axis_a * (x as f32 * step_size) + axis_b * (y as f32 * step_size) - offset_a - offset_b + center;
            let vertex = vertex.normalize() * scale;
            vertices.push(vertex.clone().into());
        }
    }

    mesh.insert_indices(Indices::U32(indices));
    mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, vertices);
    mesh.duplicate_vertices();
    mesh.compute_flat_normals();

    PbrBundle {
        mesh: meshes.add(mesh),
        material: materials.add(StandardMaterial {
            base_color: Color::linear_rgb(random::<f32>(), random::<f32>(), random::<f32>()),
            ..default()
        }),
        visibility: Visibility::Visible,
        ..default()
    }
}

#[inline(always)]
fn transform_vertex_for_noise(vertex: Vec3, scale: f32) -> [f64; 3] {
    [
        vertex.x as f64 / scale as f64,
        vertex.y as f64 / scale as f64,
        vertex.z as f64 / scale as f64
    ]
}