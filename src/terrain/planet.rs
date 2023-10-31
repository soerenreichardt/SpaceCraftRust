use bevy::asset::Assets;
use bevy::hierarchy::BuildChildren;
use bevy::pbr::StandardMaterial;
use bevy::prelude::{Commands, Component, default, Mesh, ResMut, SpatialBundle, Transform, Visibility};

use crate::terrain::terrain_quad_tree::{RenderedTerrainBundle, TerrainQuadTree, TerrainQuadTreeChild};

#[derive(Component)]
pub(crate) struct Planet {
    terrain_faces: [TerrainQuadTreeChild; 6],
    radius: u8
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
    pub(crate) fn spawn(commands: &mut Commands, meshes: &mut ResMut<Assets<Mesh>>, materials: &mut ResMut<Assets<StandardMaterial>>) {
        let planet = Planet::new(2);
        let terrain_components = planet.terrain_faces.iter().map(|terrain_face| {
            let render_component = TerrainQuadTree::compute_mesh(terrain_face.0.read().unwrap().node.center.clone(), meshes, materials);
            let rendered_terrain = RenderedTerrainBundle { terrain_component: terrain_face.into(), render_component };
            commands.spawn::<RenderedTerrainBundle>(rendered_terrain).id()
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

        Planet { terrain_faces, radius }
    }
}
