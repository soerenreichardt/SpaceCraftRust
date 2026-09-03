//! Hand-rolled marching cubes — Paul Bourke's public-domain tables as
//! const code (source: paulbourke.net/geometry/polygonise). The tables
//! are bit-exact with the published corner/edge convention; do not remap
//! them — a mismatch here surfaces as pinhole cracks along chunk seams.
//!
//! Conventions:
//! - corner `k` sits at `CORNER_OFFSETS[k]` in cell-local `(e1, e2, e3)`
//!   unit coordinates: corners 0-3 on the `e3 = 0` face (counter-
//!   clockwise in Gray order 00, 10, 11, 01), 4-7 on the `e3 = 1` face;
//! - a cell's case index sets bit `2^k` for every solid corner `k`;
//! - edge `e` connects `EDGE_CORNERS[e]`; it is crossed exactly when its
//!   two corners differ in solidity (`EDGE_TABLE` bit `e`);
//! - `TRI_TABLE` rows list crossing-edge indices in vertex triples,
//!   `-1`-terminated — the canonical resolution of the ambiguous cases,
//!   used identically everywhere. Never case-adjust per chunk.

use crate::terrain::block::BlockType;
use crate::terrain::chunk::{Blocks, Chunk};
use crate::terrain::generate::{face_owns_cell, BandClass, FaceGrid, TerrainGenerator};
use crate::terrain::terrain_octree::DimensionRange;
use crate::terrain::CHUNK_SIZE;
use bevy::math::DVec3;
use crate::terrain::mesh::blocky;

/// Sample-grid domain width: a chunk's 32³ blocks plus the +1 positive
/// ring (index 32 = the neighbor's block 0). The internal storage also
/// holds a −1 ring (the minus-side neighbor's block 31) for the blocky
/// mesher's border faces.
pub(crate) const VIEW_SIZE: usize = CHUNK_SIZE + 1;
/// Internal storage extent per axis: −1..=32.
const EXTENT: usize = VIEW_SIZE + 1;

/// Solidity of a chunk's 33³ sample grid — the chunk's own blocks plus the
/// +1 positive ring on x/y/z, every sample straight from the generator (a
/// pure function of lattice coordinates, never read from neighbor
/// entities, which don't survive merges). Same predicate as `fill_blocks`
/// (pinned by a test).
pub(crate) struct OccupancyView {
    /// Solidity over the extended domain −1..=32 per axis, indexed with a
    /// +1 offset.
    solid: Vec<bool>,
}

impl OccupancyView {
    pub(crate) fn build(generator: &TerrainGenerator, grid: &FaceGrid, chunk: &Chunk) -> Self {
        let face = chunk.face.clone();
        let x_range = chunk.range.x(face.clone());
        let y_range = chunk.range.y(face.clone());
        let z_range = chunk.range.z(face);
        let step = |r: &DimensionRange| (r.1 - r.0) as f64 / CHUNK_SIZE as f64;
        let (step_x, step_y, step_z) = (step(x_range), step(y_range), step(z_range));
        let (x0, y0, z0) = (x_range.0 as f64, y_range.0 as f64, z_range.0 as f64);

        let mut solid = vec![false; EXTENT * EXTENT * EXTENT];
        for k in -1..=VIEW_SIZE as isize - 1 {
            let gz = z0 + k as f64 * step_z;
            for j in -1..=VIEW_SIZE as isize - 1 {
                let gy = y0 + j as f64 * step_y;
                for i in -1..=VIEW_SIZE as isize - 1 {
                    let gx = x0 + i as f64 * step_x;
                    let (i, j, k) = ((i + 1) as usize, (j + 1) as usize, (k + 1) as usize);
                    solid[EXTENT * (EXTENT * k + j) + i] =
                        generator.is_solid_at(grid, gx, gy, gz);
                }
            }
        }
        OccupancyView { solid }
    }

    #[inline]
    pub(crate) fn solid(&self, i: usize, j: usize, k: usize) -> bool {
        self.solid_at(i as isize, j as isize, k as isize)
    }

    /// Solidity over the extended domain (−1 = the minus-side neighbor's
    /// outermost block; 32 = the plus-side neighbor's block 0).
    #[inline]
    pub(crate) fn solid_at(&self, i: isize, j: isize, k: isize) -> bool {
        debug_assert!(
            (-1..=VIEW_SIZE as isize - 1).contains(&i)
                && (-1..=VIEW_SIZE as isize - 1).contains(&j)
                && (-1..=VIEW_SIZE as isize - 1).contains(&k)
        );
        let (i, j, k) = ((i + 1) as usize, (j + 1) as usize, (k + 1) as usize);
        self.solid[EXTENT * (EXTENT * k + j) + i]
    }

    /// Test constructor over the extended domain (−1..=32 per axis).
    #[cfg(test)]
    pub(crate) fn from_fn(f: impl Fn(isize, isize, isize) -> bool) -> Self {
        let mut solid = vec![false; EXTENT * EXTENT * EXTENT];
        for k in -1..=VIEW_SIZE as isize - 1 {
            for j in -1..=VIEW_SIZE as isize - 1 {
                for i in -1..=VIEW_SIZE as isize - 1 {
                    let (u, v, w) = ((i + 1) as usize, (j + 1) as usize, (k + 1) as usize);
                    solid[EXTENT * (EXTENT * w + v) + u] = f(i, j, k);
                }
            }
        }
        OccupancyView { solid }
    }
}

/// One chunk's mesh output — task-local buffers, shared by both meshers.
/// The blocky mesher fills `normals` directly (per-face axis normals); the
/// MC path computes gradient normals in the payload post-pass instead.
#[derive(Default)]
pub(crate) struct McOutput {
    /// Chunk-local vertex positions — flat crossing positions, center
    /// subtracted. Never sphere-projected (that would flatten mountains).
    pub(crate) positions: Vec<[f32; 3]>,
    pub(crate) normals: Vec<[f32; 3]>,
    /// Per-vertex block colors (the solid corner each crossing edge
    /// emerges from — Grass on surface faces, Stone on cliffs and cave
    /// interiors).
    pub(crate) colors: Vec<[f32; 4]>,
    pub(crate) indices: Vec<u32>,
}

/// Which mesh a chunk renders.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum MeshTier {
    /// The flat projected quad patch — the far tier. Dormant: `tier_for`
    /// does not select it yet; distance/level thresholds will revive it.
    BasePlane,
    /// The mid tier — the noise surface as a smooth marching-cubes mesh.
    MarchingCubes,
    /// The near tier — the block world itself: one cube face per
    /// solid/air interface in `Blocks` (`blocky::blocky_mesh`). Selected
    /// for band chunks at leaf level, where the block step is exactly 1
    /// world unit (blocks ARE voxels) and the LOD keeps them near the
    /// camera.
    Blocky,
}

/// Tier selection: band chunks carry the terrain surface; off-band chunks
/// are deep interior or empty space (no mesh). Within the band, leaves
/// get the block tier, everything else marching cubes.
pub(crate) fn tier_for(chunk: &Chunk, generator: &TerrainGenerator) -> Option<MeshTier> {
    if !generator.chunk_intersects_surface(chunk) {
        return None;
    }
    if (chunk.length - CHUNK_SIZE as f32).abs() < 0.5 {
        Some(MeshTier::Blocky)
    } else {
        Some(MeshTier::MarchingCubes)
    }
}

/// A chunk's full generation payload — the output of one off-thread task.
/// Plain data, `Send`, produced by `generate_chunk_payload` and applied on
/// the main thread by the poll system.
pub(crate) struct GeneratedChunk {
    pub(crate) blocks: Blocks,
    /// Chunk-local MC vertex positions (flat crossing positions).
    pub(crate) positions: Vec<[f32; 3]>,
    /// Per-vertex block colors — Grass on surface faces, Stone below.
    pub(crate) colors: Vec<[f32; 4]>,
    /// Gradient normals (`−∇density` at the vertex) — the true surface
    /// orientation, so lighting shows the actual relief; computed
    /// off-thread so applying a result costs no `compute_normals`.
    pub(crate) normals: Vec<[f32; 3]>,
    pub(crate) indices: Vec<u32>,
}

