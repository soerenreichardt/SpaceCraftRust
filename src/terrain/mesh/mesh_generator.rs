use bevy::math::DVec3;
use crate::terrain::chunk::{Blocks, Chunk};
use crate::terrain::mesh::base_plane::CurvedBasePlane;
use crate::terrain::terrain_octree::RemoveTerrainChildren;
use bevy::pbr::wireframe::Wireframe;
use bevy::prelude::*;
use bevy::render::mesh::{Indices, PrimitiveTopology};
use bevy::render::render_asset::RenderAssetUsages;
use crate::terrain::block::BlockType;
use crate::terrain::CHUNK_SIZE;
use crate::terrain::generate::TerrainGenerator;
use crate::terrain::planet::Planet;

pub fn generate_chunk_meshes(
    mut commands: Commands,
    mut query: Query<(Entity, &mut Chunk), Added<Chunk>>,
    planets: Query<&Planet>,
    mut parents_query: Query<&mut Visibility>,
    mesh_generator: Res<MeshGenerator>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut gizmo_assets: ResMut<Assets<GizmoAsset>>,
) {
    for (entity, mut chunk) in query.iter_mut() {
        let Ok(planet) = planets.get(chunk.planet_entity) else {
            continue;
        };
        let terrain_generator = &planet.terrain_generator;

        let render_bundle = mesh_generator.compute_mesh(&mut chunk, terrain_generator, &mut meshes, &mut materials, &mut gizmo_assets);
        commands.entity(entity).insert(render_bundle);

        if let Some(parent) = chunk.parent {
            let mut visibility = parents_query.get_mut(parent).expect("Parent not found");
            *visibility = Visibility::Hidden;
        }
    }
}

pub fn restore_visibility(mut commands: Commands, mut remove_children_events: EventReader<RemoveTerrainChildren>, mut query: Query<&mut Visibility>) {
    for event in remove_children_events.read() {
        let entity = event.0;
        let mut visibility = query.get_mut(entity).expect("Entity not found");
        *visibility = Visibility::Visible;
        commands.entity(entity).despawn_related::<Children>();
    }
}

#[derive(Resource)]
pub(crate) struct MeshGenerator {
    indices: Vec<u32>,
    flipped_indices: Vec<u32>
}

impl Default for MeshGenerator {
    fn default() -> Self {
        let indices: Vec<u32> = std::str::from_utf8(include_bytes!("../../../resources/indices")).unwrap()
            .split(',')
            .map(|s| s.trim().parse().unwrap())
            .collect();
        let flipped_indices = indices
            .chunks_exact(3)
            .flat_map(|t| [t[0], t[2], t[1]])
            .collect();
        MeshGenerator {
            indices,
            flipped_indices
        }
    }
}

impl MeshGenerator {
    pub(crate) fn compute_mesh(
        &self,
        chunk: &mut Chunk,
        terrain_generator: &TerrainGenerator,
        meshes: &mut Assets<Mesh>,
        materials: &mut Assets<StandardMaterial>,
        gizmo_assets: &mut Assets<GizmoAsset>,
    ) -> impl Bundle {
        let mut gizmo = GizmoAsset::default();

        let base_plane = CurvedBasePlane::compute_for_chunk(chunk);
        gizmo.sphere(base_plane.vertex_at(8, 8).as_vec3(), 0.1, Color::WHITE);
        chunk.blocks = Some(Self::fill_blocks(&base_plane, terrain_generator, chunk));
        let mut mesh = Mesh::new(PrimitiveTopology::TriangleList, RenderAssetUsages::all());
        let (vertices, colors) = base_plane.data();
        mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, vertices);
        mesh.insert_attribute(Mesh::ATTRIBUTE_COLOR, colors);

        let (axis_a, axis_b) = chunk.face.perpendicular_vectors();
        let flip = axis_a.cross(axis_b).dot(chunk.face.direction_vector()) > 0.0;
        let indices = if flip { &self.flipped_indices } else { &self.indices };
        mesh.insert_indices(Indices::U32(indices.clone()));
        mesh.compute_normals();

