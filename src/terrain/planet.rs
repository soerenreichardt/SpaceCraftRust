use bevy::hierarchy::BuildChildren;
use bevy::prelude::{Commands, Component, default, ResMut, SpatialBundle, Transform, Visibility};

use crate::terrain::mesh_generator::{MeshGenerator, Request};
use crate::terrain::terrain_quad_tree::{TerrainQuadTree, TerrainQuadTreeChild, TerrainQuadTreeComponent};

#[derive(Component)]
pub(crate) struct Planet {
    terrain_faces: [TerrainQuadTreeChild; 6],
}

#[derive(Clone, Debug)]
pub(crate) enum Face {
    Top,
    Bottom,
    Left,
    Right,
    Front,
    Back,
}

impl Planet {
    pub(crate) fn spawn(commands: &mut Commands, mesh_generator: &mut ResMut<MeshGenerator>) {
        let planet = Planet::new(4);
        let terrain_components = planet.terrain_faces.iter().map(|terrain_face| {
            let entity = commands.spawn::<TerrainQuadTreeComponent>(terrain_face.into()).id();
            let node = terrain_face.0.write().unwrap().node.clone();
            node.write().unwrap().entity = Some(entity);
            mesh_generator.queue_generate_mesh_request(Request::create(node));
            entity
        }).collect::<Vec<_>>();

        let mut planet = commands.spawn((planet, SpatialBundle { transform: Transform::default(), visibility: Visibility::Visible, ..default() }));
        planet.push_children(&terrain_components);
    }

    fn new(radius: u8) -> Self {
        let terrain_faces: [TerrainQuadTreeChild; 6] = [
            TerrainQuadTree::root(Face::Top, radius).into(),
            TerrainQuadTree::root(Face::Bottom, radius).into(),
            TerrainQuadTree::root(Face::Left, radius).into(),
            TerrainQuadTree::root(Face::Right, radius).into(),
            TerrainQuadTree::root(Face::Front, radius).into(),
            TerrainQuadTree::root(Face::Back, radius).into()
        ];

        Planet { terrain_faces }
    }
}
