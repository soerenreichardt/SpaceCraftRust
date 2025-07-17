use bevy::pbr::wireframe::Wireframe;
use crate::terrain::chunk::Chunk;
use crate::terrain::Face;
use bevy::prelude::*;
use bevy::render::mesh::{Indices, PrimitiveTopology};
use bevy::render::render_asset::RenderAssetUsages;
use rand::random;
use crate::terrain::terrain_octree::RemoveTerrainChildren;

pub(crate) const MESH_SIZE: usize = 16;
const QUEUE_CAPACITY: usize = 10000;

pub fn generate_chunk_meshes(
    mut commands: Commands,
    query: Query<(Entity, &Chunk), Added<Chunk>>,
    mut parents_query: Query<&mut Visibility>,
    // mesh_generator: Res<MeshGenerator>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>
) {
    for (entity, chunk) in query.iter() {
        // let pbr_bundle = compute_mesh(chunk.center, chunk.length, chunk.face.clone(), chunk.scale, mesh_generator.indices.clone(), &mut meshes, &mut materials);
        // commands.entity(entity).insert(pbr_bundle);

        let length = chunk.length * chunk.scale;
        let cube_mesh = meshes.add(Cuboid::from_length(length));
        // let sphere_mesh = meshes.add(Sphere::new(10.0));
        let material = materials.add(StandardMaterial {
            base_color: Color::linear_rgb(random::<f32>(), random::<f32>(), random::<f32>()),
            ..default()
        });

        let mut entity_commands = commands.entity(entity);
        entity_commands.insert((
            Mesh3d(cube_mesh),
            MeshMaterial3d(material.clone()),
            Visibility::Visible
        ));

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
    indices: Vec<u32>
}

impl Default for MeshGenerator {
    fn default() -> Self {
        let indices = std::str::from_utf8(include_bytes!("../../resources/indices")).unwrap()
            .split(',')
            .map(|s| s.trim().parse().unwrap())
            .collect();
        MeshGenerator {
            indices
        }
    }
}

pub(crate) fn compute_mesh(
    center: Vec3,
    length: f32,
    face: Face,
    scale: f32,
    indices: Vec<u32>,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
) -> impl Bundle {
    let mut mesh = Mesh::new(PrimitiveTopology::TriangleList, RenderAssetUsages::all());
    let step_size = length / MESH_SIZE as f32;
    let offset = length / 2.0;
    let (axis_a, axis_b) = face.perpendicular_vectors();
    let (offset_a, offset_b) = (axis_a * offset, axis_b * offset);

    let mut vertices: Vec<[f32; 3]> = Vec::with_capacity((MESH_SIZE + 1) * (MESH_SIZE + 1));
    for y in 0..MESH_SIZE + 1 {
        for x in 0..MESH_SIZE + 1 {
            let vertex = axis_a * (x as f32 * step_size) + axis_b * (y as f32 * step_size) - offset_a - offset_b + center;
            let vertex = vertex.normalize() * scale;
            vertices.push(vertex.clone().into());
        }
    }

    mesh.insert_indices(Indices::U32(indices));
    mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, vertices);
    mesh.duplicate_vertices();
    mesh.compute_flat_normals();

    (
        Mesh3d(meshes.add(mesh)),
        MeshMaterial3d(materials.add(StandardMaterial {
            base_color: Color::linear_rgb(random::<f32>(), random::<f32>(), random::<f32>()),
            ..default()
        })),
        Visibility::Visible
    )
}

#[inline(always)]
fn transform_vertex_for_noise(vertex: Vec3, scale: f32) -> [f64; 3] {
    [
        vertex.x as f64 / scale as f64,
        vertex.y as f64 / scale as f64,
        vertex.z as f64 / scale as f64
    ]
}