use std::sync::{Arc, RwLock};

use bevy::prelude::*;
use bevy::render::mesh::{Indices, PrimitiveTopology};
use bevy::render::render_asset::RenderAssetUsages;
use concurrent_queue::ConcurrentQueue;
use noise::{BasicMulti, NoiseFn, Perlin};

use crate::terrain::planet::Face;
use crate::terrain::terrain_quad_tree::TerrainQuadTreeNode;

pub(crate) const MESH_SIZE: usize = 16;
const QUEUE_CAPACITY: usize = 10000;

#[derive(Resource)]
pub(crate) struct MeshGenerator {
    indices: Vec<u32>,
    queue: ConcurrentQueue<Request>
}

#[derive(Component)]
pub(crate) struct MeshAvailable;

#[derive(Debug)]
pub(crate) enum RequestKind {
    Create,
    Remove,
}

#[derive(Debug)]
pub(crate) struct Request {
    kind: RequestKind,
    node: Arc<RwLock<TerrainQuadTreeNode>>,
    scale: f32,
}

impl Request {
    pub(crate) fn create(node: Arc<RwLock<TerrainQuadTreeNode>>, scale: f32) -> Self {
        Request { kind: RequestKind::Create, node, scale }
    }

    pub(crate) fn remove(node: Arc<RwLock<TerrainQuadTreeNode>>) -> Self {
        Request { kind: RequestKind::Remove, node, scale: 0.0 }
    }
}

impl MeshGenerator {
    pub(crate) fn new() -> Self {
        let indices = std::str::from_utf8(include_bytes!("../../resources/indices")).unwrap()
            .split(',')
            .map(|s| s.trim().parse().unwrap())
            .collect();
        MeshGenerator {
            indices,
            queue: ConcurrentQueue::bounded(QUEUE_CAPACITY)
        }
    }

    pub(crate) fn queue_generate_mesh_request(&self, request: Request) {
        self.queue.push(request).expect("Failed to queue request")
    }
}

pub(crate) fn update(generator: ResMut<MeshGenerator>, mut commands: Commands, mut meshes: ResMut<Assets<Mesh>>, mut materials: ResMut<Assets<StandardMaterial>>) {
    while let Ok(request) = generator.queue.pop() {
        match request.kind {
            RequestKind::Create => {
                let node = request.node.read().unwrap();
                let entity = node.entity.unwrap();
                let mesh = compute_mesh(
                    node.center,
                    node.length,
                    node.face.clone(),
                    &node.noise,
                    request.scale,
                    &generator,
                    &mut meshes,
                    &mut materials
                );
                commands.entity(entity).insert((mesh, MeshAvailable));
            }
            RequestKind::Remove => {
                let entity = request.node.read().unwrap().entity.unwrap();
                commands.entity(entity).despawn_recursive();
            }
        }
    }
}

fn compute_mesh(
    center: Vec3,
    length: f32,
    face: Face,
    noise: &BasicMulti<Perlin>,
    scale: f32,
    mesh_generator: &ResMut<MeshGenerator>,
    meshes: &mut ResMut<Assets<Mesh>>,
    materials: &mut ResMut<Assets<StandardMaterial>>,
) -> PbrBundle {
    let mut mesh = Mesh::new(PrimitiveTopology::TriangleList, RenderAssetUsages::all());
    let step_size = length / MESH_SIZE as f32;
    let offset = length / 2.0;
    let (axis_a, axis_b) = face.perpendicular_vectors();
    let (offset_a, offset_b) = (axis_a * offset, axis_b * offset);

    let mut vertices: Vec<[f32; 3]> = Vec::with_capacity((MESH_SIZE + 1) * (MESH_SIZE + 1));
    let mut normals: Vec<[f32; 3]> = Vec::with_capacity(MESH_SIZE * MESH_SIZE);
    for y in 0..MESH_SIZE + 1 {
        for x in 0..MESH_SIZE + 1 {
            let vertex = axis_a * (x as f32 * step_size) + axis_b * (y as f32 * step_size) - offset_a - offset_b + center;

            let noise = noise.get(transform_vertex_for_noise(vertex, scale));

            let vertex = vertex.normalize() * scale;
            let vertex = vertex * (1.0 + (noise as f32 * 0.1));
            vertices.push(vertex.clone().into());
        }
    }

    mesh.insert_indices(Indices::U32(mesh_generator.indices.clone()));
    mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, vertices);
    mesh.duplicate_vertices();
    mesh.compute_flat_normals();

    PbrBundle {
        mesh: meshes.add(mesh),
        material: materials.add(StandardMaterial {
            base_color: Color::rgb(181.0 / 255.0, 144.0 / 255.0, 11.0 / 255.0),
            ..default()
        }),
        visibility: Visibility::Hidden,
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