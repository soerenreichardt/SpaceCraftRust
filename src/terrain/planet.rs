use bevy::prelude::Component;
use crate::terrain::terrain_quad_tree::{TerrainQuadTree, TerrainQuadTreeConfig};

#[derive(Component)]
pub(crate) struct Planet {
    terrain_faces: [TerrainQuadTree; 6],
}

impl Planet {
    pub(crate) fn new(config: TerrainQuadTreeConfig) -> Self {
        Planet {
            terrain_faces: [
                TerrainQuadTree::new(config),
                TerrainQuadTree::new(config),
                TerrainQuadTree::new(config),
                TerrainQuadTree::new(config),
                TerrainQuadTree::new(config),
                TerrainQuadTree::new(config)
            ]
        }
    }
}