        (
            Mesh3d(meshes.add(mesh)),
            MeshMaterial3d(materials.add(StandardMaterial {
                base_color: Color::WHITE,
                ..default()
            })),
            Visibility::Hidden,
            Wireframe,
            Gizmo {
                handle: gizmo_assets.add(gizmo),
                line_config: GizmoLineConfig {
                    width: 4.,
                    ..default()
                },
                ..default()
            }
        )
    }

    pub(crate) fn fill_blocks(base_plane: &CurvedBasePlane, terrain_generator: &TerrainGenerator, chunk: &Chunk) -> Blocks {
        let step_size = (chunk.length / CHUNK_SIZE as f32) as f64;
        let scale = chunk.scale as f64;
        let mut blocks = Blocks::default();

        for z in 0..CHUNK_SIZE {
            for y in 0..CHUNK_SIZE {
                for x in 0..CHUNK_SIZE {
                    let middle_point = base_plane.middle_point_at(x, y);
                    let depth = (z as f64 + 0.5) * step_size;
                    let sample_position = middle_point.normalize() * (scale - depth).max(0.0);
                    let noise_value = terrain_generator.sample(sample_position);
                    if noise_value > 0.0 {
                        blocks.set(BlockType::Stone, x, y, z);
                    } else {
                        blocks.set(BlockType::Air, x, y, z);
                    }
                }
            }
        }

        blocks
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::terrain::chunk::Chunk;
    use crate::terrain::terrain_octree::NodeRange;
    use crate::terrain::Face;

    /// A chunk in the geometry that actually gets meshed: the surface slice
    /// (radial range [scale/2, scale]), whose length matches the scale.
    fn test_chunk(center: Vec3) -> Chunk {
        Chunk {
            parent: None,
            center,
            face: Face::Top,
            length: 32.0,
            scale: 32.0,
            planet_entity: Entity::PLACEHOLDER,
            blocks: None,
            range: NodeRange::from_scale(32.0),
        }
    }

    fn stone_count_at_layer(blocks: &Blocks, z: usize) -> usize {
        (0..CHUNK_SIZE)
            .map(|y| {
                (0..CHUNK_SIZE)
                    .filter(|x| blocks.get(*x, y, z) == BlockType::Stone)
                    .count()
            })
            .sum()
    }

    #[test]
    fn fill_blocks_produces_a_realistic_world_space_mix() {
        let generator = TerrainGenerator::new(32.0, 0);
        let chunk = test_chunk(Vec3::Y * 32.0);
        let base_plane = CurvedBasePlane::compute_for_chunk(&chunk);
        let blocks = MeshGenerator::fill_blocks(&base_plane, &generator, &chunk);

        // At the surface (z = 0, radius ≈ sea level) the noise decides —
        // a mix of stone and air, not a uniform verdict.
        let surface = stone_count_at_layer(&blocks, 0);
        assert!(surface > 0, "no stone at the surface layer");
        assert!(surface < CHUNK_SIZE * CHUNK_SIZE, "surface layer fully solid");

        // The deepest layer sits near the planet center: all stone.
        let deep = stone_count_at_layer(&blocks, CHUNK_SIZE - 1);
        assert_eq!(deep, CHUNK_SIZE * CHUNK_SIZE, "deep layer not fully solid");

        // Some stone, some air overall.
        let total_stone: usize = (0..CHUNK_SIZE).map(|z| stone_count_at_layer(&blocks, z)).sum();
        assert!(total_stone > 0, "no stone at all");
        assert!(total_stone < CHUNK_SIZE * CHUNK_SIZE * CHUNK_SIZE, "fully solid");

        // A chunk at a different position must sample different terrain —
        // identical counts would mean the noise is being tiled per chunk.
        let other_chunk = test_chunk(Vec3::new(16.0, 32.0, 0.0));
        let other_plane = CurvedBasePlane::compute_for_chunk(&other_chunk);
        let other_blocks = MeshGenerator::fill_blocks(&other_plane, &generator, &other_chunk);
        let other_total: usize = (0..CHUNK_SIZE).map(|z| stone_count_at_layer(&other_blocks, z)).sum();
        assert_ne!(total_stone, other_total, "identical stone counts — terrain is tiled per chunk");
    }
}