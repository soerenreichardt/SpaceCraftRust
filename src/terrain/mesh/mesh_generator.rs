use bevy::math::DVec3;
use crate::terrain::chunk::{Blocks, Chunk};
use crate::terrain::mesh::base_plane::CurvedBasePlane;
use crate::terrain::terrain_octree::RemoveTerrainChildren;
use bevy::pbr::wireframe::Wireframe;
use bevy::prelude::*;
use bevy::render::mesh::{Indices, PrimitiveTopology};
use bevy::render::render_asset::RenderAssetUsages;
use crate::terrain::block::BlockType;
use crate::terrain::Face;
use crate::terrain::generate::TerrainGenerator;
use crate::terrain::planet::Planet;

pub(crate) const MESH_SIZE: usize = 16;

pub fn generate_chunk_meshes(
    mut commands: Commands,
    mut query: Query<(Entity, &mut Chunk), Added<Chunk>>,
    planets: Query<&Planet>,
    mut parents_query: Query<&mut Visibility>,
    mesh_generator: Res<MeshGenerator>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>
) {
    for (entity, mut chunk) in query.iter_mut() {
        // Mesh only the radial slice adjacent to the surface plane (gz = scale/2,
        // where the sphere crosses the face volume). The old `z.0 == 0` gate picked
        // the slice at the far end of the cube — the planet-core side on the positive
        // faces, outer space on the negative ones — whose sphere projection lands on
        // the cube's edge zones rather than the surface patch it renders at, so those
        // faces refined against the wrong reference point. The root (full range) stays
        // meshed so an unrefined face still renders.
        let scale = chunk.scale as u32;
        let radial = chunk.range.z(chunk.face.clone());
        let is_root = radial.0 == 0 && radial.1 == scale;
        let is_surface_slice = radial.0 == scale / 2;
        if !(is_root || is_surface_slice) {
            commands.entity(entity).insert(Visibility::Visible);
            continue;
        }

        let Ok(planet) = planets.get(chunk.planet_entity) else {
            continue;
        };
        let terrain_generator = &planet.terrain_generator;

        let render_bundle = mesh_generator.compute_mesh(&mut chunk, terrain_generator, &mut meshes, &mut materials);
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
    ) -> impl Bundle {
        let base_plane = CurvedBasePlane::compute_for_chunk(chunk);
        chunk.blocks = Some(Self::fill_blocks(&base_plane, terrain_generator, chunk.face.clone(), chunk.length));
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
            Visibility::Visible,
            Wireframe
        )
    }

    pub(crate) fn fill_blocks(base_plane: &CurvedBasePlane, terrain_generator: &TerrainGenerator, face: Face, length: f32) -> Blocks {
        let (axis_a, axis_b) = face.perpendicular_vectors();
        let axis_a = DVec3::from(axis_a);
        let axis_b = DVec3::from(axis_b);
        let axis_up = axis_a.cross(axis_b);
        let step_size = (length / MESH_SIZE as f32) as f64;
        let offset_length = step_size / 2.0;
        let (offset_x, offset_y) = (axis_a * offset_length, axis_b + offset_length);

        let mut blocks = Blocks::default();

        for z in 0..MESH_SIZE {
            for y in 0..MESH_SIZE {
                for x in 0..MESH_SIZE {
                    let vertex = base_plane.vertex_at(x, y);
                    let sample_position = (vertex + offset_x + offset_y) + axis_up * step_size;
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