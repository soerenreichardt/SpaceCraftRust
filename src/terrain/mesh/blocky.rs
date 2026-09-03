//! Block-terrain mesher for the near tier: one cube face per solid/air
//! interface in the chunk's `Blocks`. At leaf level the block step is
//! exactly 1 world unit — blocks ARE voxels — so this renders the block
//! world itself (what a future mining/building tier will edit).
//!
//! Border faces between two chunks are emitted exactly once **by the
//! chunk that owns the solid block**: every solid block emits its own
//! air-facing faces in all six directions, reading neighbor solidity from
//! the occupancy view's rings on both sides (−1 = the minus neighbor's
//! outermost block, 32 = the plus neighbor's block 0). A shared plane's
//! face has exactly one solid block on exactly one chunk, so duplication
//! is impossible by construction — and no face is ever skipped (the
//! earlier minus-side-ownership rule left a face unmeshed whenever the
//! solid block sat on the plus side of the border: the side-face holes).
//! At tier/LOD boundaries the neighbor may be a marching-cubes chunk; its
//! smooth surface approximates the seam (the accepted LOD-boundary
//! bucket).

use crate::terrain::block::BlockType;
use crate::terrain::chunk::{Blocks, Chunk};
use crate::terrain::generate::FaceGrid;
use crate::terrain::mesh::marching_cubes::{McOutput, OccupancyView, VIEW_SIZE};
use crate::terrain::terrain_octree::DimensionRange;
use crate::terrain::CHUNK_SIZE;
use bevy::math::DVec3;

/// Per face: (lattice axis, sign, 4 corner offsets in lattice units).
/// Corners are wound so the lattice-space normal points out of the solid
/// block; reflections (left-handed frames) reverse the winding — the
/// opposite of the marching-cubes rule, because here the winding is
/// authored outward instead of coming from Bourke's into-solid table.
const FACES: [(usize, i8, [[u8; 3]; 4]); 6] = [
    (0, 1, [[1, 0, 0], [1, 1, 0], [1, 1, 1], [1, 0, 1]]),
    (0, -1, [[0, 0, 0], [0, 0, 1], [0, 1, 1], [0, 1, 0]]),
    (1, 1, [[0, 1, 0], [0, 1, 1], [1, 1, 1], [1, 1, 0]]),
    (1, -1, [[0, 0, 0], [1, 0, 0], [1, 0, 1], [0, 0, 1]]),
    (2, 1, [[0, 0, 1], [1, 0, 1], [1, 1, 1], [0, 1, 1]]),
    (2, -1, [[0, 0, 0], [0, 1, 0], [1, 1, 0], [1, 0, 0]]),
];

/// Emits cube faces for every solid/air interface in the chunk's blocks,
/// reading neighbor solidity from the occupancy view (own cells + the +1
/// ring). Per-face axis normals (from the frame's world axes), per-face
/// block colors.
/// The "top" face direction in lattice coordinates: radially outward —
/// the +gz axis on outward-gz faces, −gz on inward-gz ones.
fn axis_of_top(grid: &FaceGrid) -> (usize, i8) {
    if grid.gz_points_outward() {
        (2, 1)
    } else {
        (2, -1)
    }
}

