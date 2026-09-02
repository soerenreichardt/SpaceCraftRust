use crate::terrain::Face;
use bevy::math::DVec3;
use bevy::prelude::Vec3;
use noiz::prelude::common_noise::{Fbm, Simplex};
use noiz::prelude::{
    FractalLayers, LayeredNoise, Noise, Normed, Persistence, Sampleable, ScalableNoise,
    SeedableNoise,
};

/// Lattice→world mapping for one planet face.
///
/// The lattice is the global per-face voxel grid (`gx, gy, gz ∈ [0, 2·scale]`,
/// one lattice unit = one world unit) that every chunk's `NodeRange` windows
/// into — it is not chunk-local. The axes below are the world axes those
/// ranges actually run along: `Octant::center_on_unit_cube` and
/// `NodeRange::split` split on world axes, so increasing range values always
/// run toward the +axis. This deliberately does NOT use
/// `Face::perpendicular_vectors()`, whose signs disagree with this frame on
/// four of the six faces (which would mirror terrain across chunk seams —
/// see the plan's Finding 1).
pub(crate) struct FaceGrid {
    scale: f64,
    /// `n * scale` — the root cube's face-plane center.
    origin: DVec3,
    /// World axes of the (gx, gy, gz) ranges.
    e1: DVec3,
    e2: DVec3,
    e3: DVec3,
}

impl FaceGrid {
    pub(crate) fn new(face: &Face, scale: f32) -> Self {
        let (e1, e2, e3) = match face {
            Face::Back | Face::Front => (Vec3::X, Vec3::Y, Vec3::Z),
            Face::Top | Face::Bottom => (Vec3::X, Vec3::Z, Vec3::Y),
            Face::Left | Face::Right => (Vec3::Z, Vec3::Y, Vec3::X),
        };
        FaceGrid {
            scale: scale as f64,
            origin: DVec3::from(face.direction_vector()) * scale as f64,
            e1: DVec3::from(e1),
            e2: DVec3::from(e2),
            e3: DVec3::from(e3),
        }
    }

    /// Flat-cube position of lattice `(gx, gy, gz)`, fractional values
    /// allowed (the MC mesher interpolates crossings in lattice space). The
    /// lattice spans `[0, 2·scale]` with one unit per world unit, so the root
    /// midpoint `(scale, scale, scale)` maps to the face-plane center;
    /// `gz = scale` lies on the face plane for every face, and whether
    /// increasing gz points outward or inward depends on the face (outward
    /// for Top/Right/Back, inward for Bottom/Left/Front).
    pub(crate) fn position(&self, gx: f64, gy: f64, gz: f64) -> DVec3 {
        let s = self.scale;
        self.origin
            + self.e1 * (gx - s)
            + self.e2 * (gy - s)
            + self.e3 * (gz - s)
    }

    /// Cube→sphere projection — **flat-tier-only** (bends `CurvedBasePlane`'s
    /// artificially planar grid onto the sphere). The density field is
    /// sampled at the raw flat positions, never projected ones (projected
    /// input is constant along origin rays ⇒ heightfield, no caves); and MC
    /// output vertices stay at their true flat crossing positions, or every
    /// mountain would flatten to sea level.
    pub(crate) fn project(&self, p: DVec3) -> DVec3 {
        p.normalize() * self.scale
    }

    /// Whether this face's lattice frame is left-handed. Left-handed faces
    /// need a triangle-winding flip when meshing.
    pub(crate) fn is_left_handed(&self) -> bool {
        self.e1.cross(self.e2).dot(self.e3) < 0.0
    }
}

/// Fractal brownian motion over 3D simplex: 6 octaves, each half the weight
/// (`Persistence`) and twice the frequency (`lacunarity`) of the last.
/// `Normed` collects the octaves into a weighted average, so the output
/// stays bounded in roughly [-1, 1].
type TerrainFbm = Fbm<Simplex>;

/// Base-frequency knob: the coarsest octave places roughly this many
/// features per planet radius; each octave doubles the count — continents
/// down to hills and roughness.
const FEATURES_PER_RADIUS: f32 = 4.0;
/// Terrain amplitude as a fraction of the planet radius: the peak
/// displacement of mountains above / valleys below sea level. Must stay
/// above the coarsest LOD block step (`scale/16`, so even the root level
/// sees a non-uniform shell) and below `(√2−1)·scale ≈ 0.41` (the coverage
/// bound for peaks near cube edges — see the plan).
const AMPLITUDE_FRACTION: f32 = 0.1;

