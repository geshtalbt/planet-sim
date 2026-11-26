use bevy::{
    asset::RenderAssetUsages,
    input::mouse::{AccumulatedMouseMotion, AccumulatedMouseScroll},
    mesh::Indices,
    prelude::*,
    render::render_resource::PrimitiveTopology,
    window::PrimaryWindow,
};
use bevy_egui::EguiContexts;

use crate::app::{Screen, SimulationRes, SimulationSet, ViewState};
use crate::planet_generator::noise::SplitMix64;
use crate::rendering::MapLayer;
use crate::rendering::mesh::{build_planet_mesh, build_rivers_mesh};

#[derive(Resource)]
pub struct OrbitCameraState {
    pub yaw: f32,
    pub target_yaw: f32,
    pub pitch: f32,
    pub target_pitch: f32,
    pub distance: f32,
    pub target_distance: f32,
    pub drag_accum: f32,
}

impl Default for OrbitCameraState {
    fn default() -> Self {
        Self {
            yaw: 0.35,
            target_yaw: 0.35,
            pitch: 0.22,
            target_pitch: 0.22,
            distance: 3.2,
            target_distance: 3.2,
            drag_accum: 0.0,
        }
    }
}

#[derive(Resource)]
pub struct OrbitalState {
    pub angle: f32,
    /// Visual orbital radius in planet-radius units.
    pub radius: f32,
    /// Real seconds per visual orbit. The simulation clock drives the climate;
    /// this is a deliberately readable visual orbit.
    pub period_seconds: f32,
}

impl Default for OrbitalState {
    fn default() -> Self {
        Self {
            angle: 0.4,
            radius: 4.5,
            period_seconds: 28.0,
        }
    }
}

#[derive(Component)]
struct SunLight;

#[derive(Component)]
struct NightFill;

#[derive(Component)]
struct StarBody;

#[derive(Component)]
struct OrbitRing;

#[derive(Resource, Default)]
pub struct PlanetVisual {
    pub planet_entity: Option<Entity>,
    pub mesh: Option<Handle<Mesh>>,
    pub rivers_entity: Option<Entity>,
    pub rivers_mesh: Option<Handle<Mesh>>,
    pub atmosphere_entity: Option<Entity>,
    pub marker_entity: Option<Entity>,
    pub star_entity: Option<Entity>,
    pub orbit_entity: Option<Entity>,
    pub last_revision: u64,
    pub last_active: usize,
    pub last_layer: Option<MapLayer>,
    pub last_exaggeration: f32,
    pub last_cell_count: usize,
    pub last_show_clouds: bool,
    pub force_rebuild: bool,
}

pub struct RenderingPlugin;

impl Plugin for RenderingPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(ClearColor(Color::srgb(0.003, 0.005, 0.012)))
            .insert_resource(OrbitCameraState::default())
            .insert_resource(OrbitalState::default())
            .init_resource::<PlanetVisual>()
            .add_systems(Startup, setup_scene)
            .add_systems(
                Update,
                (
                    update_orbital,
                    camera_control,
                    ensure_visual,
                    sync_visual,
                    update_sun_light,
                )
                    .chain()
                    .after(SimulationSet),
            );
    }
}