/// Everything a chunk's generation needs, in one pass over one set of
/// samples: band classification, block fill, and the MC mesh all read the
/// same 33³ occupancy view (the fill's solidities ARE the view's samples —
/// the mesher and the data cannot disagree). Pure and self-contained —
/// safe to run on a task pool thread.
pub(crate) fn generate_chunk_payload(
    generator: &TerrainGenerator,
    grid: &FaceGrid,
    chunk: &Chunk,
    tier: MeshTier,
) -> GeneratedChunk {
    match generator.band_class(chunk) {
        BandClass::Outside => GeneratedChunk {
            blocks: Blocks::default(),
            positions: Vec::new(),
            normals: Vec::new(),
            colors: Vec::new(),
            indices: Vec::new(),
        },
        BandClass::Inside => GeneratedChunk {
            blocks: Blocks::stone(),
            positions: Vec::new(),
            normals: Vec::new(),
            colors: Vec::new(),
            indices: Vec::new(),
        },
        BandClass::Band => {
            let view = OccupancyView::build(generator, grid, chunk);
            let blocks = blocks_from_view(&view, grid);
            let mut out = McOutput::default();
            let (positions, normals, colors, indices) = match tier {
                // The near tier consumes the block world directly; its
                // mesher emits per-face axis normals itself.
                MeshTier::Blocky => {
                    blocky::blocky_mesh(&view, &blocks, grid, chunk, &mut out);
                    (out.positions, out.normals, out.colors, out.indices)
                }
                // Mid tier: marching cubes, with gradient normals computed
                // off-thread (no `compute_normals` on the main thread).
                MeshTier::MarchingCubes | MeshTier::BasePlane => {
                    marching_cubes(&view, &blocks, grid, chunk, &mut out);
                    let center = chunk.center.as_dvec3();
                    let normals = out
                        .positions
                        .iter()
                        .map(|p| {
                            let world =
                                DVec3::new(p[0] as f64, p[1] as f64, p[2] as f64) + center;
                            let n = generator.surface_normal(world);
                            [n.x as f32, n.y as f32, n.z as f32]
                        })
                        .collect();
                    (out.positions, normals, out.colors, out.indices)
                }
            };
            GeneratedChunk {
                blocks,
                positions,
                normals,
                colors,
                indices,
            }
        }
    }
}

/// A sample column's surface block: the outermost solid along the radial
/// direction (the extreme-k solid — the block the terrain surface emerges
/// from). One rule, shared by the block fill and ring-corner vertex
/// colors, so the two cannot drift.
fn column_surface_k(
    view: &OccupancyView,
    i: usize,
    j: usize,
    outward: bool,
) -> Option<usize> {
    if outward {
        (0..VIEW_SIZE).rev().find(|&k| view.solid(i, j, k))
    } else {
        (0..VIEW_SIZE).find(|&k| view.solid(i, j, k))
    }
}

/// Fills a chunk's `Blocks` from an existing occupancy view: per (i, j)
/// column, the outermost solid block gets `Grass`, solids below it
/// `Stone`, air stays `Air` (cave pockets get `Stone` ceilings). The
/// view's samples (chunk blocks + the +1 ring) are the solidities — no
/// second sampling pass.
pub(crate) fn blocks_from_view(view: &OccupancyView, grid: &FaceGrid) -> Blocks {
    let outward = grid.gz_points_outward();
    let mut blocks = [BlockType::Air; CHUNK_SIZE * CHUNK_SIZE * CHUNK_SIZE];
    for i in 0..CHUNK_SIZE {
        for j in 0..CHUNK_SIZE {
            let surface_k = column_surface_k(view, i, j, outward);
            for k in 0..CHUNK_SIZE {
                if view.solid(i, j, k) {
                    blocks[CHUNK_SIZE * (CHUNK_SIZE * k + j) + i] =
                        if Some(k) == surface_k {
                            BlockType::Grass
                        } else {
                            BlockType::Stone
                        };
                }
            }
        }
    }
    Blocks::from_blocks(blocks)
}

/// Meshes a chunk's 32³ cells (min-corner ownership: every cell whose
/// minimum corner is one of the chunk's own blocks). Cells whose min corner
/// lies in another face's angular pyramid are skipped — that face's chunk
/// meshes them (mesh-exactly-once; cross-face agreement comes from the
/// shared world lattice and the point-canonical density). Boundary cells
/// read the +1 ring from the view. Crossings sit at the **midpoint** of the
/// two corners (binary occupancy — height quantizes to half a block step).
pub(crate) fn marching_cubes(
    view: &OccupancyView,
    blocks: &Blocks,
    grid: &FaceGrid,
    chunk: &Chunk,
    out: &mut McOutput,
) {
    let face = chunk.face.clone();
    let x_range = chunk.range.x(face.clone());
    let y_range = chunk.range.y(face.clone());
    let z_range = chunk.range.z(face.clone());
    let step = |r: &DimensionRange| (r.1 - r.0) as f64 / CHUNK_SIZE as f64;
    let (step_x, step_y, step_z) = (step(x_range), step(y_range), step(z_range));
    let (x0, y0, z0) = (x_range.0 as f64, y_range.0 as f64, z_range.0 as f64);
    let center = chunk.center.as_dvec3();

    // Bourke's table winds triangle normals INTO the solid. The four
    // left-handed face frames are reflections, which already flips their
    // orientation — so reversing the RIGHT-handed frames gives every face
    // outward normals. Same rule as the base plane's flip.
    let flip = !grid.is_left_handed();

    for k in 0..CHUNK_SIZE {
        let gz = z0 + k as f64 * step_z;
        for j in 0..CHUNK_SIZE {
            let gy = y0 + j as f64 * step_y;
            for i in 0..CHUNK_SIZE {
                let gx = x0 + i as f64 * step_x;

                // Ownership gate: another face's pyramid → their cell.
                if !face_owns_cell(grid, &face, gx, gy, gz) {
                    continue;
                }

                let mut case = 0u8;
                for (c, &[ox, oy, oz]) in CORNER_OFFSETS.iter().enumerate() {
                    if view.solid(i + ox as usize, j + oy as usize, k + oz as usize) {
                        case |= 1 << c;
                    }
                }
                if case == 0 || case == 255 {
                    continue; // no-mix fast path
                }

                let row = &TRI_TABLE[case as usize];
                let edge_count = row.iter().position(|&v| v == -1).unwrap();
                // Per-cell vertex slots (one vertex per crossed edge).
                let mut slots = [u32::MAX; 12];
                for t in 0..edge_count / 3 {
                    let tri = [
                        row[t * 3] as usize,
                        row[t * 3 + 1] as usize,
                        row[t * 3 + 2] as usize,
                    ];
                    let order = if flip {
                        [tri[0], tri[2], tri[1]]
                    } else {
                        [tri[0], tri[1], tri[2]]
                    };
                    for &e in &order {
                        if slots[e] == u32::MAX {
                            let [ca, cb] = EDGE_CORNERS[e];
                            let [ax, ay, az] = CORNER_OFFSETS[ca as usize];
                            let [bx, by, bz] = CORNER_OFFSETS[cb as usize];
                            let m_gx = gx + (ax + bx) as f64 * step_x * 0.5;
                            let m_gy = gy + (ay + by) as f64 * step_y * 0.5;
                            let m_gz = gz + (az + bz) as f64 * step_z * 0.5;
                            let p = grid.position(m_gx, m_gy, m_gz) - center;
                            slots[e] = out.positions.len() as u32;
                            out.positions.push([p.x as f32, p.y as f32, p.z as f32]);
                            // Vertex color: the block the crossing edge
                            // emerges from — the edge's SOLID corner.
                            let (sx, sy, sz) = if view.solid(
                                i + ax as usize,
                                j + ay as usize,
                                k + az as usize,
                            ) {
                                (ax, ay, az)
                            } else {
                                (bx, by, bz)
                            };
                            let (ci, cj, ck) = (
                                i + sx as usize,
                                j + sy as usize,
                                k + sz as usize,
                            );
                            let block = if ci < CHUNK_SIZE
                                && cj < CHUNK_SIZE
                                && ck < CHUNK_SIZE
                            {
                                blocks.get(ci, cj, ck)
                            } else {
                                // +1 ring corner: the neighbor's block,
                                // typed by the same column rule the
                                // neighbor's fill applies. For +x/+y rings
                                // the view spans exactly the same-level
                                // neighbor's column window, so border
                                // vertex colors match across the seam (the
                                // old Stone hardcode painted every border
                                // crossing gray). The only truncated case —
                                // the +z ring on outward-gz faces — occurs
                                // on undersides (cave ceilings), where
                                // Stone is correct anyway.
                                let surface =
                                    column_surface_k(view, ci, cj, grid.gz_points_outward());
                                if surface == Some(ck) {
                                    BlockType::Grass
                                } else {
                                    BlockType::Stone
                                }
                            };
                            out.colors.push(block.color());
                        }
                        out.indices.push(slots[e]);
                    }
                }
            }
        }
    }
}

