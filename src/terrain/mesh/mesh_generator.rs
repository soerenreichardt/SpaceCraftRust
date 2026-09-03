use crate::terrain::chunk::Chunk;
use crate::terrain::generate::{
    debug_ascii_slice, fill_blocks, FaceGrid, TerrainGenerator, DEBUG_FILL_SLICE,
};
use crate::terrain::mesh::base_plane::CurvedBasePlane;
use crate::terrain::mesh::marching_cubes::{marching_cubes, McOutput, OccupancyView};
use crate::terrain::planet::Planet;
use crate::terrain::terrain_octree::RemoveTerrainChildren;
use bevy::pbr::wireframe::Wireframe;
use bevy::prelude::*;
use bevy::render::mesh::{Indices, PrimitiveTopology};
use bevy::render::render_asset::RenderAssetUsages;

pub(crate) const MESH_SIZE: usize = 16;

/// Which mesh a chunk renders.
enum MeshTier {
    /// The flat projected quad patch — the far tier. Dormant: `tier_for`
    /// does not select it yet (MC covers the whole surface at every level);
    /// distance/level thresholds will revive it.
    BasePlane,
    MarchingCubes,
}

/// Tier selection: band chunks carry the terrain surface at every level →
/// MC everywhere it matters; everything else is deep interior or empty
/// space → no mesh. (Every terrain surface point lies in some band chunk,
/// so this covers the planet.) Later: far → `BasePlane`, near → `Blocky`.
fn tier_for(chunk: &Chunk, generator: &TerrainGenerator) -> Option<MeshTier> {
    generator
        .chunk_intersects_surface(chunk)
        .then_some(MeshTier::MarchingCubes)
}

pub fn generate_chunk_meshes(
    mut commands: Commands,
    mut query: Query<(Entity, &mut Chunk), Added<Chunk>>,
    planets: Query<&Planet>,
    mut parents_query: Query<&mut Visibility>,
    mut mesh_generator: ResMut<MeshGenerator>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut slice_logged: Local<bool>,
) {
    for (entity, mut chunk) in query.iter_mut() {
        // Populate the chunk's blocks from its planet's terrain signature,
        // then mesh it according to its tier.
        if let Ok(planet) = planets.get(chunk.planet) {
            let generator = &planet.terrain_generator;
            let grid = FaceGrid::new(&chunk.face, chunk.scale);
            if DEBUG_FILL_SLICE && !*slice_logged && generator.chunk_intersects_surface(&chunk) {
                *slice_logged = true;
                debug_ascii_slice(generator, &grid, &chunk);
            }
            chunk.blocks = Some(fill_blocks(generator, &grid, &chunk));

            match tier_for(&chunk, generator) {
                Some(MeshTier::MarchingCubes) => {
                    mesh_generator.mc.clear();
                    let view = OccupancyView::build(generator, &grid, &chunk);
                    marching_cubes(&view, &grid, &chunk, &mut mesh_generator.mc);
                    if mesh_generator.mc.indices.is_empty() {
                        // Band chunk with no owned mixed cells — nothing to
                        // mesh (the parent's coarser mesh stays visible).
                        continue;
                    }
                    let render_bundle =
                        mesh_generator.compute_mc_mesh(&mut meshes, &mut materials);
                    commands.entity(entity).insert(render_bundle);
                }
                Some(MeshTier::BasePlane) => {
                    // Dormant far tier — the flat patch path.
                    let scale = chunk.scale as u32;
                    let radial = chunk.range.z(chunk.face.clone());
                    let is_root = radial.0 == 0 && radial.1 == 2 * scale;
                    let is_surface_slice = radial.0 == scale;
                    if !(is_root || is_surface_slice) {
                        continue;
                    }
                    let render_bundle =
                        mesh_generator.compute_mesh(&chunk, &mut meshes, &mut materials);
                    commands.entity(entity).insert(render_bundle);
                }
                None => {
                    // Deep interior or empty space — no mesh.
                    continue;
                }
            }
        } else {
            continue;
        }

        // Mesh bundles carry no Visibility: a deferred `Visibility::Visible`
        // in the bundle would overwrite the direct parent-hide written in
        // the same frame (roots mesh and get hidden by their same-frame
        // children — the "flat mesh behind the terrain" bug). Instead,
        // meshed chunks with a parent get an explicit Visible to punch
        // through their hidden (superseded) ancestor chain; roots keep
        // `Inherited`, which renders as hierarchy roots.
        if chunk.parent.is_some() {
            commands.entity(entity).insert(Visibility::Visible);
        }

        // A mesh was inserted — the parent's coarser mesh is superseded.
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
    flipped_indices: Vec<u32>,
    /// Reusable marching-cubes output, cleared per chunk.
    pub(crate) mc: McOutput,
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
            flipped_indices,
            mc: McOutput::default(),
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

        // No Visibility component here — the system inserts it explicitly
        // (a deferred Visible in the bundle would overwrite the same-frame
        // parent-hide; see generate_chunk_meshes).
        (
            Mesh3d(meshes.add(mesh)),
            MeshMaterial3d(materials.add(StandardMaterial {
                base_color: Color::WHITE,
                ..default()
            })),
            Wireframe
        )
    }

    /// Renders the last marching-cubes output (`mc` must have just been
    /// filled by `marching_cubes`). Vertices are chunk-local flat crossing
    /// positions; `compute_normals` for now (radial normals later).
    pub(crate) fn compute_mc_mesh(
        &self,
        meshes: &mut Assets<Mesh>,
        materials: &mut Assets<StandardMaterial>,
    ) -> impl Bundle {
        let mut mesh = Mesh::new(PrimitiveTopology::TriangleList, RenderAssetUsages::all());
        mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, self.mc.positions.clone());
        mesh.insert_indices(Indices::U32(self.mc.indices.clone()));
        mesh.compute_normals();

        // No Visibility component here — the system inserts it explicitly
        // (a deferred Visible in the bundle would overwrite the same-frame
        // parent-hide; see generate_chunk_meshes).
        (
            Mesh3d(meshes.add(mesh)),
            MeshMaterial3d(materials.add(StandardMaterial {
                base_color: Color::WHITE,
                ..default()
            })),
            Wireframe
        )
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use crate::terrain::generate::test_support;
    use crate::terrain::Face;

    /// Band chunks get MC; deep-interior and far-outside chunks get no mesh.
    #[test]
    fn tier_is_mc_only_for_band_chunks() {
        let scale = 256.0f32;
        let gen = TerrainGenerator::new(scale, 1);
        let root =
            test_support::chunk_at(&Face::Front, scale, (0, 512), (0, 512), (0, 512));
        assert!(matches!(
            tier_for(&root, &gen),
            Some(MeshTier::MarchingCubes)
        ));
        let inside =
            test_support::chunk_at(&Face::Front, scale, (200, 232), (200, 232), (40, 72));
        assert!(tier_for(&inside, &gen).is_none());
        let outside =
            test_support::chunk_at(&Face::Front, scale, (200, 232), (200, 232), (440, 472));
        assert!(tier_for(&outside, &gen).is_none());
    }
}