fn setup_scene(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    commands.spawn((
        Name::new("Orbit camera"),
        Camera3d::default(),
        Transform::from_xyz(0.0, 0.7, 2.8).looking_at(Vec3::ZERO, Vec3::Y),
    ));

    commands.insert_resource(GlobalAmbientLight {
        color: Color::srgb(0.45, 0.58, 0.82),
        brightness: 420.0,
        ..default()
    });
    commands.spawn((
        Name::new("Stellar light"),
        SunLight,
        DirectionalLight {
            color: Color::srgb(1.0, 0.94, 0.82),
            illuminance: 12_000.0,
            shadow_maps_enabled: false,
            ..default()
        },
        Transform::from_xyz(0.0, 0.0, 0.0).looking_at(Vec3::X, Vec3::Y),
    ));
    // A faint blue fill reveals the night hemisphere without pretending that
    // it is daylight. It keeps continents readable on the dark side.
    commands.spawn((
        Name::new("Night-side fill light"),
        NightFill,
        DirectionalLight {
            color: Color::srgb(0.30, 0.48, 0.85),
            illuminance: 4_000.0,
            shadow_maps_enabled: false,
            ..default()
        },
        Transform::from_xyz(0.0, 0.0, 0.0).looking_at(-Vec3::X, Vec3::Y),
    ));

    let star_entity = commands
        .spawn((
            Name::new("Host star"),
            StarBody,
            Mesh3d(meshes.add(Sphere::new(0.30).mesh().ico(4).unwrap())),
            MeshMaterial3d(materials.add(StandardMaterial {
                base_color: Color::srgb(1.0, 0.82, 0.34),
                emissive: LinearRgba::rgb(8.0, 4.2, 0.7),
                unlit: true,
                ..default()
            })),
            Transform::default(),
            Visibility::Hidden,
        ))
        .id();

    let mut orbit_positions = Vec::new();
    let mut orbit_colors = Vec::new();
    for i in 0..=192 {
        let a = i as f32 / 192.0 * std::f32::consts::TAU;
        orbit_positions.push([4.5 * a.cos(), 0.0, 4.5 * a.sin()]);
        orbit_colors.push([0.22, 0.56, 0.95, 0.50]);
    }
    let mut orbit_mesh = Mesh::new(PrimitiveTopology::LineList, RenderAssetUsages::default());
    // Convert the closed polyline into line-list pairs.
    let mut orbit_indices = Vec::new();
    for i in 0..192u32 {
        orbit_indices.push(i);
        orbit_indices.push(i + 1);
    }
    orbit_mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, orbit_positions);
    orbit_mesh.insert_attribute(Mesh::ATTRIBUTE_COLOR, orbit_colors);
    orbit_mesh.insert_indices(Indices::U32(orbit_indices));
    let orbit_entity = commands
        .spawn((
            Name::new("Orbit path"),
            OrbitRing,
            Mesh3d(meshes.add(orbit_mesh)),
            MeshMaterial3d(materials.add(StandardMaterial {
                base_color: Color::srgba(0.22, 0.56, 0.95, 0.50),
                alpha_mode: AlphaMode::Blend,
                unlit: true,
                cull_mode: None,
                ..default()
            })),
            Transform::default(),
            Visibility::Hidden,
        ))
        .id();

    // A point light at the star guarantees the correct illuminated hemisphere
    // independently of directional-light sign conventions.
    commands.spawn((
        Name::new("Stellar point light"),
        PointLight {
            color: Color::srgb(1.0, 0.92, 0.76),
            intensity: 4_000_000.0,
            range: 80.0,
            shadow_maps_enabled: false,
            ..default()
        },
        Transform::default(),
    ));

    // Star field (visual only; not part of the simulation).
    let star_mesh = meshes.add(Sphere::new(1.0).mesh().ico(1).unwrap());
    let star_material = materials.add(StandardMaterial {
        base_color: Color::srgb(0.78, 0.86, 1.0),
        unlit: true,
        ..default()
    });
    let mut rng = SplitMix64::new(0x57A2_11C0_FFEE_1234);
    for _ in 0..320 {
        let dir = rng.unit_vec();
        let distance = rng.range(28.0, 55.0);
        let scale = rng.range(0.012, 0.045);
        commands.spawn((
            Mesh3d(star_mesh.clone()),
            MeshMaterial3d(star_material.clone()),
            Transform::from_translation(dir * distance).with_scale(Vec3::splat(scale)),
        ));
    }

    let atmosphere = commands
        .spawn((
            Name::new("Atmosphere shell"),
            Mesh3d(meshes.add(Sphere::new(1.0).mesh().ico(5).unwrap())),
            MeshMaterial3d(materials.add(StandardMaterial {
                base_color: Color::srgba(0.18, 0.40, 0.92, 0.065),
                alpha_mode: AlphaMode::Blend,
                unlit: true,
                cull_mode: None,
                double_sided: true,
                ..default()
            })),
            Transform::from_scale(Vec3::splat(1.05)),
            Visibility::Hidden,
        ))
        .id();

    let marker = commands
        .spawn((
            Name::new("Selected region marker"),
            Mesh3d(meshes.add(Sphere::new(1.0).mesh().ico(2).unwrap())),
            MeshMaterial3d(materials.add(StandardMaterial {
                base_color: Color::srgb(1.0, 0.55, 0.12),
                emissive: LinearRgba::rgb(3.5, 0.8, 0.05),
                unlit: true,
                ..default()
            })),
            Transform::from_scale(Vec3::splat(0.015)),
            Visibility::Hidden,
        ))
        .id();

    commands.insert_resource(PlanetVisual {
        atmosphere_entity: Some(atmosphere),
        marker_entity: Some(marker),
        star_entity: Some(star_entity),
        orbit_entity: Some(orbit_entity),
        ..default()
    });
}

