use crate::terrain::block::BlockType;
use crate::terrain::chunk::{Blocks, Chunk};
use crate::terrain::mesh::marching_cubes::OccupancyView;
use crate::terrain::terrain_octree::DimensionRange;
use crate::terrain::{Face, CHUNK_SIZE};
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
#[derive(Clone)]
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
    /// for Front/Top/Right — the positive-normal faces — inward for
    /// Back/Bottom/Left).
    pub(crate) fn position(&self, gx: f64, gy: f64, gz: f64) -> DVec3 {
        let s = self.scale;
        self.origin
            + self.e1 * (gx - s)
            + self.e2 * (gy - s)
            + self.e3 * (gz - s)
    }

    /// Whether increasing gz points away from the planet. True on the
    /// positive-normal faces (Front/Top/Right); on Back/Bottom/Left the
    /// lattice's gz runs from the far outside toward the center plane.
    /// Derived from the frame itself: the radial axis dotted with the face
    /// normal (the direction of `origin`).
    pub(crate) fn gz_points_outward(&self) -> bool {
        self.e3.dot(self.origin) > 0.0
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

/// The face whose angular pyramid contains `p` — the six pyramids (origin
/// through each face square) tile space exactly. The point's dominant world
/// component (by absolute value) picks the axis, its sign picks the
/// positive/negative face, and ties break by fixed axis priority (X over Y
/// over Z) so every point resolves to exactly one face, identically from
/// all six lattices. This is the mesh-exactly-once rule for MC cells: a
/// cell is meshed only by the face owning its min-corner position, which
/// dedups the wedge regions where neighboring face cubes overlap.
pub(crate) fn owning_face(p: DVec3) -> Face {
    let (x, y, z) = (p.x.abs(), p.y.abs(), p.z.abs());
    if x >= y && x >= z {
        if p.x >= 0.0 { Face::Right } else { Face::Left }
    } else if y >= z {
        if p.y >= 0.0 { Face::Top } else { Face::Bottom }
    } else {
        if p.z >= 0.0 { Face::Front } else { Face::Back }
    }
}

/// Whether `face` owns the MC cell whose minimum corner is at this lattice
/// point — min-corner ownership plus the pyramid rule means every world
/// cell is meshed exactly once across the six faces; cross-face agreement
/// on shared corners comes from the shared lattice and the point-canonical
/// density rule.
pub(crate) fn face_owns_cell(grid: &FaceGrid, face: &Face, gx: f64, gy: f64, gz: f64) -> bool {
    owning_face(grid.position(gx, gy, gz)) == *face
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
/// Safety margin on the fbm bound for the shell shortcut and band test:
/// points further than `amplitude · SHELL_MARGIN` from sea level cannot
/// cross the surface (fbm output ≈ [−1, 1]; the margin covers any marginal
/// simplex overshoot).
const SHELL_MARGIN: f64 = 1.1;

/// Per-planet terrain signature: a seeded fbm, sea level, and displacement
/// amplitude. Owned by the `Planet` component — one per planet, since every
/// planet has its own signature.
#[derive(Clone)]
pub(crate) struct TerrainGenerator {
    noise: Noise<TerrainFbm>,
    /// Sea level: the radius of the undisplaced sphere.
    sea_level: f64,
    /// Peak displacement above/below sea level in world units.
    amplitude: f64,
}

impl TerrainGenerator {
    pub(crate) fn new(scale: f32, seed: u32) -> Self {
        Self::with_amplitude(scale, seed, AMPLITUDE_FRACTION)
    }

    /// Constructor with an explicit amplitude fraction (`0.0` = perfect
    /// sphere) — tuning and tests.
    pub(crate) fn with_amplitude(scale: f32, seed: u32, amplitude_fraction: f32) -> Self {
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
            amplitude: (amplitude_fraction * scale) as f64,
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

    /// Signed density at a lattice point, or `None` when the point lies
    /// beyond the fbm's reach (`| |p| − sea_level | > amplitude·SHELL_MARGIN`)
    /// — then solidity follows from the distance alone and no noise is
    /// evaluated. `Some` results feed MC interpolation later.
    pub(crate) fn point_density(
        &self,
        grid: &FaceGrid,
        gx: f64,
        gy: f64,
        gz: f64,
    ) -> Option<f64> {
        let p = grid.position(gx, gy, gz);
        let d = p.length() - self.sea_level;
        if d.abs() > self.amplitude * SHELL_MARGIN {
            return None;
        }
        Some(self.density(p))
    }

    /// Whether the block at a lattice point is solid — a pure function of
    /// the lattice point and the planet's signature, so any face evaluating
    /// a shared point gets the same answer (the border-alignment guarantee).
    pub(crate) fn is_solid_at(&self, grid: &FaceGrid, gx: f64, gy: f64, gz: f64) -> bool {
        let p = grid.position(gx, gy, gz);
        let d = p.length() - self.sea_level;
        if d.abs() > self.amplitude * SHELL_MARGIN {
            return d < 0.0;
        }
        self.density(p) > 0.0
    }

    /// Surface normal at a near-surface point, from the density gradient
    /// (central differences, step ≈ ⅛ of the finest noise feature):
    /// points out of the solid, toward decreasing density. Unlike a radial
    /// normal — which shades everything as a perfect sphere — this
    /// captures slopes, ridges, and cave ceilings, so the terrain shades
    /// with its actual relief. Falls back to the radial direction if the
    /// gradient vanishes.
    pub(crate) fn surface_normal(&self, p: DVec3) -> DVec3 {
        let h = self.sea_level / 1024.0;
        let dx = self.density(p + DVec3::X * h) - self.density(p - DVec3::X * h);
        let dy = self.density(p + DVec3::Y * h) - self.density(p - DVec3::Y * h);
        let dz = self.density(p + DVec3::Z * h) - self.density(p - DVec3::Z * h);
        let gradient = DVec3::new(dx, dy, dz);
        if gradient.length_squared() < 1e-12 {
            return p.normalize();
        }
        (-gradient).normalize()
    }

    /// Whether the chunk's flat box can intersect the terrain band
    /// `[sea_level − amp·SHELL_MARGIN, sea_level + amp·SHELL_MARGIN]`.
    /// Uses the exact min/max of `|p|` over the chunk's axis-aligned box —
    /// the distance from the origin to the box, and the farthest corner.
    /// (Corner *sampling* would be wrong: `|p|` is convex, so its minimum
    /// need not be at a corner — the root cube's corners all sit outside the
    /// band while it contains the whole planet.)
    pub(crate) fn chunk_intersects_surface(&self, chunk: &Chunk) -> bool {
        matches!(self.band_class(chunk), BandClass::Band)
    }

    pub(crate) fn band_class(&self, chunk: &Chunk) -> BandClass {
        let c = chunk.center.as_dvec3();
        let h = chunk.length as f64 / 2.0;
        let band = self.amplitude * SHELL_MARGIN;
        // Exact closest approach of the box to the origin (0 if inside).
        let min_r = DVec3::new(
            (c.x.abs() - h).max(0.0),
            (c.y.abs() - h).max(0.0),
            (c.z.abs() - h).max(0.0),
        )
        .length();
        // Exact farthest point: align corner signs with the center.
        let max_r = (c.abs() + DVec3::splat(h)).length();
        if min_r >= self.sea_level + band {
            BandClass::Outside
        } else if max_r <= self.sea_level - band {
            BandClass::Inside
        } else {
            BandClass::Band
        }
    }
}

/// Radial classification of a chunk relative to the terrain band.
pub(crate) enum BandClass {
    /// Entirely outside the band — all air.
    Outside,
    /// Entirely inside the band's floor — all solid.
    Inside,
    /// Possibly intersects the surface — needs per-voxel sampling.
    Band,
}

/// Fills a chunk's `Blocks` from the density field. Per (i, j) column, walk
/// gz from the outside in: the first (outermost) solid block gets `Grass`,
/// solids below it `Stone`, air stays `Air` — cave pockets interior to the
/// column get `Stone` ceilings (accepted for now). Off-band chunks fill
/// uniformly with zero noise evaluations.
pub(crate) fn fill_blocks(generator: &TerrainGenerator, grid: &FaceGrid, chunk: &Chunk) -> Blocks {
    use crate::terrain::mesh::marching_cubes::blocks_from_view;
    match generator.band_class(chunk) {
        BandClass::Outside => Blocks::default(),
        BandClass::Inside => Blocks::stone(),
        // The band arm shares one sampling pass with the mesher: the
        // occupancy view's samples ARE the block solidities.
        BandClass::Band => {
            blocks_from_view(&OccupancyView::build(generator, grid, chunk), grid)
        }
    }
}

/// Temporary: log one ASCII slice (solid `#` / air `.`) of the first
/// band-filled chunk — rows gz outward → inward, columns gx, at gy's middle
/// column. Expect solid interior, air exterior, a noisy transition row.
pub(crate) const DEBUG_FILL_SLICE: bool = true;

pub(crate) fn debug_ascii_slice(generator: &TerrainGenerator, grid: &FaceGrid, chunk: &Chunk) {
    use bevy::prelude::info;
    let face = chunk.face.clone();
    let x_range = chunk.range.x(face.clone());
    let y_range = chunk.range.y(face.clone());
    let z_range = chunk.range.z(face);
    let step = |r: &DimensionRange| (r.1 - r.0) as f64 / CHUNK_SIZE as f64;
    let (step_x, step_y, step_z) = (step(x_range), step(y_range), step(z_range));
    let (x0, y0, z0) = (x_range.0 as f64, y_range.0 as f64, z_range.0 as f64);

    let gy = y0 + (CHUNK_SIZE / 2) as f64 * step_y;
    let mut ks: Vec<usize> = (0..CHUNK_SIZE).collect();
    if grid.gz_points_outward() {
        ks.reverse();
    }
    let mut out = String::from("DEBUG fill slice (rows: outward → inward):");
    for k in ks {
        let gz = z0 + k as f64 * step_z;
        let mut row = String::with_capacity(CHUNK_SIZE);
        for i in 0..CHUNK_SIZE {
            let gx = x0 + i as f64 * step_x;
            row.push(if generator.is_solid_at(grid, gx, gy, gz) { '#' } else { '.' });
        }
        out.push('\n');
        out.push_str(&row);
    }
    info!("{out}");
}

/// Shared test support: chunk construction from face-remapped lattice
/// windows, consistent with the accessor mapping and `FaceGrid`.
#[cfg(test)]
pub(crate) mod test_support {
    use super::*;
    use crate::terrain::terrain_octree::NodeRange;
    use bevy::prelude::Entity;

    /// A chunk around hand-chosen face-remapped lattice windows (gx, gy,
    /// gz), with raw stores, center, and length derived consistently.
    pub(crate) fn chunk_at(
        face: &Face,
        scale: f32,
        gx: (u32, u32),
        gy: (u32, u32),
        gz: (u32, u32),
    ) -> Chunk {
        // Raw (x, y, z) stores from the face-remapped windows, per the
        // NodeRange accessor mapping.
        let (xs, ys, zs) = match face {
            Face::Front | Face::Back => (gx, gy, gz),
            Face::Top | Face::Bottom => (gx, gz, gy),
            Face::Left | Face::Right => (gz, gy, gx),
        };
        let grid = FaceGrid::new(face, scale);
        let mid = |r: (u32, u32)| (r.0 + r.1) as f64 / 2.0;
        let center = grid.position(mid(gx), mid(gy), mid(gz));
        Chunk {
            parent: None,
            planet: Entity::PLACEHOLDER,
            center: center.as_vec3(),
            face: face.clone(),
            length: (gx.1 - gx.0) as f32,
            scale,
            blocks: None,
            range: NodeRange::from_ranges(
                DimensionRange(xs.0, xs.1),
                DimensionRange(ys.0, ys.1),
                DimensionRange(zs.0, zs.1),
            ),
        }
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

    /// Finding 1 orientation pin: increasing gz points outward on the
    /// positive-normal faces, inward on the negative ones.
    #[test]
    fn gz_orientation_per_face() {
        for face in [Face::Front, Face::Top, Face::Right] {
            assert!(FaceGrid::new(&face, 256.0).gz_points_outward(), "{face:?}");
        }
        for face in [Face::Back, Face::Bottom, Face::Left] {
            assert!(!FaceGrid::new(&face, 256.0).gz_points_outward(), "{face:?}");
        }
    }

    /// Build a chunk around hand-chosen face-remapped lattice ranges, with
    /// center/length consistent with the FaceGrid mapping (the guard-test
    /// identity: center = position of the range midpoints).
    fn chunk_at(
        face: &Face,
        scale: f32,
        x: (u32, u32),
        y: (u32, u32),
        z: (u32, u32),
    ) -> Chunk {
        test_support::chunk_at(face, scale, x, y, z)
    }

    /// Without noise (`amp = 0`) the fill is the perfect sphere: solid iff
    /// `|p| < scale`, one transition per column, `Grass` on the outermost
    /// solid block, `Stone` below it.
    #[test]
    fn fill_matches_sphere_without_noise() {
        let scale = 256.0f32;
        let gen = TerrainGenerator::with_amplitude(scale, 1, 0.0);
        let face = Face::Front;
        let grid = FaceGrid::new(&face, scale);
        let root = chunk_at(&face, scale, (0, 512), (0, 512), (0, 512));
        assert!(gen.chunk_intersects_surface(&root));
        let blocks = fill_blocks(&gen, &grid, &root);
        // Face-center column (gx = gy = 256 → x = y = 0): p = (0, 0, gz),
        // gz = k·16 → solid iff gz < 256, i.e. k < 16.
        for k in 0..CHUNK_SIZE {
            assert_eq!(blocks.is_solid(16, 16, k), k < 16, "k = {k}");
        }
        assert_eq!(blocks.get(16, 16, 15), BlockType::Grass);
        assert_eq!(blocks.get(16, 16, 14), BlockType::Stone);
        assert_eq!(blocks.get(16, 16, 16), BlockType::Air);
    }

    /// Off-band chunks fill uniformly with zero noise evaluation: fully
    /// inside → all Stone, fully outside → all Air.
    #[test]
    fn fill_is_uniform_off_band() {
        let scale = 256.0f32;
        let gen = TerrainGenerator::new(scale, 1);
        let face = Face::Front;
        let grid = FaceGrid::new(&face, scale);
        // Deep inside: world |p| ≤ ~105 ≪ sea − band.
        let inside = chunk_at(&face, scale, (200, 232), (200, 232), (40, 72));
        let blocks = fill_blocks(&gen, &grid, &inside);
        assert!(!gen.chunk_intersects_surface(&inside));
        for k in 0..CHUNK_SIZE {
            for j in 0..CHUNK_SIZE {
                for i in 0..CHUNK_SIZE {
                    assert_eq!(blocks.get(i, j, k), BlockType::Stone);
                }
            }
        }
        // Far outside: world |p| ≥ ~441 ≫ sea + band.
        let outside = chunk_at(&face, scale, (200, 232), (200, 232), (440, 472));
        let blocks = fill_blocks(&gen, &grid, &outside);
        for k in 0..CHUNK_SIZE {
            for j in 0..CHUNK_SIZE {
                for i in 0..CHUNK_SIZE {
                    assert_eq!(blocks.get(i, j, k), BlockType::Air);
                }
            }
        }
    }

    /// Same signature → identical fill; different seed → a different fill.
    #[test]
    fn fill_is_deterministic_per_seed() {
        let scale = 256.0f32;
        let a = TerrainGenerator::new(scale, 7);
        let b = TerrainGenerator::new(scale, 7);
        let c = TerrainGenerator::new(scale, 8);
        let face = Face::Front;
        let grid = FaceGrid::new(&face, scale);
        let root = chunk_at(&face, scale, (0, 512), (0, 512), (0, 512));
        let blocks_a = fill_blocks(&a, &grid, &root);
        let blocks_b = fill_blocks(&b, &grid, &root);
        let blocks_c = fill_blocks(&c, &grid, &root);
        let mut differs_from_c = false;
        for k in 0..CHUNK_SIZE {
            for j in 0..CHUNK_SIZE {
                for i in 0..CHUNK_SIZE {
                    assert_eq!(blocks_a.get(i, j, k), blocks_b.get(i, j, k));
                    differs_from_c |= blocks_a.get(i, j, k) != blocks_c.get(i, j, k);
                }
            }
        }
        assert!(differs_from_c, "different seed filled identically");
    }

    /// With noise, the face-center column has exactly one `Grass` block and
    /// it is the outermost solid one.
    #[test]
    fn fill_puts_grass_on_the_outermost_solid() {
        let scale = 256.0f32;
        let gen = TerrainGenerator::new(scale, 3);
        let face = Face::Front;
        let grid = FaceGrid::new(&face, scale);
        let root = chunk_at(&face, scale, (0, 512), (0, 512), (0, 512));
        let blocks = fill_blocks(&gen, &grid, &root);
        // k ascends inward → outward on Front: deep Stone first, then the
        // surface block. Invariants: exactly one Grass, and nothing solid
        // outward of it.
        let mut grasses = 0;
        let mut grass_k = None;
        for k in 0..CHUNK_SIZE {
            match blocks.get(16, 16, k) {
                BlockType::Air => {}
                BlockType::Grass => {
                    grasses += 1;
                    grass_k = Some(k);
                }
                _ => {}
            }
        }
        assert_eq!(grasses, 1, "expected exactly one surface block");
        let k = grass_k.expect("no solid in the center column");
        for kk in (k + 1)..CHUNK_SIZE {
            assert!(!blocks.is_solid(16, 16, kk), "solid block above the Grass at k={kk}");
        }
    }

    /// Cross-face agreement: the same world point, addressed through two
    /// faces' lattices, resolves identically — the border-alignment
    /// guarantee end to end.
    #[test]
    fn solidity_agrees_across_faces() {
        let scale = 256.0f32;
        let gen = TerrainGenerator::new(scale, 9);
        let front = FaceGrid::new(&Face::Front, scale);
        let top = FaceGrid::new(&Face::Top, scale);
        // World (150, 100, 120):
        //   Front lattice (gx = x+S, gy = y+S, gz = z)          → (406, 356, 120)
        //   Top lattice   (world x = gx−S, y = gz, z = gy−S)     → (406, 376, 100)
        let p_front = (406.0, 356.0, 120.0);
        let p_top = (406.0, 376.0, 100.0);
        assert_eq!(
            front.position(p_front.0, p_front.1, p_front.2),
            top.position(p_top.0, p_top.1, p_top.2)
        );
        assert_eq!(
            gen.is_solid_at(&front, p_front.0, p_front.1, p_front.2),
            gen.is_solid_at(&top, p_top.0, p_top.1, p_top.2)
        );
        assert_eq!(
            gen.point_density(&front, p_front.0, p_front.1, p_front.2),
            gen.point_density(&top, p_top.0, p_top.1, p_top.2)
        );
    }

    /// Pyramid ownership: interiors resolve to their face, diagonal ties
    /// break by the fixed axis priority X > Y > Z.
    #[test]
    fn pyramid_ownership_resolves_points() {
        assert_eq!(owning_face(DVec3::new(10.0, 0.0, 200.0)), Face::Front);
        assert_eq!(owning_face(DVec3::new(0.0, 0.0, -200.0)), Face::Back);
        assert_eq!(owning_face(DVec3::new(200.0, 10.0, 10.0)), Face::Right);
        assert_eq!(owning_face(DVec3::new(-200.0, 10.0, 10.0)), Face::Left);
        assert_eq!(owning_face(DVec3::new(10.0, 200.0, 10.0)), Face::Top);
        assert_eq!(owning_face(DVec3::new(10.0, -200.0, 10.0)), Face::Bottom);
        // Ties: X beats Y beats Z.
        assert_eq!(owning_face(DVec3::new(100.0, 100.0, 100.0)), Face::Right);
        assert_eq!(owning_face(DVec3::new(-100.0, 100.0, 100.0)), Face::Left);
        assert_eq!(owning_face(DVec3::new(0.0, 100.0, 100.0)), Face::Top);
        assert_eq!(owning_face(DVec3::new(0.0, -100.0, 100.0)), Face::Bottom);
        assert_eq!(owning_face(DVec3::new(0.0, 0.0, 100.0)), Face::Front);
    }

    /// Lattice coordinates of a world point in each face's frame (inverse
    /// of `FaceGrid::position`).
    fn lattice_coords(face: &Face, p: DVec3, s: f64) -> (f64, f64, f64) {
        match face {
            Face::Front => (p.x + s, p.y + s, p.z),
            Face::Back => (p.x + s, p.y + s, p.z + 2.0 * s),
            Face::Top => (p.x + s, p.z + s, p.y),
            Face::Bottom => (p.x + s, p.z + s, p.y + 2.0 * s),
            Face::Right => (p.z + s, p.y + s, p.x),
            Face::Left => (p.z + s, p.y + s, p.x + 2.0 * s),
        }
    }

    /// The mesh-exactly-once invariant end to end: over a sweep of sphere
    /// directions, each world point is owned by exactly one of the six
    /// faces — addressed through every face's own lattice.
    #[test]
    fn every_direction_has_exactly_one_owner() {
        let scale = 256.0f64;
        let grids: Vec<(Face, FaceGrid)> = [
            Face::Front,
            Face::Back,
            Face::Top,
            Face::Bottom,
            Face::Left,
            Face::Right,
        ]
        .iter()
        .map(|f| (f.clone(), FaceGrid::new(f, scale as f32)))
        .collect();

        let golden = std::f64::consts::TAU / (1.0 + 5.0f64.sqrt());
        for i in 0..500 {
            let z = 1.0 - 2.0 * (i as f64 + 0.5) / 500.0;
            let r = (1.0 - z * z).sqrt();
            let phi = i as f64 * golden;
            let p = DVec3::new(r * phi.cos(), r * phi.sin(), z) * scale;
            let owners = grids
                .iter()
                .filter(|(face, grid)| {
                    let (gx, gy, gz) = lattice_coords(&face, p, scale);
                    face_owns_cell(grid, &face, gx, gy, gz)
                })
                .count();
            assert_eq!(owners, 1, "point {p:?} has {owners} owners");
        }
    }
}
