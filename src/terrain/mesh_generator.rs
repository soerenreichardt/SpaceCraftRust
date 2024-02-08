use std::sync::{Arc, RwLock};

use bevy::prelude::*;
use bevy::prelude::shape::Icosphere;
use concurrent_queue::ConcurrentQueue;

use crate::terrain::terrain_quad_tree::TerrainQuadTreeNode;

const QUEUE_CAPACITY: usize = 10000;

#[derive(Resource)]
pub(crate) struct MeshGenerator {
    queue: ConcurrentQueue<Request>
}

#[derive(Debug)]
pub(crate) enum RequestKind {
    Create,
    Remove
}

#[derive(Debug)]
pub(crate) struct Request {
    kind: RequestKind,
    node: Arc<RwLock<TerrainQuadTreeNode>>
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
        MeshGenerator {
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
                let mesh = compute_mesh(node.center, &mut meshes, &mut materials);
                commands.entity(entity).insert(mesh);
            }
            RequestKind::Remove => {
                let entity = request.node.read().unwrap().entity.unwrap();
                commands.entity(entity).despawn_recursive();
            }
        }
    }
}

fn compute_mesh(center: Vec3, meshes: &mut ResMut<Assets<Mesh>>, materials: &mut ResMut<Assets<StandardMaterial>>) -> PbrBundle {
    PbrBundle {
        mesh: meshes.add(Mesh::try_from(Icosphere { radius: 0.1, subdivisions: 2 }).unwrap()),
        material: materials.add(StandardMaterial {
            base_color: Color::hex("#ffd891").unwrap(),
            ..default()
        }),
        transform: Transform::from_translation(center),
        visibility: Visibility::Visible,
        ..default()
    }
}