fn orbit_position(orbital: &OrbitalState) -> Vec3 {
    Vec3::new(
        orbital.angle.cos() * orbital.radius,
        0.0,
        orbital.angle.sin() * orbital.radius,
    )
}

fn update_orbital(
    time: Res<Time>,
    simulation: Res<SimulationRes>,
    view: Res<ViewState>,
    mut orbital: ResMut<OrbitalState>,
) {
    if view.screen != Screen::Simulation {
        return;
    }
    let Some(sim) = simulation.0.as_ref() else {
        return;
    };
    if sim.time.paused || time.delta_secs() <= 0.0 {
        return;
    }
    orbital.angle = (orbital.angle
        + time.delta_secs() * std::f32::consts::TAU / orbital.period_seconds)
        .rem_euclid(std::f32::consts::TAU);
}

fn update_sun_light(
    simulation: Res<SimulationRes>,
    view: Res<ViewState>,
    orbital: Res<OrbitalState>,
    mut sun: Single<&mut Transform, (With<SunLight>, Without<NightFill>)>,
    mut fill: Single<&mut Transform, (With<NightFill>, Without<SunLight>)>,
) {
    if view.screen != Screen::Simulation || simulation.0.is_none() {
        return;
    }
    let target = orbit_position(&orbital);
    if target.length_squared() > 1.0e-4 {
        **sun = Transform::from_translation(Vec3::ZERO).looking_at(target, Vec3::Y);
        **fill = Transform::from_translation(-target).looking_at(target, Vec3::Y);
    }
}

fn ensure_visual(
    mut commands: Commands,
    simulation: Res<SimulationRes>,
    mut visual: ResMut<PlanetVisual>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    let Some(sim) = simulation.0.as_ref() else {
        hide_visual(&mut commands, &visual);
        return;
    };
    if visual.planet_entity.is_some() {
        return;
    }
    let planet = sim.active_planet();
    let mesh = meshes.add(build_planet_mesh(planet, MapLayer::Natural, 22.0, true));
    let material = materials.add(StandardMaterial {
        base_color: Color::WHITE,
        perceptual_roughness: 0.92,
        metallic: 0.0,
        reflectance: 0.05,
        cull_mode: None,
        ..default()
    });
    let entity = commands
        .spawn((
            Name::new("Planet surface"),
            Mesh3d(mesh.clone()),
            MeshMaterial3d(material),
            Transform::default(),
        ))
        .id();

    let river_mesh = meshes.add(build_rivers_mesh(planet, 22.0));
    let river_material = materials.add(StandardMaterial {
        base_color: Color::srgb(0.25, 0.75, 1.0),
        unlit: true,
        alpha_mode: AlphaMode::Add,
        ..default()
    });
    let rivers_entity = commands
        .spawn((
            Name::new("Rivers"),
            Mesh3d(river_mesh.clone()),
            MeshMaterial3d(river_material),
            Transform::default(),
        ))
        .id();

    visual.planet_entity = Some(entity);
    visual.mesh = Some(mesh);
    visual.rivers_entity = Some(rivers_entity);
    visual.rivers_mesh = Some(river_mesh);
}