/// Cell-local corner positions in `(e1, e2, e3)` unit coordinates.
pub(crate) const CORNER_OFFSETS: [[u8; 3]; 8] = [
    [0, 0, 0], // corner 0
    [1, 0, 0], // corner 1
    [1, 1, 0], // corner 2
    [0, 1, 0], // corner 3
    [0, 0, 1], // corner 4
    [1, 0, 1], // corner 5
    [1, 1, 1], // corner 6
    [0, 1, 1], // corner 7
];

/// The two corners each of the 12 edges connects.
pub(crate) const EDGE_CORNERS: [[u8; 2]; 12] = [
    [0, 1], // edge 0
    [1, 2], // edge 1
    [2, 3], // edge 2
    [3, 0], // edge 3
    [4, 5], // edge 4
    [5, 6], // edge 5
    [6, 7], // edge 6
    [7, 4], // edge 7
    [0, 4], // edge 8
    [1, 5], // edge 9
    [2, 6], // edge 10
    [3, 7], // edge 11
];

/// Per case: bitmask of crossed edges (bit `e` = edge `e`).
pub(crate) const EDGE_TABLE: [u16; 256] = [
    0x000, 0x109, 0x203, 0x30a, 0x406, 0x50f, 0x605, 0x70c,
    0x80c, 0x905, 0xa0f, 0xb06, 0xc0a, 0xd03, 0xe09, 0xf00,
    0x190, 0x099, 0x393, 0x29a, 0x596, 0x49f, 0x795, 0x69c,
    0x99c, 0x895, 0xb9f, 0xa96, 0xd9a, 0xc93, 0xf99, 0xe90,
    0x230, 0x339, 0x033, 0x13a, 0x636, 0x73f, 0x435, 0x53c,
    0xa3c, 0xb35, 0x83f, 0x936, 0xe3a, 0xf33, 0xc39, 0xd30,
    0x3a0, 0x2a9, 0x1a3, 0x0aa, 0x7a6, 0x6af, 0x5a5, 0x4ac,
    0xbac, 0xaa5, 0x9af, 0x8a6, 0xfaa, 0xea3, 0xda9, 0xca0,
    0x460, 0x569, 0x663, 0x76a, 0x066, 0x16f, 0x265, 0x36c,
    0xc6c, 0xd65, 0xe6f, 0xf66, 0x86a, 0x963, 0xa69, 0xb60,
    0x5f0, 0x4f9, 0x7f3, 0x6fa, 0x1f6, 0x0ff, 0x3f5, 0x2fc,
    0xdfc, 0xcf5, 0xfff, 0xef6, 0x9fa, 0x8f3, 0xbf9, 0xaf0,
    0x650, 0x759, 0x453, 0x55a, 0x256, 0x35f, 0x055, 0x15c,
    0xe5c, 0xf55, 0xc5f, 0xd56, 0xa5a, 0xb53, 0x859, 0x950,
    0x7c0, 0x6c9, 0x5c3, 0x4ca, 0x3c6, 0x2cf, 0x1c5, 0x0cc,
    0xfcc, 0xec5, 0xdcf, 0xcc6, 0xbca, 0xac3, 0x9c9, 0x8c0,
    0x8c0, 0x9c9, 0xac3, 0xbca, 0xcc6, 0xdcf, 0xec5, 0xfcc,
    0x0cc, 0x1c5, 0x2cf, 0x3c6, 0x4ca, 0x5c3, 0x6c9, 0x7c0,
    0x950, 0x859, 0xb53, 0xa5a, 0xd56, 0xc5f, 0xf55, 0xe5c,
    0x15c, 0x055, 0x35f, 0x256, 0x55a, 0x453, 0x759, 0x650,
    0xaf0, 0xbf9, 0x8f3, 0x9fa, 0xef6, 0xfff, 0xcf5, 0xdfc,
    0x2fc, 0x3f5, 0x0ff, 0x1f6, 0x6fa, 0x7f3, 0x4f9, 0x5f0,
    0xb60, 0xa69, 0x963, 0x86a, 0xf66, 0xe6f, 0xd65, 0xc6c,
    0x36c, 0x265, 0x16f, 0x066, 0x76a, 0x663, 0x569, 0x460,
    0xca0, 0xda9, 0xea3, 0xfaa, 0x8a6, 0x9af, 0xaa5, 0xbac,
    0x4ac, 0x5a5, 0x6af, 0x7a6, 0x0aa, 0x1a3, 0x2a9, 0x3a0,
    0xd30, 0xc39, 0xf33, 0xe3a, 0x936, 0x83f, 0xb35, 0xa3c,
    0x53c, 0x435, 0x73f, 0x636, 0x13a, 0x033, 0x339, 0x230,
    0xe90, 0xf99, 0xc93, 0xd9a, 0xa96, 0xb9f, 0x895, 0x99c,
    0x69c, 0x795, 0x49f, 0x596, 0x29a, 0x393, 0x099, 0x190,
    0xf00, 0xe09, 0xd03, 0xc0a, 0xb06, 0xa0f, 0x905, 0x80c,
    0x70c, 0x605, 0x50f, 0x406, 0x30a, 0x203, 0x109, 0x000,
];

