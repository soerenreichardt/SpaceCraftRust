use crate::terrain::chunk::Chunk;
use crate::terrain::generate::{debug_ascii_slice, fill_blocks, FaceGrid, DEBUG_FILL_SLICE};
use crate::terrain::mesh::base_plane::CurvedBasePlane;
use crate::terrain::planet::Planet;
use crate::terrain::terrain_octree::RemoveTerrainChildren;
use bevy::pbr::wireframe::Wireframe;
use bevy::prelude::*;
use bevy::render::mesh::{Indices, PrimitiveTopology};
use bevy::render::render_asset::RenderAssetUsages;

pub(crate) const MESH_SIZE: usize = 16;

pub fn generate_chunk_meshes(
    mut commands: Commands,
    mut query: Query<(Entity, &mut Chunk), Added<Chunk>>,
    planets: Query<&Planet>,
    mut parents_query: Query<&mut Visibility>,
    mesh_generator: Res<MeshGenerator>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut slice_logged: Local<bool>,
) {
    for (entity, mut chunk) in query.iter_mut() {
        // Populate the chunk's blocks from its planet's terrain signature.
        if let Ok(planet) = planets.get(chunk.planet) {
            let grid = FaceGrid::new(&chunk.face, chunk.scale);
            if DEBUG_FILL_SLICE && !*slice_logged
                && planet.terrain_generator.chunk_intersects_surface(&chunk)
            {
                *slice_logged = true;
                debug_ascii_slice(&planet.terrain_generator, &grid, &chunk);
            }
            chunk.blocks = Some(fill_blocks(&planet.terrain_generator, &grid, &chunk));
        }

        let scale = chunk.scale as u32;
        let radial = chunk.range.z(chunk.face.clone());
        let is_root = radial.0 == 0 && radial.1 == 2 * scale;
        let is_surface_slice = radial.0 == scale;
        if !(is_root || is_surface_slice) {
            commands.entity(entity).insert(Visibility::Visible);
            continue;
        }
        let render_bundle = mesh_generator.compute_mesh(&chunk, &mut meshes, &mut materials);
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
        chunk: &Chunk,
        meshes: &mut Assets<Mesh>,
        materials: &mut Assets<StandardMaterial>,
    ) -> impl Bundle {
        let base_plane = CurvedBasePlane::compute_for_chunk(chunk);
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
}