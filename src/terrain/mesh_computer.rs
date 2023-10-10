use std::sync::{Arc, RwLock};
use bevy::asset::Assets;
use bevy::pbr::{PbrBundle, StandardMaterial};

use bevy::prelude::{BuildChildren, Color, Commands, default, Mesh, ResMut, Resource, Transform, Visibility};
use bevy::prelude::shape::Icosphere;
use concurrent_queue::ConcurrentQueue;

use crate::datastructures::quad_tree::QuadTree;
use crate::terrain::terrain_quad_tree::TerrainQuadTreeNode;

#[derive(Clone, Debug, Resource)]
pub(crate) struct MeshComputer {
    pub(crate) queue: Arc<ConcurrentQueue<MeshRequest>>
}

impl Default for MeshComputer {
    fn default() -> Self {
        MeshComputer {
            queue: Arc::new(ConcurrentQueue::bounded(1000))
        }
    }
}

impl MeshComputer {
    pub(crate) fn schedule(&self, request: MeshRequest) {
        self.queue.push(request).unwrap();
    }
}

#[derive(Debug)]
pub(crate) struct MeshRequest {
    pub(crate) node: Arc<RwLock<QuadTree<TerrainQuadTreeNode>>>
}

pub(crate) fn compute_meshes(
    mut mesh_computer: ResMut<MeshComputer>,
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>
) {
    if !mesh_computer.queue.is_empty() {
        let request = mesh_computer.queue.pop().unwrap();
        let node = request.node.read().unwrap();

        let center = node.node.center;

        let mut entity_commands = commands.spawn(PbrBundle {
            mesh: meshes.add(Mesh::try_from(Icosphere { radius: 0.1, subdivisions: 2 }).unwrap()),
            material: materials.add(StandardMaterial {
                base_color: Color::hex("#ffd891").unwrap(),
                ..default()
            }),
            transform: Transform::from_translation(center),
            visibility: Visibility::Visible,
            ..default()
        });

        drop(node);
        let mut node = request.node.write().unwrap();
        entity_commands.set_parent(node.parent().unwrap().node.entity.unwrap());
        node.node.entity = Some(entity_commands.id());
    } else {
        return;
    }
}
