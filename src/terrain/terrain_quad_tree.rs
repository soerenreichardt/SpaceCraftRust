use std::sync::Mutex;

use bevy::prelude::{Commands, Component, Query, Transform, With};

use crate::datastructures::quad_tree::{InitializeNode, QuadTree};
use crate::MainCamera;

pub(crate) trait DistanceFunction {
    fn distance() -> f32;
}

#[derive(Copy, Clone)]
pub(crate) struct TerrainQuadTreeConfig {
    pub(crate) lod_threshold: f32
}

#[derive(Default)]
pub(crate) struct TerrainQuadTreeNode;

#[derive(Component)]
pub(crate) struct TerrainQuadTree {
    quad_tree: Mutex<QuadTree<TerrainQuadTreeNode>>,
    config: TerrainQuadTreeConfig
}

impl TerrainQuadTree {

    pub(crate) fn new(config: TerrainQuadTreeConfig) -> Self {
        TerrainQuadTree {
            quad_tree: Mutex::new(QuadTree::default()),
            config
        }
    }

    pub(crate) fn update(&mut self, command: &mut Commands, distance: f32) {
        println!("update {}", distance);
        if distance < self.config.lod_threshold {
            if !self.quad_tree.lock().unwrap().has_children() {
                self.quad_tree.get_mut().expect("Could not acquire write lock").split();
            }
        } else {
            self.quad_tree.lock().unwrap().merge();
        }
    }
}

impl InitializeNode for TerrainQuadTreeNode {
    fn new() -> TerrainQuadTreeNode {
        TerrainQuadTreeNode {}
    }
}

pub(crate) fn update(mut command: Commands, mut query: Query<(&mut TerrainQuadTree, &Transform)>, camera_query: Query<(&Transform, With<MainCamera>)>) {
    let camera_transform = camera_query.iter().next().expect("No camera found").0;
    for (mut terrain_quad_tree, transform) in query.iter_mut() {
        let distance = (camera_transform.translation - transform.translation).length();
        terrain_quad_tree.update(&mut command, distance);
    }
}
