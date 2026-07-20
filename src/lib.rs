use bevy::input::mouse::{AccumulatedMouseMotion, AccumulatedMouseScroll};
use bevy::prelude::*;
use bevy_embedded_assets::EmbeddedAssetPlugin;

/// Android entry point. cargo-apk builds the cdylib and NativeActivity
/// calls into this via the #[bevy_main] generated android_main.
#[bevy_main]
fn main() {
    run_game();
}

/// Marker for the player-controlled sphere.
#[derive(Component)]
struct Player;

/// Orbit-camera state: angles around the player and zoom distance.
#[derive(Resource)]
struct OrbitCamera {
    yaw: f32,
    pitch: f32,
    distance: f32,
}

impl Default for OrbitCamera {
    fn default() -> Self {
        Self {
            yaw: 0.0,
            pitch: 0.6,
            distance: 12.0,
        }
    }
}

const PLAYER_RADIUS: f32 = 0.5;
const MOVE_SPEED: f32 = 6.0;
const GROUND_HALF_EXTENT: f32 = 24.0;

const PILLARS: [(f32, f32); 5] = [(6.0, 6.0), (-6.0, 6.0), (6.0, -6.0), (-6.0, -6.0), (0.0, 10.0)];
const PILLAR_HALF: f32 = 0.3;
const PILLAR_TOP: f32 = 2.0;

/// Knock sound played when the sphere bumps into a pillar or arena wall.
#[derive(Resource)]
struct CollisionSound(Handle<AudioSource>);

pub fn run_game() {
    App::new()
        .insert_resource(ClearColor(Color::srgb(0.04, 0.05, 0.09)))
        // EmbeddedAssetPlugin must be added BEFORE DefaultPlugins so the
        // embedded asset source is registered before the AssetServer starts.
        .add_plugins(EmbeddedAssetPlugin::default())
        .add_plugins(DefaultPlugins.set(WindowPlugin {
            primary_window: Some(Window {
                title: "Game Base".into(),
                // Browser build: render into the canvas provided by
                // web/index.html and track its CSS size. Ignored on native.
                canvas: Some("#game-canvas".into()),
                fit_canvas_to_parent: true,
                ..default()
            }),
            ..default()
        }))
        .init_resource::<OrbitCamera>()
        .add_systems(Startup, setup)
        .add_systems(
            Update,
            (move_player, collide_player, control_camera, sync_camera).chain(),
        )
        .run();
}

fn setup(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    commands.insert_resource(CollisionSound(asset_server.load("bounce.wav")));

    // Ground
    commands.spawn((
        Mesh3d(meshes.add(Plane3d::default().mesh().size(GROUND_HALF_EXTENT * 2.0, GROUND_HALF_EXTENT * 2.0))),
        MeshMaterial3d(materials.add(StandardMaterial {
            base_color: Color::srgb(0.16, 0.22, 0.18),
            perceptual_roughness: 0.95,
            ..default()
        })),
    ));

    // Player sphere
    commands.spawn((
        Player,
        Mesh3d(meshes.add(Sphere::new(PLAYER_RADIUS).mesh().uv(32, 18))),
        MeshMaterial3d(materials.add(StandardMaterial {
            base_color: Color::srgb(0.9, 0.35, 0.15),
            perceptual_roughness: 0.4,
            metallic: 0.1,
            ..default()
        })),
        Transform::from_xyz(0.0, PLAYER_RADIUS, 0.0),
    ));

    // A few reference pillars so motion is readable
    let pillar_mesh = meshes.add(Cuboid::new(0.6, 2.0, 0.6));
    let pillar_mat = materials.add(StandardMaterial {
        base_color: Color::srgb(0.35, 0.4, 0.55),
        ..default()
    });
    for (x, z) in PILLARS {
        commands.spawn((
            Mesh3d(pillar_mesh.clone()),
            MeshMaterial3d(pillar_mat.clone()),
            Transform::from_xyz(x, 1.0, z),
        ));
    }

    // Sun
    commands.spawn((
        DirectionalLight {
            illuminance: 8_000.0,
            shadows_enabled: true,
            ..default()
        },
        Transform::from_rotation(Quat::from_euler(EulerRot::YXZ, -0.8, -0.9, 0.0)),
    ));
    commands.insert_resource(AmbientLight {
        color: Color::WHITE,
        brightness: 120.0,
    });

    // Camera (positioned every frame by sync_camera)
    commands.spawn((
        Camera3d::default(),
        Transform::from_xyz(0.0, 8.0, 12.0).looking_at(Vec3::ZERO, Vec3::Y),
    ));

    // Controls overlay
    commands.spawn((
        Text::new("WASD / arrows: move   Space / Ctrl: up & down   Mouse drag: orbit   Scroll: zoom"),
        TextFont {
            font_size: 16.0,
            ..default()
        },
        TextColor(Color::srgba(1.0, 1.0, 1.0, 0.8)),
        Node {
            position_type: PositionType::Absolute,
            left: Val::Px(12.0),
            bottom: Val::Px(12.0),
            ..default()
        },
    ));
}

