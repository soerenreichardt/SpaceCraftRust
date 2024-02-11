use bevy::hierarchy::BuildChildren;
use bevy::prelude::{Commands, Component, default, ResMut, SpatialBundle, Transform, Vec3, Visibility};

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

impl Face {
    pub(crate) fn direction_vector(&self) -> Vec3 {
        match self {
            Face::Top => Vec3::Y,
            Face::Bottom => -Vec3::Y,
            Face::Left => -Vec3::X,
            Face::Right => Vec3::X,
            Face::Front => -Vec3::Z,
            Face::Back => Vec3::Z,
        }
    }

    pub(crate) fn perpendicular_vectors(&self) -> (Vec3, Vec3) {
        match self {
            Face::Top => (Vec3::X, Vec3::Z),
            Face::Bottom => (Vec3::X, -Vec3::Z),
            Face::Left => (Vec3::Y, Vec3::Z),
            Face::Right => (Vec3::Y, -Vec3::Z),
            Face::Front => (Vec3::X, Vec3::Y),
            Face::Back => (Vec3::X, -Vec3::Y),
        }
    }
}

impl Planet {
    pub(crate) fn spawn(commands: &mut Commands, mesh_generator: &mut ResMut<MeshGenerator>) {
        let radius = 6;
        let planet = Planet::new(radius);
        let terrain_components = planet.terrain_faces.iter().map(|terrain_face| {
            let entity = commands.spawn::<TerrainQuadTreeComponent>(terrain_face.into()).id();
            let node = terrain_face.0.write().unwrap().node.clone();
            node.write().unwrap().entity = Some(entity);
            mesh_generator.queue_generate_mesh_request(Request::create(node, radius as f32));
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