/// Per case: crossing-edge indices in vertex triples, `-1`-terminated.
pub(crate) const TRI_TABLE: [[i8; 16]; 256] = [
    [-1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1],
    [0, 8, 3, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1],
    [0, 1, 9, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1],
    [1, 8, 3, 9, 8, 1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1],
    [1, 2, 10, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1],
    [0, 8, 3, 1, 2, 10, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1],
    [9, 2, 10, 0, 2, 9, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1],
    [2, 8, 3, 2, 10, 8, 10, 9, 8, -1, -1, -1, -1, -1, -1, -1],
    [3, 11, 2, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1],
    [0, 11, 2, 8, 11, 0, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1],
    [1, 9, 0, 2, 3, 11, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1],
    [1, 11, 2, 1, 9, 11, 9, 8, 11, -1, -1, -1, -1, -1, -1, -1],
    [3, 10, 1, 11, 10, 3, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1],
    [0, 10, 1, 0, 8, 10, 8, 11, 10, -1, -1, -1, -1, -1, -1, -1],
    [3, 9, 0, 3, 11, 9, 11, 10, 9, -1, -1, -1, -1, -1, -1, -1],
    [9, 8, 10, 10, 8, 11, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1],
    [4, 7, 8, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1],
    [4, 3, 0, 7, 3, 4, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1],
    [0, 1, 9, 8, 4, 7, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1],
    [4, 1, 9, 4, 7, 1, 7, 3, 1, -1, -1, -1, -1, -1, -1, -1],
    [1, 2, 10, 8, 4, 7, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1],
    [3, 4, 7, 3, 0, 4, 1, 2, 10, -1, -1, -1, -1, -1, -1, -1],
    [9, 2, 10, 9, 0, 2, 8, 4, 7, -1, -1, -1, -1, -1, -1, -1],
    [2, 10, 9, 2, 9, 7, 2, 7, 3, 7, 9, 4, -1, -1, -1, -1],
    [8, 4, 7, 3, 11, 2, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1],
    [11, 4, 7, 11, 2, 4, 2, 0, 4, -1, -1, -1, -1, -1, -1, -1],
    [9, 0, 1, 8, 4, 7, 2, 3, 11, -1, -1, -1, -1, -1, -1, -1],
    [4, 7, 11, 9, 4, 11, 9, 11, 2, 9, 2, 1, -1, -1, -1, -1],
    [3, 10, 1, 3, 11, 10, 7, 8, 4, -1, -1, -1, -1, -1, -1, -1],
    [1, 11, 10, 1, 4, 11, 1, 0, 4, 7, 11, 4, -1, -1, -1, -1],
    [4, 7, 8, 9, 0, 11, 9, 11, 10, 11, 0, 3, -1, -1, -1, -1],
    [4, 7, 11, 4, 11, 9, 9, 11, 10, -1, -1, -1, -1, -1, -1, -1],
    [9, 5, 4, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1],
    [9, 5, 4, 0, 8, 3, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1],
    [0, 5, 4, 1, 5, 0, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1],
    [8, 5, 4, 8, 3, 5, 3, 1, 5, -1, -1, -1, -1, -1, -1, -1],
    [1, 2, 10, 9, 5, 4, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1],
    [3, 0, 8, 1, 2, 10, 4, 9, 5, -1, -1, -1, -1, -1, -1, -1],
    [5, 2, 10, 5, 4, 2, 4, 0, 2, -1, -1, -1, -1, -1, -1, -1],
    [2, 10, 5, 3, 2, 5, 3, 5, 4, 3, 4, 8, -1, -1, -1, -1],
    [9, 5, 4, 2, 3, 11, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1],
    [0, 11, 2, 0, 8, 11, 4, 9, 5, -1, -1, -1, -1, -1, -1, -1],
    [0, 5, 4, 0, 1, 5, 2, 3, 11, -1, -1, -1, -1, -1, -1, -1],
    [2, 1, 5, 2, 5, 8, 2, 8, 11, 4, 8, 5, -1, -1, -1, -1],
    [10, 3, 11, 10, 1, 3, 9, 5, 4, -1, -1, -1, -1, -1, -1, -1],
    [4, 9, 5, 0, 8, 1, 8, 10, 1, 8, 11, 10, -1, -1, -1, -1],
    [5, 4, 0, 5, 0, 11, 5, 11, 10, 11, 0, 3, -1, -1, -1, -1],
    [5, 4, 8, 5, 8, 10, 10, 8, 11, -1, -1, -1, -1, -1, -1, -1],
    [9, 7, 8, 5, 7, 9, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1],
    [9, 3, 0, 9, 5, 3, 5, 7, 3, -1, -1, -1, -1, -1, -1, -1],
    [0, 7, 8, 0, 1, 7, 1, 5, 7, -1, -1, -1, -1, -1, -1, -1],
    [1, 5, 3, 3, 5, 7, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1],
    [9, 7, 8, 9, 5, 7, 10, 1, 2, -1, -1, -1, -1, -1, -1, -1],
    [10, 1, 2, 9, 5, 0, 5, 3, 0, 5, 7, 3, -1, -1, -1, -1],
    [8, 0, 2, 8, 2, 5, 8, 5, 7, 10, 5, 2, -1, -1, -1, -1],
    [2, 10, 5, 2, 5, 3, 3, 5, 7, -1, -1, -1, -1, -1, -1, -1],
    [7, 9, 5, 7, 8, 9, 3, 11, 2, -1, -1, -1, -1, -1, -1, -1],
    [9, 5, 7, 9, 7, 2, 9, 2, 0, 2, 7, 11, -1, -1, -1, -1],
    [2, 3, 11, 0, 1, 8, 1, 7, 8, 1, 5, 7, -1, -1, -1, -1],
    [11, 2, 1, 11, 1, 7, 7, 1, 5, -1, -1, -1, -1, -1, -1, -1],
    [9, 5, 8, 8, 5, 7, 10, 1, 3, 10, 3, 11, -1, -1, -1, -1],
    [5, 7, 0, 5, 0, 9, 7, 11, 0, 1, 0, 10, 11, 10, 0, -1],
    [11, 10, 0, 11, 0, 3, 10, 5, 0, 8, 0, 7, 5, 7, 0, -1],
    [11, 10, 5, 7, 11, 5, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1],
    [10, 6, 5, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1],
    [0, 8, 3, 5, 10, 6, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1],
    [9, 0, 1, 5, 10, 6, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1],
    [1, 8, 3, 1, 9, 8, 5, 10, 6, -1, -1, -1, -1, -1, -1, -1],
    [1, 6, 5, 2, 6, 1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1],
    [1, 6, 5, 1, 2, 6, 3, 0, 8, -1, -1, -1, -1, -1, -1, -1],
    [9, 6, 5, 9, 0, 6, 0, 2, 6, -1, -1, -1, -1, -1, -1, -1],
    [5, 9, 8, 5, 8, 2, 5, 2, 6, 3, 2, 8, -1, -1, -1, -1],
    [2, 3, 11, 10, 6, 5, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1],
    [11, 0, 8, 11, 2, 0, 10, 6, 5, -1, -1, -1, -1, -1, -1, -1],
    [0, 1, 9, 2, 3, 11, 5, 10, 6, -1, -1, -1, -1, -1, -1, -1],
    [5, 10, 6, 1, 9, 2, 9, 11, 2, 9, 8, 11, -1, -1, -1, -1],
    [6, 3, 11, 6, 5, 3, 5, 1, 3, -1, -1, -1, -1, -1, -1, -1],
    [0, 8, 11, 0, 11, 5, 0, 5, 1, 5, 11, 6, -1, -1, -1, -1],
    [3, 11, 6, 0, 3, 6, 0, 6, 5, 0, 5, 9, -1, -1, -1, -1],
    [6, 5, 9, 6, 9, 11, 11, 9, 8, -1, -1, -1, -1, -1, -1, -1],
    [5, 10, 6, 4, 7, 8, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1],
    [4, 3, 0, 4, 7, 3, 6, 5, 10, -1, -1, -1, -1, -1, -1, -1],
    [1, 9, 0, 5, 10, 6, 8, 4, 7, -1, -1, -1, -1, -1, -1, -1],
    [10, 6, 5, 1, 9, 7, 1, 7, 3, 7, 9, 4, -1, -1, -1, -1],
    [6, 1, 2, 6, 5, 1, 4, 7, 8, -1, -1, -1, -1, -1, -1, -1],
    [1, 2, 5, 5, 2, 6, 3, 0, 4, 3, 4, 7, -1, -1, -1, -1],
    [8, 4, 7, 9, 0, 5, 0, 6, 5, 0, 2, 6, -1, -1, -1, -1],
    [7, 3, 9, 7, 9, 4, 3, 2, 9, 5, 9, 6, 2, 6, 9, -1],
    [3, 11, 2, 7, 8, 4, 10, 6, 5, -1, -1, -1, -1, -1, -1, -1],
    [5, 10, 6, 4, 7, 2, 4, 2, 0, 2, 7, 11, -1, -1, -1, -1],
    [0, 1, 9, 4, 7, 8, 2, 3, 11, 5, 10, 6, -1, -1, -1, -1],
    [9, 2, 1, 9, 11, 2, 9, 4, 11, 7, 11, 4, 5, 10, 6, -1],
    [8, 4, 7, 3, 11, 5, 3, 5, 1, 5, 11, 6, -1, -1, -1, -1],
    [5, 1, 11, 5, 11, 6, 1, 0, 11, 7, 11, 4, 0, 4, 11, -1],
    [0, 5, 9, 0, 6, 5, 0, 3, 6, 11, 6, 3, 8, 4, 7, -1],
    [6, 5, 9, 6, 9, 11, 4, 7, 9, 7, 11, 9, -1, -1, -1, -1],
    [10, 4, 9, 6, 4, 10, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1],
    [4, 10, 6, 4, 9, 10, 0, 8, 3, -1, -1, -1, -1, -1, -1, -1],
    [10, 0, 1, 10, 6, 0, 6, 4, 0, -1, -1, -1, -1, -1, -1, -1],
    [8, 3, 1, 8, 1, 6, 8, 6, 4, 6, 1, 10, -1, -1, -1, -1],
    [1, 4, 9, 1, 2, 4, 2, 6, 4, -1, -1, -1, -1, -1, -1, -1],
    [3, 0, 8, 1, 2, 9, 2, 4, 9, 2, 6, 4, -1, -1, -1, -1],
    [0, 2, 4, 4, 2, 6, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1],
    [8, 3, 2, 8, 2, 4, 4, 2, 6, -1, -1, -1, -1, -1, -1, -1],
    [10, 4, 9, 10, 6, 4, 11, 2, 3, -1, -1, -1, -1, -1, -1, -1],
    [0, 8, 2, 2, 8, 11, 4, 9, 10, 4, 10, 6, -1, -1, -1, -1],
    [3, 11, 2, 0, 1, 6, 0, 6, 4, 6, 1, 10, -1, -1, -1, -1],
    [6, 4, 1, 6, 1, 10, 4, 8, 1, 2, 1, 11, 8, 11, 1, -1],
    [9, 6, 4, 9, 3, 6, 9, 1, 3, 11, 6, 3, -1, -1, -1, -1],
    [8, 11, 1, 8, 1, 0, 11, 6, 1, 9, 1, 4, 6, 4, 1, -1],
    [3, 11, 6, 3, 6, 0, 0, 6, 4, -1, -1, -1, -1, -1, -1, -1],
    [6, 4, 8, 11, 6, 8, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1],
    [7, 10, 6, 7, 8, 10, 8, 9, 10, -1, -1, -1, -1, -1, -1, -1],
    [0, 7, 3, 0, 10, 7, 0, 9, 10, 6, 7, 10, -1, -1, -1, -1],
    [10, 6, 7, 1, 10, 7, 1, 7, 8, 1, 8, 0, -1, -1, -1, -1],
    [10, 6, 7, 10, 7, 1, 1, 7, 3, -1, -1, -1, -1, -1, -1, -1],
    [1, 2, 6, 1, 6, 8, 1, 8, 9, 8, 6, 7, -1, -1, -1, -1],
    [2, 6, 9, 2, 9, 1, 6, 7, 9, 0, 9, 3, 7, 3, 9, -1],
    [7, 8, 0, 7, 0, 6, 6, 0, 2, -1, -1, -1, -1, -1, -1, -1],
    [7, 3, 2, 6, 7, 2, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1],
    [2, 3, 11, 10, 6, 8, 10, 8, 9, 8, 6, 7, -1, -1, -1, -1],
    [2, 0, 7, 2, 7, 11, 0, 9, 7, 6, 7, 10, 9, 10, 7, -1],
    [1, 8, 0, 1, 7, 8, 1, 10, 7, 6, 7, 10, 2, 3, 11, -1],
    [11, 2, 1, 11, 1, 7, 10, 6, 1, 6, 7, 1, -1, -1, -1, -1],
    [8, 9, 6, 8, 6, 7, 9, 1, 6, 11, 6, 3, 1, 3, 6, -1],
    [0, 9, 1, 11, 6, 7, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1],
    [7, 8, 0, 7, 0, 6, 3, 11, 0, 11, 6, 0, -1, -1, -1, -1],
    [7, 11, 6, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1],
    [7, 6, 11, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1],
    [3, 0, 8, 11, 7, 6, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1],
    [0, 1, 9, 11, 7, 6, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1],
    [8, 1, 9, 8, 3, 1, 11, 7, 6, -1, -1, -1, -1, -1, -1, -1],
    [10, 1, 2, 6, 11, 7, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1],
    [1, 2, 10, 3, 0, 8, 6, 11, 7, -1, -1, -1, -1, -1, -1, -1],
    [2, 9, 0, 2, 10, 9, 6, 11, 7, -1, -1, -1, -1, -1, -1, -1],
    [6, 11, 7, 2, 10, 3, 10, 8, 3, 10, 9, 8, -1, -1, -1, -1],
    [7, 2, 3, 6, 2, 7, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1],
    [7, 0, 8, 7, 6, 0, 6, 2, 0, -1, -1, -1, -1, -1, -1, -1],
    [2, 7, 6, 2, 3, 7, 0, 1, 9, -1, -1, -1, -1, -1, -1, -1],
    [1, 6, 2, 1, 8, 6, 1, 9, 8, 8, 7, 6, -1, -1, -1, -1],
    [10, 7, 6, 10, 1, 7, 1, 3, 7, -1, -1, -1, -1, -1, -1, -1],
    [10, 7, 6, 1, 7, 10, 1, 8, 7, 1, 0, 8, -1, -1, -1, -1],
    [0, 3, 7, 0, 7, 10, 0, 10, 9, 6, 10, 7, -1, -1, -1, -1],
    [7, 6, 10, 7, 10, 8, 8, 10, 9, -1, -1, -1, -1, -1, -1, -1],
    [6, 8, 4, 11, 8, 6, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1],
    [3, 6, 11, 3, 0, 6, 0, 4, 6, -1, -1, -1, -1, -1, -1, -1],
    [8, 6, 11, 8, 4, 6, 9, 0, 1, -1, -1, -1, -1, -1, -1, -1],
    [9, 4, 6, 9, 6, 3, 9, 3, 1, 11, 3, 6, -1, -1, -1, -1],
    [6, 8, 4, 6, 11, 8, 2, 10, 1, -1, -1, -1, -1, -1, -1, -1],
    [1, 2, 10, 3, 0, 11, 0, 6, 11, 0, 4, 6, -1, -1, -1, -1],
    [4, 11, 8, 4, 6, 11, 0, 2, 9, 2, 10, 9, -1, -1, -1, -1],
    [10, 9, 3, 10, 3, 2, 9, 4, 3, 11, 3, 6, 4, 6, 3, -1],
    [8, 2, 3, 8, 4, 2, 4, 6, 2, -1, -1, -1, -1, -1, -1, -1],
    [0, 4, 2, 4, 6, 2, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1],
    [1, 9, 0, 2, 3, 4, 2, 4, 6, 4, 3, 8, -1, -1, -1, -1],
    [1, 9, 4, 1, 4, 2, 2, 4, 6, -1, -1, -1, -1, -1, -1, -1],
    [8, 1, 3, 8, 6, 1, 8, 4, 6, 6, 10, 1, -1, -1, -1, -1],
    [10, 1, 0, 10, 0, 6, 6, 0, 4, -1, -1, -1, -1, -1, -1, -1],
    [4, 6, 3, 4, 3, 8, 6, 10, 3, 0, 3, 9, 10, 9, 3, -1],
    [10, 9, 4, 6, 10, 4, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1],
    [4, 9, 5, 7, 6, 11, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1],
    [0, 8, 3, 4, 9, 5, 11, 7, 6, -1, -1, -1, -1, -1, -1, -1],
    [5, 0, 1, 5, 4, 0, 7, 6, 11, -1, -1, -1, -1, -1, -1, -1],
    [11, 7, 6, 8, 3, 4, 3, 5, 4, 3, 1, 5, -1, -1, -1, -1],
    [9, 5, 4, 10, 1, 2, 7, 6, 11, -1, -1, -1, -1, -1, -1, -1],
    [6, 11, 7, 1, 2, 10, 0, 8, 3, 4, 9, 5, -1, -1, -1, -1],
    [7, 6, 11, 5, 4, 10, 4, 2, 10, 4, 0, 2, -1, -1, -1, -1],
    [3, 4, 8, 3, 5, 4, 3, 2, 5, 10, 5, 2, 11, 7, 6, -1],
    [7, 2, 3, 7, 6, 2, 5, 4, 9, -1, -1, -1, -1, -1, -1, -1],
    [9, 5, 4, 0, 8, 6, 0, 6, 2, 6, 8, 7, -1, -1, -1, -1],
    [3, 6, 2, 3, 7, 6, 1, 5, 0, 5, 4, 0, -1, -1, -1, -1],
    [6, 2, 8, 6, 8, 7, 2, 1, 8, 4, 8, 5, 1, 5, 8, -1],
    [9, 5, 4, 10, 1, 6, 1, 7, 6, 1, 3, 7, -1, -1, -1, -1],
    [1, 6, 10, 1, 7, 6, 1, 0, 7, 8, 7, 0, 9, 5, 4, -1],
    [4, 0, 10, 4, 10, 5, 0, 3, 10, 6, 10, 7, 3, 7, 10, -1],
    [7, 6, 10, 7, 10, 8, 5, 4, 10, 4, 8, 10, -1, -1, -1, -1],
    [6, 9, 5, 6, 11, 9, 11, 8, 9, -1, -1, -1, -1, -1, -1, -1],
    [3, 6, 11, 0, 6, 3, 0, 5, 6, 0, 9, 5, -1, -1, -1, -1],
    [0, 11, 8, 0, 5, 11, 0, 1, 5, 5, 6, 11, -1, -1, -1, -1],
    [6, 11, 3, 6, 3, 5, 5, 3, 1, -1, -1, -1, -1, -1, -1, -1],
    [1, 2, 10, 9, 5, 11, 9, 11, 8, 11, 5, 6, -1, -1, -1, -1],
    [0, 11, 3, 0, 6, 11, 0, 9, 6, 5, 6, 9, 1, 2, 10, -1],
    [11, 8, 5, 11, 5, 6, 8, 0, 5, 10, 5, 2, 0, 2, 5, -1],
    [6, 11, 3, 6, 3, 5, 2, 10, 3, 10, 5, 3, -1, -1, -1, -1],
    [5, 8, 9, 5, 2, 8, 5, 6, 2, 3, 8, 2, -1, -1, -1, -1],
    [9, 5, 6, 9, 6, 0, 0, 6, 2, -1, -1, -1, -1, -1, -1, -1],
    [1, 5, 8, 1, 8, 0, 5, 6, 8, 3, 8, 2, 6, 2, 8, -1],
    [1, 5, 6, 2, 1, 6, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1],
    [1, 3, 6, 1, 6, 10, 3, 8, 6, 5, 6, 9, 8, 9, 6, -1],
    [10, 1, 0, 10, 0, 6, 9, 5, 0, 5, 6, 0, -1, -1, -1, -1],
    [0, 3, 8, 5, 6, 10, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1],
    [10, 5, 6, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1],
    [11, 5, 10, 7, 5, 11, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1],
    [11, 5, 10, 11, 7, 5, 8, 3, 0, -1, -1, -1, -1, -1, -1, -1],
    [5, 11, 7, 5, 10, 11, 1, 9, 0, -1, -1, -1, -1, -1, -1, -1],
    [10, 7, 5, 10, 11, 7, 9, 8, 1, 8, 3, 1, -1, -1, -1, -1],
    [11, 1, 2, 11, 7, 1, 7, 5, 1, -1, -1, -1, -1, -1, -1, -1],
    [0, 8, 3, 1, 2, 7, 1, 7, 5, 7, 2, 11, -1, -1, -1, -1],
    [9, 7, 5, 9, 2, 7, 9, 0, 2, 2, 11, 7, -1, -1, -1, -1],
    [7, 5, 2, 7, 2, 11, 5, 9, 2, 3, 2, 8, 9, 8, 2, -1],
    [2, 5, 10, 2, 3, 5, 3, 7, 5, -1, -1, -1, -1, -1, -1, -1],
    [8, 2, 0, 8, 5, 2, 8, 7, 5, 10, 2, 5, -1, -1, -1, -1],
    [9, 0, 1, 5, 10, 3, 5, 3, 7, 3, 10, 2, -1, -1, -1, -1],
    [9, 8, 2, 9, 2, 1, 8, 7, 2, 10, 2, 5, 7, 5, 2, -1],
    [1, 3, 5, 3, 7, 5, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1],
    [0, 8, 7, 0, 7, 1, 1, 7, 5, -1, -1, -1, -1, -1, -1, -1],
    [9, 0, 3, 9, 3, 5, 5, 3, 7, -1, -1, -1, -1, -1, -1, -1],
    [9, 8, 7, 5, 9, 7, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1],
    [5, 8, 4, 5, 10, 8, 10, 11, 8, -1, -1, -1, -1, -1, -1, -1],
    [5, 0, 4, 5, 11, 0, 5, 10, 11, 11, 3, 0, -1, -1, -1, -1],
    [0, 1, 9, 8, 4, 10, 8, 10, 11, 10, 4, 5, -1, -1, -1, -1],
    [10, 11, 4, 10, 4, 5, 11, 3, 4, 9, 4, 1, 3, 1, 4, -1],
    [2, 5, 1, 2, 8, 5, 2, 11, 8, 4, 5, 8, -1, -1, -1, -1],
    [0, 4, 11, 0, 11, 3, 4, 5, 11, 2, 11, 1, 5, 1, 11, -1],
    [0, 2, 5, 0, 5, 9, 2, 11, 5, 4, 5, 8, 11, 8, 5, -1],
    [9, 4, 5, 2, 11, 3, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1],
    [2, 5, 10, 3, 5, 2, 3, 4, 5, 3, 8, 4, -1, -1, -1, -1],
    [5, 10, 2, 5, 2, 4, 4, 2, 0, -1, -1, -1, -1, -1, -1, -1],
    [3, 10, 2, 3, 5, 10, 3, 8, 5, 4, 5, 8, 0, 1, 9, -1],
    [5, 10, 2, 5, 2, 4, 1, 9, 2, 9, 4, 2, -1, -1, -1, -1],
    [8, 4, 5, 8, 5, 3, 3, 5, 1, -1, -1, -1, -1, -1, -1, -1],
    [0, 4, 5, 1, 0, 5, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1],
    [8, 4, 5, 8, 5, 3, 9, 0, 5, 0, 3, 5, -1, -1, -1, -1],
    [9, 4, 5, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1],
    [4, 11, 7, 4, 9, 11, 9, 10, 11, -1, -1, -1, -1, -1, -1, -1],
    [0, 8, 3, 4, 9, 7, 9, 11, 7, 9, 10, 11, -1, -1, -1, -1],
    [1, 10, 11, 1, 11, 4, 1, 4, 0, 7, 4, 11, -1, -1, -1, -1],
    [3, 1, 4, 3, 4, 8, 1, 10, 4, 7, 4, 11, 10, 11, 4, -1],
    [4, 11, 7, 9, 11, 4, 9, 2, 11, 9, 1, 2, -1, -1, -1, -1],
    [9, 7, 4, 9, 11, 7, 9, 1, 11, 2, 11, 1, 0, 8, 3, -1],
    [11, 7, 4, 11, 4, 2, 2, 4, 0, -1, -1, -1, -1, -1, -1, -1],
    [11, 7, 4, 11, 4, 2, 8, 3, 4, 3, 2, 4, -1, -1, -1, -1],
    [2, 9, 10, 2, 7, 9, 2, 3, 7, 7, 4, 9, -1, -1, -1, -1],
    [9, 10, 7, 9, 7, 4, 10, 2, 7, 8, 7, 0, 2, 0, 7, -1],
    [3, 7, 10, 3, 10, 2, 7, 4, 10, 1, 10, 0, 4, 0, 10, -1],
    [1, 10, 2, 8, 7, 4, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1],
    [4, 9, 1, 4, 1, 7, 7, 1, 3, -1, -1, -1, -1, -1, -1, -1],
    [4, 9, 1, 4, 1, 7, 0, 8, 1, 8, 7, 1, -1, -1, -1, -1],
    [4, 0, 3, 7, 4, 3, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1],
    [4, 8, 7, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1],
    [9, 10, 8, 10, 11, 8, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1],
    [3, 0, 9, 3, 9, 11, 11, 9, 10, -1, -1, -1, -1, -1, -1, -1],
    [0, 1, 10, 0, 10, 8, 8, 10, 11, -1, -1, -1, -1, -1, -1, -1],
    [3, 1, 10, 11, 3, 10, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1],
    [1, 2, 11, 1, 11, 9, 9, 11, 8, -1, -1, -1, -1, -1, -1, -1],
    [3, 0, 9, 3, 9, 11, 1, 2, 9, 2, 11, 9, -1, -1, -1, -1],
    [0, 2, 11, 8, 0, 11, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1],
    [3, 2, 11, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1],
    [2, 3, 8, 2, 8, 10, 10, 8, 9, -1, -1, -1, -1, -1, -1, -1],
    [9, 10, 2, 0, 9, 2, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1],
    [2, 3, 8, 2, 8, 10, 0, 1, 8, 1, 10, 8, -1, -1, -1, -1],
    [1, 10, 2, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1],
    [1, 3, 8, 9, 1, 8, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1],
    [0, 9, 1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1],
    [0, 3, 8, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1],
    [-1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1],
];