/// WASD/arrow movement on the ground plane, relative to the camera's yaw,
/// plus Space/Ctrl for vertical flight. The sphere rolls as it moves.
fn move_player(
    time: Res<Time>,
    keys: Res<ButtonInput<KeyCode>>,
    orbit: Res<OrbitCamera>,
    mut player: Query<&mut Transform, With<Player>>,
) {
    let Ok(mut transform) = player.get_single_mut() else {
        return;
    };

    let mut input = Vec3::ZERO;
    if keys.pressed(KeyCode::KeyW) || keys.pressed(KeyCode::ArrowUp) {
        input.z -= 1.0;
    }
    if keys.pressed(KeyCode::KeyS) || keys.pressed(KeyCode::ArrowDown) {
        input.z += 1.0;
    }
    if keys.pressed(KeyCode::KeyA) || keys.pressed(KeyCode::ArrowLeft) {
        input.x -= 1.0;
    }
    if keys.pressed(KeyCode::KeyD) || keys.pressed(KeyCode::ArrowRight) {
        input.x += 1.0;
    }
    if keys.pressed(KeyCode::Space) {
        input.y += 1.0;
    }
    if keys.pressed(KeyCode::ControlLeft) || keys.pressed(KeyCode::ControlRight) {
        input.y -= 1.0;
    }

    if input == Vec3::ZERO {
        return;
    }

    // Rotate the horizontal input by the camera yaw so "forward" is always
    // away from the camera.
    let yaw_rotation = Quat::from_rotation_y(orbit.yaw);
    let horizontal = yaw_rotation * Vec3::new(input.x, 0.0, input.z);
    let velocity = (horizontal + Vec3::Y * input.y).normalize_or_zero() * MOVE_SPEED;
    let delta = velocity * time.delta_secs();

    transform.translation += delta;
    transform.translation.x = transform.translation.x.clamp(-GROUND_HALF_EXTENT, GROUND_HALF_EXTENT);
    transform.translation.z = transform.translation.z.clamp(-GROUND_HALF_EXTENT, GROUND_HALF_EXTENT);
    transform.translation.y = transform.translation.y.clamp(PLAYER_RADIUS, 20.0);

    // Roll the sphere around the axis perpendicular to its horizontal motion.
    let horizontal_delta = Vec3::new(delta.x, 0.0, delta.z);
    if horizontal_delta.length_squared() > 0.0 {
        let axis = Vec3::Y.cross(horizontal_delta.normalize());
        let angle = horizontal_delta.length() / PLAYER_RADIUS;
        transform.rotate(Quat::from_axis_angle(axis, angle));
    }
}

/// Keeps the sphere out of the pillars and plays a knock on new contact
/// with a pillar or an arena wall.
fn collide_player(
    mut commands: Commands,
    sound: Res<CollisionSound>,
    mut player: Query<&mut Transform, With<Player>>,
    mut was_hit: Local<bool>,
) {
    let Ok(mut transform) = player.get_single_mut() else {
        return;
    };
    let mut hit = false;

    // Pillars: circle vs square in the XZ plane, only below the pillar top.
    if transform.translation.y - PLAYER_RADIUS < PILLAR_TOP {
        for (px, pz) in PILLARS {
            let closest = Vec3::new(
                transform.translation.x.clamp(px - PILLAR_HALF, px + PILLAR_HALF),
                transform.translation.y,
                transform.translation.z.clamp(pz - PILLAR_HALF, pz + PILLAR_HALF),
            );
            let mut delta = transform.translation - closest;
            delta.y = 0.0;
            let dist = delta.length();
            if dist < PLAYER_RADIUS {
                let push = if dist > 1e-4 { delta / dist } else { Vec3::X };
                transform.translation += push * (PLAYER_RADIUS - dist);
                hit = true;
            }
        }
    }

    // Arena walls (move_player clamps position to these bounds).
    hit |= transform.translation.x.abs() >= GROUND_HALF_EXTENT - 1e-3
        || transform.translation.z.abs() >= GROUND_HALF_EXTENT - 1e-3;

    // Only knock on the rising edge, with position-derived pitch variation
    // so repeated bumps don't sound canned.
    if hit && !*was_hit {
        let variation =
            0.9 + ((transform.translation.x + transform.translation.z).abs() % 1.0) * 0.25;
        commands.spawn((
            AudioPlayer::new(sound.0.clone()),
            PlaybackSettings {
                speed: variation,
                ..PlaybackSettings::DESPAWN
            },
        ));
    }
    *was_hit = hit;
}

/// Mouse drag orbits the camera, scroll wheel zooms. A one-finger touch
/// drag also orbits, so the Android build is usable out of the box.
fn control_camera(
    buttons: Res<ButtonInput<MouseButton>>,
    mouse_motion: Res<AccumulatedMouseMotion>,
    mouse_scroll: Res<AccumulatedMouseScroll>,
    touches: Res<Touches>,
    mut orbit: ResMut<OrbitCamera>,
) {
    let mut drag = Vec2::ZERO;
    if buttons.pressed(MouseButton::Left) || buttons.pressed(MouseButton::Right) {
        drag += mouse_motion.delta;
    }
    for touch in touches.iter() {
        drag += touch.delta();
    }

    orbit.yaw -= drag.x * 0.005;
    orbit.pitch = (orbit.pitch + drag.y * 0.005).clamp(0.05, 1.5);
    orbit.distance = (orbit.distance - mouse_scroll.delta.y * 1.2).clamp(3.0, 40.0);
}

/// Places the camera on its orbit around the player every frame.
fn sync_camera(
    orbit: Res<OrbitCamera>,
    player: Query<&Transform, (With<Player>, Without<Camera3d>)>,
    mut camera: Query<&mut Transform, With<Camera3d>>,
) {
    let (Ok(player), Ok(mut camera)) = (player.get_single(), camera.get_single_mut()) else {
        return;
    };

    let rotation = Quat::from_rotation_y(orbit.yaw) * Quat::from_rotation_x(-orbit.pitch);
    let offset = rotation * Vec3::new(0.0, 0.0, orbit.distance);
    camera.translation = player.translation + offset;
    camera.look_at(player.translation, Vec3::Y);
}
