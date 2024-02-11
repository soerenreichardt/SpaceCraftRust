use std::sync::{Arc, RwLock};

use bevy::prelude::*;
use bevy::render::mesh::{Indices, PrimitiveTopology};
use concurrent_queue::ConcurrentQueue;
use rand::Rng;

use crate::terrain::terrain_quad_tree::TerrainQuadTreeNode;

const QUEUE_CAPACITY: usize = 10000;
const MESH_SIZE: usize = 16;

#[derive(Resource)]
pub(crate) struct MeshGenerator {
    indices: Vec<u32>,
    queue: ConcurrentQueue<Request>,
}

#[derive(Debug)]
pub(crate) enum RequestKind {
    Create,
    Remove,
}

#[derive(Debug)]
pub(crate) struct Request {
    kind: RequestKind,
    node: Arc<RwLock<TerrainQuadTreeNode>>,
}

impl Request {
    pub(crate) fn create(node: Arc<RwLock<TerrainQuadTreeNode>>) -> Self {
        Request { kind: RequestKind::Create, node }
    }

    pub(crate) fn remove(node: Arc<RwLock<TerrainQuadTreeNode>>) -> Self {
        Request { kind: RequestKind::Remove, node }
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
            queue: ConcurrentQueue::bounded(QUEUE_CAPACITY),
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
                let mesh = compute_mesh(node.center, node.length, node.face.perpendicular_vectors(), &generator, &mut meshes, &mut materials);
                commands.entity(entity).insert(mesh);
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
    perpendicular_vectors: (Vec3, Vec3),
    mesh_generator: &ResMut<MeshGenerator>,
    meshes: &mut ResMut<Assets<Mesh>>,
    materials: &mut ResMut<Assets<StandardMaterial>>,
) -> PbrBundle {
    let mut mesh = Mesh::new(PrimitiveTopology::TriangleList);
    let step_size = length / MESH_SIZE as f32;
    let offset = length / 2.0;
    let (axis_a, axis_b) = perpendicular_vectors;
    let (offset_a, offset_b) = (axis_a * offset, axis_b * offset);

    let mut vertices: Vec<[f32; 3]> = Vec::with_capacity((MESH_SIZE + 1) * (MESH_SIZE + 1));
    for y in 0..MESH_SIZE + 1 {
        for x in 0..MESH_SIZE + 1 {
            let vertex = axis_a * (x as f32 * step_size) + axis_b * (y as f32 * step_size) - offset_a - offset_b;
            vertices.push(vertex.into());
        }
    }
    mesh.set_indices(Some(Indices::U32(mesh_generator.indices.clone())));
    mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, vertices);

    let mut rng = rand::thread_rng();
    PbrBundle {
        mesh: meshes.add(mesh),
        material: materials.add(StandardMaterial {
            base_color: Color::rgb(rng.gen(), rng.gen(), rng.gen()),
            ..default()
        }),
        transform: Transform::from_translation(center),
        visibility: Visibility::Visible,
        ..default()
    }
}