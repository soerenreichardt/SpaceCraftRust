use bevy::math::DVec3;
use bevy::prelude::Vec3;
use noiz::layering::LayeredNoise;
use noiz::{Noise, Sampleable};
use noiz::prelude::common_noise::{Fbm, Simplex};
use noiz::prelude::{FractalLayers, Normed, Persistence, SeedableNoise, ScalableNoise};

type TerrainFbm = Fbm<Simplex>;

/// Base-frequency knob: the coarsest octave places roughly this many
/// features per planet radius; each octave doubles the count — continents
/// down to hills and roughness.
const FEATURES_PER_RADIUS: f32 = 4.0;

/// Terrain amplitude as a fraction of the planet radius: the peak
/// displacement of mountains above / valleys below sea level.
const AMPLITUDE_FRACTION: f32 = 0.1;


pub(crate) struct TerrainGenerator {
    noise: Noise<TerrainFbm>,
    sea_level: f64,
    amplitude: f64
}

impl TerrainGenerator {
    pub(crate) fn new(scale: f32, seed: u32) -> Self {
        Self::with_amplitude(scale, seed, AMPLITUDE_FRACTION)
    }

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

    pub(crate) fn sample(&self, position: DVec3) -> f64 {
        let fbm =
            self.noise.sample_for::<f32>(Vec3::new(position.x as f32, position.y as f32, position.z as f32)) as f64;
        self.amplitude * fbm - (position.length() - self.sea_level)
    }
}