#[cfg(test)]
mod tests {
    use super::*;

    fn triangles(case: u8) -> Vec<[i8; 3]> {
        let row = &TRI_TABLE[case as usize];
        row.chunks_exact(3)
            .take_while(|t| t[0] != -1)
            .map(|t| [t[0], t[1], t[2]])
            .collect()
    }

    fn solid_corner(case: u8, k: u8) -> bool {
        (case >> k) & 1 == 1
    }

    /// Cases 0 and 255 (all air / all solid) are empty, with no crossed edges.
    #[test]
    fn empty_cases() {
        assert!(triangles(0).is_empty());
        assert!(triangles(255).is_empty());
        assert_eq!(EDGE_TABLE[0], 0);
        assert_eq!(EDGE_TABLE[255], 0);
    }

    /// Every non-sentinel entry is a valid edge index, rows are triangle
    /// triples, and rows are -1-padded to width 16.
    #[test]
    fn table_shape() {
        for case in 0..=255u8 {
            let row = &TRI_TABLE[case as usize];
            let core_len = row.iter().position(|&v| v == -1).unwrap();
            assert!(row[core_len..].iter().all(|&v| v == -1), "case {case}: not padded");
            assert_eq!(core_len % 3, 0, "case {case}: not a multiple of 3");
            assert!(core_len <= 15, "case {case}: more than 5 triangles");
            assert!(row[..core_len].iter().all(|&v| (0..12).contains(&v)), "case {case}: bad edge index");
        }
    }

