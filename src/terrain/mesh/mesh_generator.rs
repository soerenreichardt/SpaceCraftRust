use crate::terrain::chunk::Chunk;
use crate::terrain::generate::{
    debug_ascii_slice, fill_blocks, FaceGrid, TerrainGenerator, DEBUG_FILL_SLICE,
};
use crate::terrain::mesh::base_plane::CurvedBasePlane;
use crate::terrain::mesh::marching_cubes::{generate_chunk_payload, GeneratedChunk};
use crate::terrain::planet::Planet;
use crate::terrain::terrain_octree::RemoveTerrainChildren;
use bevy::pbr::wireframe::Wireframe;
use bevy::prelude::*;
use bevy::render::mesh::{Indices, PrimitiveTopology};
use bevy::render::render_asset::RenderAssetUsages;
use bevy::tasks::{AsyncComputeTaskPool, Task};
use std::future::Future;

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

/// A chunk's in-flight generation task (fill + occupancy view + marching
/// cubes) on the async compute pool. Dropped — and thereby canceled — with
/// the chunk if it merges before the result lands.
#[derive(Component)]
pub(crate) struct ChunkTask(Task<GeneratedChunk>);

/// Polls a finished task once and takes its output. Never blocks: an
/// unfinished task (or one whose output is not ready to be reaped) yields
/// `None` and is retried next frame.
fn try_take<T>(task: &mut Task<T>) -> Option<T> {
    if !task.is_finished() {
        return None;
    }
    let mut cx = std::task::Context::from_waker(std::task::Waker::noop());
    match std::pin::Pin::new(task).poll(&mut cx) {
        std::task::Poll::Ready(value) => Some(value),
        std::task::Poll::Pending => None,
    }
}

pub fn generate_chunk_meshes(
    mut commands: Commands,
    mut query: Query<(Entity, &mut Chunk), Added<Chunk>>,
    planets: Query<&Planet>,
    mut mesh_generator: Res<MeshGenerator>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut slice_logged: Local<bool>,
) {
    for (entity, mut chunk) in query.iter_mut() {
        // Resolve the chunk's planet and its terrain signature.
        let Ok(planet) = planets.get(chunk.planet) else {
            continue;
        };
        let generator = &planet.terrain_generator;
        let grid = FaceGrid::new(&chunk.face, chunk.scale);
        if DEBUG_FILL_SLICE && !*slice_logged && generator.chunk_intersects_surface(&chunk) {
            *slice_logged = true;
            debug_ascii_slice(generator, &grid, &chunk);
        }

        match tier_for(&chunk, generator) {
            Some(MeshTier::MarchingCubes) => {
                // Band chunk: generate off-thread. While the task is in
                // flight the parent's coarser mesh stays visible — no
                // holes, and no frame stalls on ~36k noise evaluations.
                // The payload is a pure function of (signature, lattice
                // window): fill, occupancy, and mesh all read one set of
                // samples.
                let generator = generator.clone();
                let grid = grid.clone();
                let snapshot = chunk.clone();
                let task = AsyncComputeTaskPool::get().spawn(async move {
                    generate_chunk_payload(&generator, &grid, &snapshot)
                });
                commands.entity(entity).insert(ChunkTask(task));
            }
            Some(MeshTier::BasePlane) => {
                // Dormant far tier (not selected yet) — the synchronous
                // flat-patch path.
                chunk.blocks = Some(fill_blocks(generator, &grid, &chunk));
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
                // Deep interior or empty space — uniform fill (all Stone /
                // all Air), cheap enough to stay synchronous; no mesh.
                chunk.blocks = Some(fill_blocks(generator, &grid, &chunk));
                continue;
            }
        }
    }
}

/// Applies finished generation tasks: block fills, meshes, and the
/// visibility/hide-parent rules — the same semantics the synchronous path
/// had, just deferred to when each result lands.
/// Results applied per frame at most — a cascade can finish hundreds of
/// tasks at once, and each application creates a mesh asset; bounding this
/// keeps the frame smooth while finished results drain over the next
/// frames (progressive refinement).
const MAX_RESULTS_PER_FRAME: usize = 16;

