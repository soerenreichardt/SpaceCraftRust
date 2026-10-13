use crate::terrain::chunk::Chunk;
use crate::terrain::generate::TerrainGenerator;
use crate::terrain::mesh::base_plane::CurvedBasePlane;
use crate::terrain::mesh::marching_cubes_tables::{EDGE_TABLE, TRI_TABLE};
use crate::terrain::CHUNK_SIZE;
use bevy::math::DVec3;
use bevy::prelude::Mesh;
use bevy::render::mesh::{Indices, PrimitiveTopology};
use bevy::render::render_asset::RenderAssetUsages;

const ISO_LEVEL: f64 = 0.0;

// Corner offsets in the classic Lorensen–Cline numbering used by the tables:
// corners 0–3 on the z=0 plane, 4–7 on the z=1 plane.
const CORNERS: [(usize, usize, usize); 8] = [
    (0, 0, 0),
    (1, 0, 0),
    (1, 1, 0),
    (0, 1, 0),
    (0, 0, 1),
    (1, 0, 1),
    (1, 1, 1),
    (0, 1, 1),
];

const EDGE_CORNERS: [(usize, usize); 12] = [
    (0, 1),
    (1, 2),
    (2, 3),
    (3, 0),
    (4, 5),
    (5, 6),
    (6, 7),
    (7, 4),
    (0, 4),
    (1, 5),
    (2, 6),
    (3, 7),
];

pub fn generate_mesh(
    base_plane: &CurvedBasePlane,
    terrain_generator: &TerrainGenerator,
    chunk: &Chunk,
) -> Mesh {
    let step_size = (chunk.length / CHUNK_SIZE as f32) as f64;
    let noise_values = noise_values(base_plane, terrain_generator, step_size);

    let (axis_a, axis_b) = chunk.face.perpendicular_vectors();
    let flip = axis_a.cross(axis_b).dot(chunk.face.direction_vector()) > 0.0;

    let mut vertices: Vec<[f32; 3]> = Vec::new();
    let mut indices: Vec<u32> = Vec::new();

    for z in 0..CHUNK_SIZE {
        for y in 0..CHUNK_SIZE {
            for x in 0..CHUNK_SIZE {
                let mut cube_index = 0usize;
                for (i, (dx, dy, dz)) in CORNERS.iter().enumerate() {
                    if grid_value(&noise_values, x + dx, y + dy, z + dz) > ISO_LEVEL {
                        cube_index |= 1 << i;
                    }
                }

                if EDGE_TABLE[cube_index] == 0 {
                    continue;
                }

                // Interpolated crossing point per intersected edge, in edge order 0–11.
                let mut edge_vertices: [Option<DVec3>; 12] = [None; 12];
                for edge in 0..12 {
                    if EDGE_TABLE[cube_index] & (1 << edge) == 0 {
                        continue;
                    }
                    let (c1, c2) = EDGE_CORNERS[edge];
                    edge_vertices[edge] = Some(interpolate_edge(
                        base_plane,
                        &noise_values,
                        step_size,
                        x,
                        y,
                        z,
                        c1,
                        c2,
                    ));
                }

                for triangle in TRI_TABLE[cube_index].chunks_exact(3) {
                    if triangle[0] < 0 {
                        break;
                    }
                    let winding = if flip {
                        [triangle[2], triangle[1], triangle[0]]
                    } else {
                        [triangle[0], triangle[1], triangle[2]]
                    };
                    for edge in winding {
                        let vertex = edge_vertices[edge as usize].expect("Interpolated edge vertex missing");
                        let base = vertices.len() as u32;
                        vertices.push(vertex.as_vec3().into());
                        indices.push(base);
                    }
                }
            }
        }
    }

    let mut mesh = Mesh::new(PrimitiveTopology::TriangleList, RenderAssetUsages::all());
    mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, vertices);
    mesh.insert_indices(Indices::U32(indices));
    mesh.compute_normals();
    mesh
}

#[inline]
fn grid_value(noise_values: &[f64], x: usize, y: usize, z: usize) -> f64 {
    noise_values[x + y * (CHUNK_SIZE + 1) + z * (CHUNK_SIZE + 1) * (CHUNK_SIZE + 1)]
}

#[inline]
fn grid_position(base_plane: &CurvedBasePlane, step_size: f64, x: usize, y: usize, z: usize) -> DVec3 {
    let plane_vertex = base_plane.vertex_at(x, y);
    plane_vertex + plane_vertex.normalize() * (z as f64 * step_size)
}

#[inline]
fn interpolate_edge(
    base_plane: &CurvedBasePlane,
    noise_values: &[f64],
    step_size: f64,
    x: usize,
    y: usize,
    z: usize,
    c1: usize,
    c2: usize,
) -> DVec3 {
    let (dx1, dy1, dz1) = CORNERS[c1];
    let (dx2, dy2, dz2) = CORNERS[c2];
    let p1 = grid_position(base_plane, step_size, x + dx1, y + dy1, z + dz1);
    let p2 = grid_position(base_plane, step_size, x + dx2, y + dy2, z + dz2);
    let v1 = grid_value(noise_values, x + dx1, y + dy1, z + dz1);
    let v2 = grid_value(noise_values, x + dx2, y + dy2, z + dz2);

    let diff = v2 - v1;
    let t = if diff.abs() < f64::EPSILON {
        0.5
    } else {
        ((ISO_LEVEL - v1) / diff).clamp(0.0, 1.0)
    };
    p1.lerp(p2, t)
}

fn noise_values(base_plane: &CurvedBasePlane, terrain_generator: &TerrainGenerator, step_size: f64) -> Vec<f64> {
    let mut noise_values = Vec::with_capacity((CHUNK_SIZE + 1) * (CHUNK_SIZE + 1) * (CHUNK_SIZE + 1));

    for z in 0..(CHUNK_SIZE + 1) {
        for y in 0..(CHUNK_SIZE + 1) {
            for x in 0..(CHUNK_SIZE + 1) {
                let plane_vertex = base_plane.vertex_at(x, y);
                let depth = z as f64 * step_size;
                let offset = plane_vertex.normalize() * depth;
                let offset_vertex = plane_vertex + offset;
                let noise_value = terrain_generator.sample(offset_vertex);
                noise_values.push(noise_value);
            }
        }
    }

    noise_values
}