    /// A single solid corner cuts exactly one triangle out of the cell.
    #[test]
    fn single_corner_cases_have_one_triangle() {
        for case in [1u8, 2, 4, 8, 16, 32, 64, 128] {
            assert_eq!(triangles(case).len(), 1, "case {case}");
        }
    }

    /// EDGE_TABLE agrees with corner solidity: an edge is crossed iff its
    /// two corners differ — checked for every case and edge. Also, every
    /// edge referenced by a triangle is crossed in that case.
    #[test]
    fn edge_table_matches_corner_signs() {
        for case in 0..=255u8 {
            let mask = EDGE_TABLE[case as usize];
            for (e, &[a, b]) in EDGE_CORNERS.iter().enumerate() {
                let crossed = solid_corner(case, a) != solid_corner(case, b);
                assert_eq!(
                    (mask >> e) & 1 == 1,
                    crossed,
                    "case {case}, edge {e}: edge bit disagrees with corner signs"
                );
            }
            for tri in triangles(case) {
                for &edge in &tri {
                    assert!(
                        (mask >> edge) & 1 == 1,
                        "case {case}: triangle uses uncrossed edge {edge}"
                    );
                }
            }
        }
    }

    // ---- mesher tests ----

    use crate::terrain::chunk::Blocks;
    use crate::terrain::generate::{fill_blocks, test_support, TerrainGenerator};
    use crate::terrain::Face;
    use bevy::math::DVec3;
use crate::terrain::mesh::blocky;