fn sync_visual(
    mut commands: Commands,
    simulation: Res<SimulationRes>,
    view: Res<ViewState>,
    orbital: Res<OrbitalState>,
    mut visual: ResMut<PlanetVisual>,
    mut meshes: ResMut<Assets<Mesh>>,
) {
    let Some(sim) = simulation.0.as_ref() else {
        hide_visual(&mut commands, &visual);
        return;
    };
    if view.screen != Screen::Simulation {
        hide_visual(&mut commands, &visual);
        return;
    }
    let planet = sim.active_planet();
    let Some(planet_entity) = visual.planet_entity else {
        return;
    };
    let planet_position = orbit_position(&orbital);
    commands.entity(planet_entity).insert((
        Visibility::Visible,
        Transform::from_translation(planet_position),
    ));
    if let Some(entity) = visual.rivers_entity {
        commands
            .entity(entity)
            .insert(Transform::from_translation(planet_position));
    }
    if let Some(entity) = visual.star_entity {
        commands.entity(entity).insert(Visibility::Visible);
    }
    if let Some(entity) = visual.orbit_entity {
        commands.entity(entity).insert(Visibility::Visible);
    }

    let cell_count_changed = visual.last_cell_count != planet.grid.cell_count();
    let rebuild = visual.force_rebuild
        || cell_count_changed
        || visual.last_revision != planet.revision
        || visual.last_active != sim.active
        || visual.last_layer != Some(view.layer)
        || visual.last_show_clouds != view.show_clouds
        || (visual.last_exaggeration - view.elevation_exaggeration).abs() > 0.01;

    if rebuild {
        if let Some(handle) = visual.mesh.clone()
            && let Some(mut mesh) = meshes.get_mut(&handle)
        {
            *mesh = build_planet_mesh(
                planet,
                view.layer,
                view.elevation_exaggeration,
                view.show_clouds,
            );
        }
        if let Some(handle) = visual.rivers_mesh.clone()
            && let Some(mut mesh) = meshes.get_mut(&handle)
        {
            *mesh = build_rivers_mesh(planet, view.elevation_exaggeration);
        }
        visual.last_revision = planet.revision;
        visual.last_active = sim.active;
        visual.last_layer = Some(view.layer);
        visual.last_exaggeration = view.elevation_exaggeration;
        visual.last_cell_count = planet.grid.cell_count();
        visual.last_show_clouds = view.show_clouds;
        visual.force_rebuild = false;
    }

    if let Some(entity) = visual.rivers_entity {
        let show = view.show_rivers
            && matches!(
                view.layer,
                MapLayer::Natural | MapLayer::Water | MapLayer::Elevation
            );
        commands.entity(entity).insert(if show {
            Visibility::Visible
        } else {
            Visibility::Hidden
        });
    }
    if let Some(entity) = visual.atmosphere_entity {
        let scale = 1.035 + view.elevation_exaggeration * 0.0006;
        commands.entity(entity).insert((
            if view.show_atmosphere {
                Visibility::Visible
            } else {
                Visibility::Hidden
            },
            Transform::from_translation(planet_position).with_scale(Vec3::splat(scale)),
        ));
    }

    if let Some(marker) = visual.marker_entity {
        if let Some(cell) = view.selected_cell.filter(|c| *c < planet.grid.cell_count()) {
            let radius =
                crate::rendering::surface_radius(planet, cell, view.elevation_exaggeration) * 1.012;
            let pos = planet_position + planet.grid.direction(cell) * radius;
            commands.entity(marker).insert((
                Transform::from_translation(pos).with_scale(Vec3::splat(0.013)),
                Visibility::Visible,
            ));
        } else {
            commands.entity(marker).insert(Visibility::Hidden);
        }
    }
}