pub fn poll_chunk_tasks(
    mut commands: Commands,
    mut query: Query<(Entity, &mut Chunk, &mut ChunkTask)>,
    mut mesh_generator: ResMut<MeshGenerator>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    // Apply up to the throttle's worth of finished results: blocks and
    // mesh bundles. No visibility writes here at all — visibility is
    // recomputed from state every frame by `update_chunk_visibility`
    // (incremental hide/show bookkeeping repeatedly lost to
    // deferred-vs-direct write ordering races).
    let mut applied_this_frame = 0usize;
    for (entity, mut chunk, mut task) in query.iter_mut() {
        if applied_this_frame >= MAX_RESULTS_PER_FRAME {
            break;
        }
        let Some(result) = try_take(&mut task.0) else {
            continue;
        };
        applied_this_frame += 1;
        chunk.blocks = Some(result.blocks);
        if !result.indices.is_empty() {
            let render_bundle = mesh_generator.mc_mesh(
                result.positions,
                result.normals,
                result.indices,
                &mut meshes,
                &mut materials,
            );
            commands.entity(entity).insert(render_bundle);
        }
        commands.entity(entity).remove::<ChunkTask>();
    }
}

/// Chunk visibility, recomputed every frame as a pure function of state:
/// a meshed chunk is hidden iff it is superseded — all of its children are
/// settled (no pending `ChunkTask`) and at least one child is meshed —
/// otherwise visible (an explicit `Visible` punches through hidden
/// ancestors). Running after the command sync point, this frame's mesh
/// inserts and task removals are already applied, so there are no
/// deferred-vs-direct write races, no trigger-timing dependence (every
/// meshed parent is checked every frame), and no split flicker: while
/// sibling tasks are still pending, the parent's coarser mesh renders
/// underneath the staggered children.
pub fn update_chunk_visibility(
    mut meshed: Query<(&mut Visibility, Option<&Children>), With<Mesh3d>>,
    meshed_query: Query<(), With<Mesh3d>>,
    task_query: Query<(), With<ChunkTask>>,
) {
    for (mut visibility, children) in &mut meshed {
        let superseded = children.is_some_and(|children| {
            let any_meshed = children.iter().any(|child| meshed_query.contains(child));
            let any_pending = children.iter().any(|child| task_query.contains(child));
            any_meshed && !any_pending
        });
        *visibility = if superseded {
            Visibility::Hidden
        } else {
            Visibility::Visible
        };
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
    /// Lazily-created shared material for all MC meshes (vertex colors
    /// later; per-chunk materials were one asset per chunk for no reason).
    mc_material: Option<Handle<StandardMaterial>>,
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
            mc_material: None,
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

    /// Renders a finished generation payload's mesh data (produced
    /// off-thread). Main thread only — asset creation. Vertices are
    /// chunk-local flat crossing positions; `compute_normals` for now
    /// (radial normals later).
    pub(crate) fn mc_mesh(
        &mut self,
        positions: Vec<[f32; 3]>,
        normals: Vec<[f32; 3]>,
        indices: Vec<u32>,
        meshes: &mut Assets<Mesh>,
        materials: &mut Assets<StandardMaterial>,
    ) -> impl Bundle {
        let mut mesh = Mesh::new(PrimitiveTopology::TriangleList, RenderAssetUsages::all());
        mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, positions);
        mesh.insert_attribute(Mesh::ATTRIBUTE_NORMAL, normals);
        mesh.insert_indices(Indices::U32(indices));

        // One shared material for every MC mesh. No Visibility component
        // here — the system inserts it explicitly (a deferred Visible in
        // the bundle would overwrite the same-frame parent-hide; see
        // generate_chunk_meshes).
        let material = self
            .mc_material
            .get_or_insert_with(|| {
                materials.add(StandardMaterial {
                    base_color: Color::WHITE,
                    ..default()
                })
            })
            .clone();
        (Mesh3d(meshes.add(mesh)), MeshMaterial3d(material), Wireframe)
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