    use test_support::chunk_at;

    /// A small Front chunk well inside Front's pyramid: world box
    /// x,y ∈ [−64, −32], z ∈ [224, 256] — every cell is Front-owned.
    fn front_chunk() -> Chunk {
        chunk_at(
            &Face::Front,
            256.0,
            (192, 224),
            (192, 224),
            (224, 256),
        )
    }

    fn world_box(chunk: &Chunk) -> (DVec3, DVec3) {
        let c = chunk.center.as_dvec3();
        let h = (chunk.length as f64) / 2.0;
        (c - DVec3::splat(h), c + DVec3::splat(h))
    }

    fn all_positions_inside(chunk: &Chunk, out: &McOutput) {
        let (min, max) = world_box(chunk);
        for p in &out.positions {
            let p = DVec3::new(p[0] as f64, p[1] as f64, p[2] as f64) + chunk.center.as_dvec3();
            assert!(
                p.cmplt(max + DVec3::splat(1e-4)).all() && p.cmpgt(min - DVec3::splat(1e-4)).all(),
                "vertex {p:?} outside the chunk box {min:?}..{max:?}"
            );
        }
    }

    #[test]
    fn uniform_views_produce_nothing() {
        let chunk = front_chunk();
        let grid = FaceGrid::new(&chunk.face, 256.0);
        for solid in [false, true] {
            let view = OccupancyView::from_fn(|_, _, _| solid);
            let mut out = McOutput::default();
            marching_cubes(&view, &Blocks::default(), &grid, &chunk, &mut out);
            assert!(out.indices.is_empty(), "solid = {solid}");
        }
    }

    #[test]
    fn single_solid_point_cuts_eight_triangles() {
        // Root-level Front chunk (step 16); one solid sample at lattice
        // (272, 272, 272) — world (16, 16, 272), inside Front's pyramid.
        let chunk = chunk_at(
            &Face::Front,
            256.0,
            (0, 512),
            (0, 512),
            (0, 512),
        );
        let grid = FaceGrid::new(&chunk.face, 256.0);
        let view = OccupancyView::from_fn(|i, j, k| (i, j, k) == (17, 17, 17));
        let mut out = McOutput::default();
        marching_cubes(&view, &Blocks::default(), &grid, &chunk, &mut out);
        // The 8 cells around the point each cut exactly one triangle.
        assert_eq!(out.indices.len() / 3, 8);
        assert_eq!(out.positions.len(), 24); // 3 distinct vertices per cell, no sharing
        all_positions_inside(&chunk, &out);
    }

    /// One triangle's world-space normal from the output buffers.
    fn triangle_normal(chunk: &Chunk, out: &McOutput, t: usize) -> DVec3 {
        let v = |i: usize| {
            let p = out.positions[out.indices[t * 3 + i] as usize];
            DVec3::new(p[0] as f64, p[1] as f64, p[2] as f64) + chunk.center.as_dvec3()
        };
        let (a, b, c) = (v(0), v(1), v(2));
        (b - a).cross(c - a)
    }

    #[test]
    fn plane_sheet_on_right_handed_face() {
        // Front (right-handed): solid below world z = 240 → crossing between
        // samples z=239 and z=240 → midpoint plane z = 239.5. All 32×32
        // cells at the k=15 row are mixed → 1024 cells × 2 triangles.
        let chunk = front_chunk();
        let grid = FaceGrid::new(&chunk.face, 256.0);
        // world z = 224 + k (step 1); solid iff z < 240 → k < 16.
        let view = OccupancyView::from_fn(|_i, _j, k| k < 16);
        let mut out = McOutput::default();
        marching_cubes(&view, &Blocks::default(), &grid, &chunk, &mut out);

        assert_eq!(out.indices.len() / 3, 2 * 32 * 32);
        all_positions_inside(&chunk, &out);
        // Every vertex lies on the midpoint plane, chunk-local z = −0.5.
        for p in &out.positions {
            assert!((p[2] + 0.5).abs() < 1e-4, "vertex off the plane: {p:?}");
        }
        // Normals point out of the solid (upward, +z) after the winding fix.
        for t in 0..out.indices.len() / 3 {
            assert!(
                triangle_normal(&chunk, &out, t).z > 0.0,
                "triangle {t} normal points into the solid"
            );
        }
    }