fn hide_visual(commands: &mut Commands, visual: &PlanetVisual) {
    for entity in [
        visual.planet_entity,
        visual.rivers_entity,
        visual.atmosphere_entity,
        visual.marker_entity,
        visual.star_entity,
        visual.orbit_entity,
    ]
    .into_iter()
    .flatten()
    {
        commands.entity(entity).insert(Visibility::Hidden);
    }
}

fn camera_control(
    mut camera: Single<(&Camera, &mut Transform), With<Camera3d>>,
    mut orbit: ResMut<OrbitCameraState>,
    motion: Res<AccumulatedMouseMotion>,
    scroll: Res<AccumulatedMouseScroll>,
    buttons: Res<ButtonInput<MouseButton>>,
    window: Single<&Window, With<PrimaryWindow>>,
    mut view: ResMut<ViewState>,
    simulation: Res<SimulationRes>,
    orbital: Res<OrbitalState>,
    mut contexts: EguiContexts,
    time: Res<Time>,
) -> Result {
    let ctx = contexts.ctx_mut()?;
    let pointer_over_ui = ctx.egui_wants_pointer_input();
    let (camera, transform) = &mut *camera;
    let delta = motion.delta;
    if buttons.just_pressed(MouseButton::Left) {
        orbit.drag_accum = 0.0;
    }
    if buttons.pressed(MouseButton::Left) && !pointer_over_ui {
        if delta != Vec2::ZERO {
            orbit.target_yaw -= delta.x * 0.006;
            orbit.target_pitch = (orbit.target_pitch + delta.y * 0.005).clamp(-1.35, 1.35);
            orbit.drag_accum += delta.x.abs() + delta.y.abs();
        }
    }
    if !pointer_over_ui && scroll.delta.y != 0.0 {
        orbit.target_distance =
            (orbit.target_distance * (-scroll.delta.y * 0.12).exp()).clamp(1.35, 9.0);
    }

    let smooth = 1.0 - (-time.delta_secs() * 12.0).exp();
    orbit.yaw += (orbit.target_yaw - orbit.yaw) * smooth;
    orbit.pitch += (orbit.target_pitch - orbit.pitch) * smooth;
    orbit.distance += (orbit.target_distance - orbit.distance) * smooth;
    let cos_pitch = orbit.pitch.cos();
    let target = if simulation.0.is_some() {
        orbit_position(&orbital)
    } else {
        Vec3::ZERO
    };
    let offset = Vec3::new(
        orbit.yaw.sin() * cos_pitch,
        orbit.pitch.sin(),
        orbit.yaw.cos() * cos_pitch,
    ) * orbit.distance;
    **transform = Transform::from_translation(target + offset).looking_at(target, Vec3::Y);

    if buttons.just_released(MouseButton::Left)
        && !pointer_over_ui
        && orbit.drag_accum < 6.0
        && let Some(cursor) = window.cursor_position()
    {
        let global = GlobalTransform::from(**transform);
        if let Ok(ray) = camera.viewport_to_world(&global, cursor) {
            let origin = ray.origin;
            let direction = ray.direction;
            let to_center = origin - target;
            let b = to_center.dot(*direction);
            let c = to_center.length_squared() - 1.0;
            let disc = b * b - c;
            if disc > 0.0 {
                let t = -b - disc.sqrt();
                if t > 0.0 {
                    let point = origin + direction * t;
                    if let Some(sim) = simulation.0.as_ref() {
                        let cell = sim
                            .active_planet()
                            .grid
                            .nearest_cell((point - target).normalize());
                        view.selected_cell = Some(cell);
                    }
                }
            }
        }
    }
    Ok(())
}