pub(crate) fn blocky_mesh(
    view: &OccupancyView,
    blocks: &Blocks,
    grid: &FaceGrid,
    chunk: &Chunk,
    out: &mut McOutput,
) {
    let face = chunk.face.clone();
    let x_range = chunk.range.x(face.clone());
    let y_range = chunk.range.y(face.clone());
    let z_range = chunk.range.z(face);
    let step = |r: &DimensionRange| (r.1 - r.0) as f64 / CHUNK_SIZE as f64;
    let (step_x, step_y, step_z) = (step(x_range), step(y_range), step(z_range));
    let (x0, y0, z0) = (x_range.0 as f64, y_range.0 as f64, z_range.0 as f64);
    let center = chunk.center.as_dvec3();
    let flip = grid.is_left_handed();

    for k in 0..CHUNK_SIZE {
        for j in 0..CHUNK_SIZE {
            for i in 0..CHUNK_SIZE {
                let block = blocks.get(i, j, k);
                if block == BlockType::Air {
                    continue;
                }
                // Grass grows radially: green on the top (outward) face,
                // earthy dirt on the sides and underside.
                let top = axis_of_top(grid);
                'faces: for &(axis, sign, corners) in &FACES {
                    // Neighbor cell solidity, via the view's rings on both
                    // sides (−1 ..= 32) — a face is emitted iff the
                    // neighbor is air, regardless of which chunk owns it.
                    let (ni, nj, nk) = match axis {
                        0 => (i as isize + sign as isize, j as isize, k as isize),
                        1 => (i as isize, j as isize + sign as isize, k as isize),
                        _ => (i as isize, j as isize, k as isize + sign as isize),
                    };
                    if view.solid_at(ni, nj, nk) {
                        continue 'faces; // buried face
                    }
                    let color = if block == BlockType::Grass && (axis, sign) != top {
                        BlockType::Dirt.color()
                    } else {
                        block.color()
                    };

                    // Face quad: 4 corners, one normal, one color each.
                    let base = out.positions.len() as u32;
                    for c in &corners {
                        let gx = x0 + (i + c[0] as usize) as f64 * step_x;
                        let gy = y0 + (j + c[1] as usize) as f64 * step_y;
                        let gz = z0 + (k + c[2] as usize) as f64 * step_z;
                        let p = grid.position(gx, gy, gz) - center;
                        out.positions.push([p.x as f32, p.y as f32, p.z as f32]);
                        out.colors.push(color);
                    }
                    let n = grid.lattice_axis(axis) * (sign as f64);
                    let n = [n.x as f32, n.y as f32, n.z as f32];
                    for _ in 0..4 {
                        out.normals.push(n);
                    }
                    let idx = |o: u32| base + o;
                    let quad = if flip {
                        [idx(0), idx(2), idx(1), idx(0), idx(3), idx(2)]
                    } else {
                        [idx(0), idx(1), idx(2), idx(0), idx(2), idx(3)]
                    };
                    out.indices.extend_from_slice(&quad);
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::terrain::generate::test_support;
    use crate::terrain::mesh::marching_cubes::{blocks_from_view, OccupancyView};
    use crate::terrain::Face;

    fn mesh_with(
        face: &Face,
        solidity: impl Fn(isize, isize, isize) -> bool,
    ) -> (McOutput, Chunk) {
        let chunk = test_support::chunk_at(face, 256.0, (192, 224), (192, 224), (224, 256));
        let grid = FaceGrid::new(face, 256.0);
        let view = OccupancyView::from_fn(|i, j, k| solidity(i, j, k));
        let blocks = blocks_from_view(&view, &grid);
        let mut out = McOutput::default();
        blocky_mesh(&view, &blocks, &grid, &chunk, &mut out);
        (out, chunk)
    }

    /// Winding and the normal attribute must agree (the cross product of
    /// each triangle's world corners points along its normal), and normals
    /// point away from the block — the frame-agnostic handedness check,
    /// run on one right-handed and one left-handed face. Block 16 sits at
    /// the chunk center, so its local cell center is (0.5, 0.5, 0.5).
    #[test]
    fn winding_matches_normals_on_both_handednesses() {
        for face in [Face::Front, Face::Top] {
            let (out, _chunk) = mesh_with(&face, |i, j, k| (i, j, k) == (16, 16, 16));
            // One isolated block: exactly six faces.
            assert_eq!(out.indices.len(), 12 * 3, "{face:?}");
            assert_eq!(out.positions.len(), 24, "{face:?}");
            let block_center = DVec3::splat(0.5);
            for t in 0..out.indices.len() / 3 {
                let v = |o: usize| {
                    let p = out.positions[out.indices[t * 3 + o] as usize];
                    DVec3::new(p[0] as f64, p[1] as f64, p[2] as f64)
                };
                let (a, b, c) = (v(0), v(1), v(2));
                let cross = (b - a).cross(c - a);
                let n = out.normals[out.indices[t * 3] as usize];
                let n = DVec3::new(n[0] as f64, n[1] as f64, n[2] as f64);
                assert!(
                    cross.dot(n) > 0.0,
                    "{face:?}: triangle {t} winding disagrees with its normal"
                );
                let face_center = (a + b + c) / 3.0;
                assert!(
                    n.dot(face_center - block_center) > 0.0,
                    "{face:?}: triangle {t} normal points into the block"
                );
            }
        }
    }

    /// Adjacent solid blocks cull their shared face.
    #[test]
    fn adjacent_blocks_share_faces() {
        let (out, _) = mesh_with(&Face::Front, |i, j, k| {
            (i, j, k) == (16, 16, 16) || (i, j, k) == (17, 16, 16)
        });
        // 12 faces minus the 2 shared sides = 10 faces = 20 triangles.
        assert_eq!(out.indices.len(), 20 * 3);
    }

    /// Border completeness: a chunk whose own blocks are all solid with
    /// air on BOTH rings emits faces on all six borders (6·32² — the old
    /// rule skipped the three minus borders and left holes); fully
    /// enclosed (solid rings too) it emits nothing.
    #[test]
    fn border_faces_emit_on_all_six_sides() {
        let (out, _) = mesh_with(&Face::Front, |i, j, k| {
            (0..CHUNK_SIZE as isize).contains(&i)
                && (0..CHUNK_SIZE as isize).contains(&j)
                && (0..CHUNK_SIZE as isize).contains(&k)
        });
        assert_eq!(out.indices.len(), 6 * 32 * 32 * 6);

        let (out, _) = mesh_with(&Face::Front, |_i, _j, _k| true);
        assert!(out.indices.is_empty(), "fully enclosed chunk emitted faces");
    }

    /// Grass blocks are green on top (radially outward) and dirt-colored
    /// on the sides — on both gz orientations.
    #[test]
    fn grass_tops_are_green_sides_are_dirt() {
        for face in [Face::Front, Face::Back] {
            // A single surface block, isolated in air.
            let (out, _) = mesh_with(&face, |i, j, k| (i, j, k) == (16, 16, 16));
            let grid = FaceGrid::new(&face, 256.0);
            let (top_axis, top_sign) = if grid.gz_points_outward() {
                (2, 1)
            } else {
                (2, -1)
            };
            // Rebuild the face each vertex belongs to: match its normal.
            let mut green = 0;
            let mut dirt = 0;
            for &idx in &out.indices {
                let n = out.normals[idx as usize];
                let expected_top = grid.lattice_axis(top_axis) * (top_sign as f64);
                let is_top = (n[0] as f64 - expected_top.x).abs() < 1e-3
                    && (n[1] as f64 - expected_top.y).abs() < 1e-3
                    && (n[2] as f64 - expected_top.z).abs() < 1e-3;
                let c = out.colors[idx as usize];
                if is_top {
                    green += 1;
                } else if c == BlockType::Dirt.color() {
                    dirt += 1;
                }
            }
            assert_eq!(green, 6, "{face:?}: top face triangles not grass-green");
            assert!(dirt >= 18, "{face:?}: side faces not dirt-colored");
        }
    }
}