    #[test]
    fn plane_sheet_on_left_handed_face() {
        // Top (left-handed frame): same sheet through world y = 239.5.
        // Top's lattice: world = (gx−S, gz, gy−S), so the face-remapped
        // ranges are x: world x, y: world z, z: world y (radial).
        let chunk = chunk_at(
            &Face::Top,
            256.0,
            (192, 224), // world x ∈ [−64, −32]
            (192, 224), // world z ∈ [−64, −32]
            (224, 256), // world y ∈ [224, 256]
        );
        let grid = FaceGrid::new(&chunk.face, 256.0);
        // world y = 224 + k (gz = 224 + k); solid iff y < 240 → k < 16.
        let view = OccupancyView::from_fn(|_i, _j, k| k < 16);
        let mut out = McOutput::default();
        marching_cubes(&view, &Blocks::default(), &grid, &chunk, &mut out);

        assert_eq!(out.indices.len() / 3, 2 * 32 * 32);
        all_positions_inside(&chunk, &out);
        for p in &out.positions {
            assert!((p[1] + 0.5).abs() < 1e-4, "vertex off the plane: {p:?}");
        }
        for t in 0..out.indices.len() / 3 {
            assert!(
                triangle_normal(&chunk, &out, t).y > 0.0,
                "triangle {t} normal points into the solid"
            );
        }
    }

    #[test]
    fn ownership_gate_skips_foreign_cells() {
        // Front chunk pushed to the +x edge: world x ∈ [224, 256], z ∈
        // [224, 256]. At the mixed row (min corner z = 239), Front owns a
        // cell iff z > x (ties go to the higher-priority X axis → Right),
        // so x ≤ 238 → the first 15 i-columns only; the rest belong to
        // Right's pyramid and must be skipped: 15·32·2 triangles instead
        // of 32·32·2.
        let chunk = chunk_at(
            &Face::Front,
            256.0,
            (480, 512),
            (192, 224),
            (224, 256),
        );
        let grid = FaceGrid::new(&chunk.face, 256.0);
        let view = OccupancyView::from_fn(|_i, _j, k| k < 16);
        let mut out = McOutput::default();
        marching_cubes(&view, &Blocks::default(), &grid, &chunk, &mut out);
        assert_eq!(out.indices.len() / 3, 2 * 15 * 32);
    }

    #[test]
    fn view_matches_fill_predicate() {
        let scale = 256.0f32;
        let gen = TerrainGenerator::new(scale, 3);
        let chunk = chunk_at(
            &Face::Front,
            scale,
            (0, 512),
            (0, 512),
            (0, 512),
        );
        let grid = FaceGrid::new(&chunk.face, scale);
        let view = OccupancyView::build(&gen, &grid, &chunk);
        let blocks: Blocks = fill_blocks(&gen, &grid, &chunk);
        for k in 0..CHUNK_SIZE {
            for j in 0..CHUNK_SIZE {
                for i in 0..CHUNK_SIZE {
                    assert_eq!(
                        view.solid(i, j, k),
                        blocks.is_solid(i, j, k),
                        "view/fill disagree at ({i}, {j}, {k})"
                    );
                }
            }
        }
    }

    /// The off-thread payload produces exactly the fill's blocks and a
    /// non-empty mesh for a band chunk — the single-sampling guarantee
    /// end to end.
    #[test]
    fn payload_matches_fill_blocks() {
        let scale = 256.0f32;
        let gen = TerrainGenerator::new(scale, 3);
        let chunk = chunk_at(&Face::Front, scale, (0, 512), (0, 512), (0, 512));
        let grid = FaceGrid::new(&chunk.face, scale);
        let payload = generate_chunk_payload(&gen, &grid, &chunk, MeshTier::MarchingCubes);
        let blocks = fill_blocks(&gen, &grid, &chunk);
        for k in 0..CHUNK_SIZE {
            for j in 0..CHUNK_SIZE {
                for i in 0..CHUNK_SIZE {
                    assert_eq!(
                        payload.blocks.get(i, j, k),
                        blocks.get(i, j, k),
                        "payload/fill disagree at ({i}, {j}, {k})"
                    );
                }
            }
        }
        assert!(!payload.indices.is_empty());
        assert_eq!(payload.indices.len() % 3, 0);
        assert_eq!(payload.normals.len(), payload.positions.len());
        // Gradient normals: unit length, pointing out of the solid —
        // toward decreasing density, which also holds on cave ceilings —
        // and deviating from the radial direction where the terrain has
        // relief (otherwise lighting shades the planet as a bare sphere).
        let mut max_radial_deviation = 0.0f64;
        for (p, n) in payload.positions.iter().zip(&payload.normals) {
            let world = DVec3::new(p[0] as f64, p[1] as f64, p[2] as f64)
                + chunk.center.as_dvec3();
            let n = DVec3::new(n[0] as f64, n[1] as f64, n[2] as f64);
            assert!((n.length() - 1.0).abs() < 1e-3, "normal not unit: {n:?}");
            let delta = 0.5;
            let toward_air = gen.density(world + n * delta);
            let toward_solid = gen.density(world - n * delta);
            assert!(
                toward_air < toward_solid,
                "normal points into the solid at {world:?}"
            );
            max_radial_deviation =
                max_radial_deviation.max(1.0 - n.dot(world.normalize()));
        }
        assert!(
            max_radial_deviation > 0.01,
            "normals are purely radial — the gradient is not reaching the mesh"
        );
        // Vertex colors: one per vertex, opaque, and both surface (Grass)
        // and subsurface/cliff (Stone) colors present.
        assert_eq!(payload.colors.len(), payload.positions.len());
        for c in &payload.colors {
            assert_eq!(c[3], 1.0, "vertex color not opaque: {c:?}");
        }
        assert!(
            payload.colors.contains(&BlockType::Grass.color()),
            "no Grass-colored vertex — surface coloring missing"
        );
        assert!(
            payload.colors.contains(&BlockType::Stone.color()),
            "no Stone-colored vertex — cliff/subsurface coloring missing"
        );
    }

    /// Ring-corner vertex colors: a border crossing whose solid corner is
    /// a +1 ring point that IS its column's surface block must be Grass —
    /// the old Stone hardcode painted every chunk border gray.
    #[test]
    fn ring_corners_take_the_column_surface_color() {
        // Front chunk, world box x,y ∈ [−64, −32], z ∈ [224, 256], step 1.
        let chunk = chunk_at(&Face::Front, 256.0, (192, 224), (192, 224), (224, 256));
        let grid = FaceGrid::new(&chunk.face, 256.0);
        // The ring column's surface sits one step higher than the own
        // columns': own blocks solid for k < 15, ring (i = 32) solid for
        // k < 16. The border cell (31, j, 15) crosses between its own
        // air corner (31, k = 15) and the solid ring corner (32, k = 15) —
        // which is that ring column's outermost solid (the surface block).
        let view = OccupancyView::from_fn(|i, _j, k| {
            if i == VIEW_SIZE as isize - 1 {
                k < 16
            } else {
                k < 15
            }
        });
        let blocks = blocks_from_view(&view, &grid);
        let mut out = McOutput::default();
        marching_cubes(&view, &blocks, &grid, &chunk, &mut out);
        assert!(!out.colors.is_empty());

        // The x-edge crossing vertices of the border cells sit at
        // chunk-local x = +15.5 (midpoint of i = 31 and the ring), z = −1
        // (k = 15). All of them must be Grass-colored.
        let mut border_vertices = 0;
        for (p, c) in out.positions.iter().zip(&out.colors) {
            if (p[0] - 15.5).abs() < 1e-3 && (p[2] + 1.0).abs() < 1e-3 {
                border_vertices += 1;
                assert_eq!(
                    *c,
                    BlockType::Grass.color(),
                    "border vertex at {p:?} is not Grass-colored"
                );
            }
        }
        assert!(border_vertices > 0, "no border-crossing vertices found");
    }
}