/// Per-planet terrain signature: a seeded fbm, sea level, and displacement
/// amplitude. Owned by the `Planet` component — one per planet, since every
/// planet has its own signature.
pub(crate) struct TerrainGenerator {
    noise: Noise<TerrainFbm>,
    /// Sea level: the radius of the undisplaced sphere.
    sea_level: f64,
    /// Peak displacement above/below sea level in world units.
    amplitude: f64,
}

impl TerrainGenerator {
    pub(crate) fn new(scale: f32, seed: u32) -> Self {
        let mut noise: Noise<TerrainFbm> = LayeredNoise::new(
            Normed::<f32>::default(),
            Persistence(0.5),
            FractalLayers {
                layer: Default::default(),
                lacunarity: 2.0,
                amount: 6,
            },
        )
        .into();
        noise.set_seed(seed);
        // Cycles per world unit: the same feature count per radius for
        // planets of any size. `4/scale` is a power of two for our scales
        // (`2^(radius-1)·32`), so it is exact in f32.
        noise.set_frequency(FEATURES_PER_RADIUS / scale);
        TerrainGenerator {
            noise,
            sea_level: scale as f64,
            amplitude: (AMPLITUDE_FRACTION * scale) as f64,
        }
    }

    /// Signed density at a flat planet-space position: positive inside
    /// (solid), negative outside (air), zero on the terrain surface —
    /// wherever the fbm exceeds the signed distance from sea level.
    /// Sampled at the **true flat position**, so the field varies in all
    /// three dimensions and multiple surface crossings along a ray are
    /// possible (caves, overhangs). A projected input would be constant
    /// along origin rays and collapse this to a heightfield.
    pub(crate) fn density(&self, p: DVec3) -> f64 {
        let fbm =
            self.noise.sample_for::<f32>(Vec3::new(p.x as f32, p.y as f32, p.z as f32)) as f64;
        self.amplitude * fbm - (p.length() - self.sea_level)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::terrain::octree::Octant;
    use crate::terrain::terrain_octree::{DimensionRange, NodeRange};

    fn octants() -> [Octant; 8] {
        [
            Octant::TopLeftFront,
            Octant::TopLeftBack,
            Octant::TopRightFront,
            Octant::TopRightBack,
            Octant::BottomLeftFront,
            Octant::BottomLeftBack,
            Octant::BottomRightFront,
            Octant::BottomRightBack,
        ]
    }

    fn mid(range: &DimensionRange) -> f64 {
        (range.0 + range.1) as f64 / 2.0
    }

    /// Lattice position of a range's center, through the face-remapped
    /// accessors — the same axes the block fill will use.
    fn range_center(grid: &FaceGrid, range: &NodeRange, face: &Face) -> DVec3 {
        grid.position(
            mid(range.x(face.clone())),
            mid(range.y(face.clone())),
            mid(range.z(face.clone())),
        )
    }

    fn assert_near(a: DVec3, b: DVec3, scale: f32) {
        let tolerance = 1e-6 * scale as f64;
        assert!(
            (a - b).length() <= tolerance,
            "mismatch: {a:?} vs {b:?} (tolerance {tolerance})"
        );
    }

    /// The frame must match the octree's own placement math, not just itself:
    /// for every face and octant, the split child's range-midpoint position
    /// must equal `parent center + octant.center_on_unit_cube() * child
    /// length` (how `TerrainOctreeNode::split` places chunks), recursively.
    /// The root range is built exactly as `TerrainOctree::new` does:
    /// `from_scale(2·scale)` — the lattice spans the full face width.
    #[test]
    fn face_grid_matches_octree_placement() {
        for scale in [256.0f32, 2048.0] {
            for face in [
                Face::Back,
                Face::Front,
                Face::Left,
                Face::Right,
                Face::Top,
                Face::Bottom,
            ] {
                let grid = FaceGrid::new(&face, scale);
                let face_center = DVec3::from(face.direction_vector()) * scale as f64;
                let root = NodeRange::from_scale(2.0 * scale);

                // Root midpoint sits on the face plane.
                assert_near(range_center(&grid, &root, &face), face_center, scale);

                for octant in octants() {
                    let expected = face_center + DVec3::from(octant.center_on_unit_cube()) * scale as f64;
                    let child = root.split(octant);
                    assert_near(range_center(&grid, &child, &face), expected, scale);

                    // One split deeper: grandchild length is scale/2.
                    for octant2 in octants() {
                        let expected2 =
                            expected + DVec3::from(octant2.center_on_unit_cube()) * (scale as f64 / 2.0);
                        let grandchild = child.split(octant2);
                        assert_near(range_center(&grid, &grandchild, &face), expected2, scale);
                    }
                }
            }
        }
    }

    /// Finding 1: Back/Front frames are right-handed, the other four are
    /// left-handed and will need a winding flip when meshing.
    #[test]
    fn face_handedness() {
        for face in [Face::Back, Face::Front] {
            assert!(!FaceGrid::new(&face, 256.0).is_left_handed(), "{face:?}");
        }
        for face in [Face::Left, Face::Right, Face::Top, Face::Bottom] {
            assert!(FaceGrid::new(&face, 256.0).is_left_handed(), "{face:?}");
        }
    }

    const TEST_POINTS: [DVec3; 4] = [
        DVec3::new(100.0, 20.0, 250.0),
        DVec3::new(-200.0, 0.0, 100.0),
        DVec3::new(0.0, 256.0, 0.0),
        DVec3::new(60.0, 60.0, 60.0),
    ];

    /// Same signature (scale, seed) → identical field; different seed → a
    /// different field.
    #[test]
    fn terrain_density_is_deterministic_per_seed() {
        let a = TerrainGenerator::new(256.0, 7);
        let b = TerrainGenerator::new(256.0, 7);
        let c = TerrainGenerator::new(256.0, 8);
        for p in TEST_POINTS {
            assert_eq!(a.density(p), b.density(p), "at {p:?}");
        }
        assert!(
            TEST_POINTS.iter().any(|p| a.density(*p) != c.density(*p)),
            "different seed produced the same field at all test points"
        );
    }

    /// The fbm is a weighted average of [-1, 1] octaves, so the surface
    /// stays within the amplitude of sea level — the bound the shell
    /// shortcut and band test rely on. Also pins deep-inside → solid and
    /// far-outside → air without sampling.
    #[test]
    fn terrain_density_stays_within_amplitude_of_sea_level() {
        let gen = TerrainGenerator::new(256.0, 3);
        let amp = 0.1 * 256.0;
        // Low-discrepancy directions over the sphere (golden-angle spiral).
        let golden = std::f64::consts::TAU / (1.0 + 5.0f64.sqrt());
        let mut max_displacement = 0.0f64;
        for i in 0..2000 {
            let z = 1.0 - 2.0 * (i as f64 + 0.5) / 2000.0;
            let r = (1.0 - z * z).sqrt();
            let phi = i as f64 * golden;
            let dir = DVec3::new(r * phi.cos(), r * phi.sin(), z);
            let d = gen.density(dir * 256.0);
            max_displacement = max_displacement.max(d.abs());
        }
        assert!(
            max_displacement <= amp * 1.05,
            "surface displaced {max_displacement} > amp·1.05 = {}",
            amp * 1.05
        );
        // Deep inside / far outside resolve without needing the noise value.
        assert!(gen.density(DVec3::splat(0.0)) > 0.0, "planet center not solid");
        let far = DVec3::new(0.0, 0.0, 512.0);
        assert!(gen.density(far) < 0.0, "2× radius not air");
    }

    /// The signature is scale-relative: a planet twice the size produces the
    /// same terrain shape at twice the coordinates (density exactly doubles).
    #[test]
    fn terrain_scales_with_planet_scale() {
        let small = TerrainGenerator::new(256.0, 5);
        let big = TerrainGenerator::new(512.0, 5);
        for p in TEST_POINTS {
            let d_small = small.density(p);
            let d_big = big.density(p * 2.0);
            assert!(
                (d_big - 2.0 * d_small).abs() < 1e-2,
                "scale invariance broken at {p:?}: {d_big} vs {}",
                2.0 * d_small
            );
        }
    }
}
