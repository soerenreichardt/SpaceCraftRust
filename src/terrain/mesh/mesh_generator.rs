use crate::terrain::block::BlockType;
use crate::terrain::chunk::{Blocks, Chunk};
use crate::terrain::generate::TerrainGenerator;
use crate::terrain::mesh::base_plane::CurvedBasePlane;
use crate::terrain::planet::Planet;
use crate::terrain::terrain_octree::RemoveTerrainChildren;
use crate::terrain::CHUNK_SIZE;
use bevy::pbr::wireframe::Wireframe;
use bevy::prelude::*;
use bevy::render::mesh::{Indices, PrimitiveTopology};
use bevy::render::render_asset::RenderAssetUsages;
use crate::terrain::mesh::cube::{SurfacePatch, SurfacePatches};

pub fn generate_chunk_meshes(
    mut commands: Commands,
    mut query: Query<(Entity, &mut Chunk), Added<Chunk>>,
    planets: Query<&Planet>,
    mut parents_query: Query<&mut Visibility>,
    mesh_generator: Res<MeshGenerator>,
    surface_patches: Res<SurfacePatches>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    for (entity, mut chunk) in query.iter_mut() {
        let Ok(planet) = planets.get(chunk.planet_entity) else {
            continue;
        };
        let surface_patch = surface_patches.get_patch(chunk.level, chunk.face);
        let terrain_generator = &planet.terrain_generator;
        
        let render_bundle = mesh_generator.compute_mesh(&mut chunk, surface_patch, terrain_generator, &mut meshes, &mut materials);
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
        surface_patch: &SurfacePatch,
        terrain_generator: &TerrainGenerator,
        meshes: &mut Assets<Mesh>,
        materials: &mut Assets<StandardMaterial>,
    ) -> impl Bundle {
        let base_plane = CurvedBasePlane::compute_for_chunk(chunk, surface_patch);

        chunk.blocks = Some(Self::fill_blocks(&base_plane, terrain_generator, chunk));
        let mut mesh = Mesh::new(PrimitiveTopology::TriangleList, RenderAssetUsages::all());
        let vertices = base_plane.vertices();
        mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, vertices);

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
            Visibility::Visible,
            Wireframe,
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