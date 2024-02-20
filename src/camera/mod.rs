use bevy::input::Input;
use bevy::math::Vec3;
use bevy::prelude::{Component, KeyCode, Query, Res, Transform, With};

#[derive(Component)]
pub struct MainCamera;

pub(crate) fn update_camera_position(
    mut query: Query<&mut Transform, With<MainCamera>>,
    keys: Res<Input<KeyCode>>
) {
    let mut transform = query.single_mut();
    let mut translation = Vec3::default();
    if keys.pressed(KeyCode::W) {
        translation += Vec3::new(0.0, 0.0, -0.1);
    }
    if keys.pressed(KeyCode::S) {
        translation += Vec3::new(0.0, 0.0, 0.1);
    }
    if keys.pressed(KeyCode::A) {
        translation += Vec3::new(-0.1, 0.0, 0.0);
    }
    if keys.pressed(KeyCode::D) {
        translation += Vec3::new(0.1, 0.0, 0.0);
    }
    if keys.pressed(KeyCode::Space) {
        translation += Vec3::new(0.0, 0.1, 0.0);
    }
    if keys.pressed(KeyCode::C) {
        translation += Vec3::new(0.0, -0.1, 0.0);
    }

    transform.translation += translation * 10.0;
}
