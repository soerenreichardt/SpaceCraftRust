use bevy::input::Input;
use bevy::input::mouse::MouseMotion;
use bevy::math::{EulerRot, Quat, Vec3};
use bevy::prelude::{Component, EventReader, KeyCode, MouseButton, Mut, Query, Res, Transform, Window, With};
use bevy::window::{CursorGrabMode, PrimaryWindow};

#[derive(Component)]
pub struct MainCamera;

pub(crate) fn update_camera_position(
    mut query: Query<(&mut Transform, &mut MainCamera)>,
    keys: Res<Input<KeyCode>>,
    mouse_buttons: Res<Input<MouseButton>>,
    mut motion_events: EventReader<MouseMotion>,
    mut windows: Query<&mut Window, With<PrimaryWindow>>,
) {
    let (mut transform, mut camera) = query.single_mut();
    if handle_mouse_click(mouse_buttons, &mut windows) {
        handle_rotation(&mut motion_events, &mut transform);
    } else {
        motion_events.clear();
    }

    let translation = handle_movement(keys, &transform);

    transform.translation += translation * 10.0;
}

fn handle_rotation(motion_events: &mut EventReader<MouseMotion>, transform: &mut Mut<Transform>) {
    let (mut yaw, mut pitch, _) = transform.rotation.to_euler(EulerRot::YXZ);
    for motion_event in motion_events.read() {
        let delta = motion_event.delta;
        pitch -= (delta.y * 0.1).to_radians();
        yaw -= (delta.x * 0.1).to_radians();
    }

    transform.rotation =
        Quat::from_axis_angle(Vec3::Y, yaw) * Quat::from_axis_angle(Vec3::X, pitch);
}

fn handle_mouse_click(mouse_buttons: Res<Input<MouseButton>>, windows: &mut Query<&mut Window, With<PrimaryWindow>>) -> bool {
    let mut primary_window = windows.single_mut();
    if mouse_buttons.just_pressed(MouseButton::Left) {
        primary_window.cursor.grab_mode = CursorGrabMode::Locked;
        primary_window.cursor.visible = false;
        return true;
    }
    if mouse_buttons.pressed(MouseButton::Left) {
        return true;
    }
    if mouse_buttons.just_released(MouseButton::Left) {
        primary_window.cursor.grab_mode = CursorGrabMode::None;
        primary_window.cursor.visible = true;
    }
    false
}

fn handle_movement(keys: Res<Input<KeyCode>>, transform: &Mut<Transform>) -> Vec3 {
    let forward = transform.forward();
    let up = transform.up();
    let left = Vec3::cross(forward, up);

    let mut translation = Vec3::default();
    if keys.pressed(KeyCode::W) {
        translation += forward;
    }
    if keys.pressed(KeyCode::S) {
        translation -= forward;
    }
    if keys.pressed(KeyCode::A) {
        translation -= left;
    }
    if keys.pressed(KeyCode::D) {
        translation += left;
    }
    if keys.pressed(KeyCode::Space) {
        translation += up;
    }
    if keys.pressed(KeyCode::C) {
        translation -= up;
    }
    translation
}
