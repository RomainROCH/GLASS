//! Integration coverage for the display-free GLASS lifecycle.

use glass_overlay::layout::Widget;
use glass_overlay::modules::ModuleRegistry;
use glass_overlay::{
    Anchor, ClockModule, Color, FpsCounterModule, LayoutManager, ModuleInfo, ModulesConfig, NodeId,
    OverlayConfig, OverlayModule, Scene, SceneNode, SystemStatsModule, TextProps, WidgetWrapper,
};
use std::time::Duration;

#[derive(Debug)]
struct TrackingModule {
    id: &'static str,
    label: &'static str,
    enabled: bool,
    node_id: Option<NodeId>,
    position: (f32, f32),
    size: (f32, f32),
    updates: u32,
}

impl TrackingModule {
    fn new(id: &'static str, label: &'static str, width: f32, height: f32) -> Self {
        Self {
            id,
            label,
            enabled: true,
            node_id: None,
            position: (0.0, 0.0),
            size: (width, height),
            updates: 0,
        }
    }

    fn render_text(&self) -> String {
        format!(
            "{}#{}@{:.0},{:.0}",
            self.label, self.updates, self.position.0, self.position.1
        )
    }
}

impl OverlayModule for TrackingModule {
    fn info(&self) -> ModuleInfo {
        ModuleInfo {
            id: self.id,
            name: self.label,
            description: "Integration-test tracking module",
        }
    }

    fn init(&mut self, scene: &mut Scene) {
        let id = scene.add_text(TextProps {
            x: self.position.0,
            y: self.position.1,
            text: self.render_text(),
            font_size: 14.0,
            color: Color::WHITE,
        });
        self.node_id = Some(id);
    }

    fn update(&mut self, scene: &mut Scene, _dt: Duration) -> bool {
        self.updates += 1;
        if let Some(id) = self.node_id {
            scene.update(
                id,
                SceneNode::Text(TextProps {
                    x: self.position.0,
                    y: self.position.1,
                    text: self.render_text(),
                    font_size: 14.0,
                    color: Color::WHITE,
                }),
            )
        } else {
            false
        }
    }

    fn deinit(&mut self, scene: &mut Scene) {
        if let Some(id) = self.node_id.take() {
            scene.remove(id);
        }
    }

    fn enabled(&self) -> bool {
        self.enabled
    }

    fn set_enabled(&mut self, enabled: bool) {
        self.enabled = enabled;
    }

    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }

    fn set_position(&mut self, x: f32, y: f32) {
        self.position = (x, y);
    }

    fn content_size(&self) -> (f32, f32) {
        self.size
    }
}

fn text_nodes(scene: &Scene) -> Vec<(NodeId, TextProps)> {
    let mut nodes: Vec<_> = scene
        .iter()
        .filter_map(|(id, node)| match node {
            SceneNode::Text(props) => Some((id, props.clone())),
            SceneNode::Rect(_) => None,
        })
        .collect();
    nodes.sort_by_key(|(id, _)| id.0);
    nodes
}

fn text_node(scene: &Scene, id: NodeId) -> TextProps {
    text_nodes(scene)
        .into_iter()
        .find_map(|(node_id, props)| (node_id == id).then_some(props))
        .unwrap_or_else(|| panic!("missing text node {id}"))
}

fn text_node_by_prefix(scene: &Scene, prefix: &str) -> (NodeId, TextProps) {
    text_nodes(scene)
        .into_iter()
        .find(|(_, props)| props.text.starts_with(prefix))
        .unwrap_or_else(|| panic!("missing text node with prefix {prefix:?}"))
}

fn unique_ids(scene: &Scene) -> Vec<NodeId> {
    let ids: Vec<_> = scene.iter().map(|(id, _)| id).collect();
    let mut deduped = ids.clone();
    deduped.sort_by_key(|id| id.0);
    deduped.dedup();
    assert_eq!(deduped.len(), ids.len(), "scene node IDs must stay unique");
    ids
}

#[test]
fn overlay_config_default_round_trip_preserves_valid_values() {
    let config = OverlayConfig::default();

    let encoded = ron::to_string(&config).expect("default config should serialize");
    let decoded: OverlayConfig = ron::from_str(&encoded).expect("serialized config should parse");

    assert_eq!(decoded, config);
    assert!((0.0..=1.0).contains(&decoded.opacity));
    assert!(decoded.size.width > 0.0);
    assert!(decoded.size.height > 0.0);
}

#[test]
fn overlay_config_deserializes_module_and_layout_settings() {
    let config: OverlayConfig = ron::from_str(
        r#"
        (
            opacity: 0.42,
            modules: (
                clock_enabled: false,
                clock_format: "%I:%M %p",
                system_stats_enabled: true,
                stats_interval_ms: 250,
                fps_enabled: false,
            ),
            layout: (
                clock: (anchor: TopRight, margin_x: 16.0, margin_y: 12.0),
                system_stats: (
                    anchor: ScreenPercentage(0.25, 0.75),
                    margin_x: -4.0,
                    margin_y: 6.0,
                ),
                fps: (anchor: Center, margin_x: 0.0, margin_y: 20.0),
            ),
        )
        "#,
    )
    .expect("integration config should parse");

    assert_eq!(config.opacity, 0.42);
    assert!(!config.modules.clock_enabled);
    assert_eq!(config.modules.clock_format, "%I:%M %p");
    assert_eq!(config.modules.stats_interval_ms, 250);
    assert!(!config.modules.fps_enabled);
    assert_eq!(config.layout.clock.anchor, Anchor::TopRight);
    assert_eq!(config.layout.clock.margin_x, 16.0);
    assert_eq!(config.layout.clock.margin_y, 12.0);
    assert_eq!(
        config.layout.system_stats.anchor,
        Anchor::ScreenPercentage(0.25, 0.75)
    );
}

#[test]
fn scene_graph_add_update_remove_cycle_keeps_expected_nodes() {
    let mut scene = Scene::new();
    let ids: Vec<_> = (0..10)
        .map(|index| {
            scene.add_text(TextProps {
                x: index as f32 * 10.0,
                y: index as f32 * 5.0,
                text: format!("node-{index}"),
                font_size: 14.0,
                color: Color::WHITE,
            })
        })
        .collect();

    scene.clear_dirty();
    assert!(scene.update(
        ids[1],
        SceneNode::Text(TextProps {
            x: 10.0,
            y: 5.0,
            text: "updated-1".into(),
            font_size: 14.0,
            color: Color::WHITE,
        }),
    ));
    assert!(scene.update(
        ids[5],
        SceneNode::Text(TextProps {
            x: 50.0,
            y: 25.0,
            text: "updated-5".into(),
            font_size: 18.0,
            color: Color::WHITE,
        }),
    ));
    assert!(scene.update(
        ids[7],
        SceneNode::Text(TextProps {
            x: 70.0,
            y: 35.0,
            text: "updated-7".into(),
            font_size: 12.0,
            color: Color::WHITE,
        }),
    ));

    for id in ids.iter().take(5) {
        assert!(scene.remove(*id));
    }

    let remaining = text_nodes(&scene);
    assert_eq!(remaining.len(), 5);
    assert!(scene.is_dirty());
    assert_eq!(text_node(&scene, ids[5]).text, "updated-5");
    assert_eq!(text_node(&scene, ids[7]).text, "updated-7");
    assert!(
        scene.iter().all(|(id, _)| !ids[..5].contains(&id)),
        "removed nodes must not remain in the scene"
    );
}

#[test]
fn clock_module_lifecycle_updates_existing_node_then_deinits() {
    let mut scene = Scene::new();
    let mut clock = ClockModule::new("%S");
    clock.set_position(25.0, 40.0);

    clock.init(&mut scene);
    assert_eq!(scene.len(), 1);

    let (clock_id, initial) = text_nodes(&scene)
        .into_iter()
        .next()
        .expect("clock should add one node");
    assert_eq!(initial.x, 25.0);
    assert_eq!(initial.y, 40.0);
    assert_eq!(initial.text.len(), 2);

    std::thread::sleep(Duration::from_millis(1_100));
    assert!(clock.update(&mut scene, Duration::ZERO));

    let updated = text_node(&scene, clock_id);
    assert_eq!(updated.x, 25.0);
    assert_eq!(updated.y, 40.0);
    assert_eq!(updated.text.len(), 2);
    assert_ne!(updated.text, initial.text);

    clock.deinit(&mut scene);
    assert!(scene.is_empty());
}

#[test]
fn system_stats_temp_source_injection_refreshes_cpu_text() {
    let mut scene = Scene::new();
    let mut temps = vec![Some(71.0), Some(73.0)].into_iter();
    let mut stats = SystemStatsModule::new();
    stats.set_position(30.0, 45.0);
    stats.set_interval(Duration::ZERO);
    stats.set_temp_source(Box::new(move || temps.next().flatten()));

    stats.init(&mut scene);
    assert_eq!(scene.len(), 2);

    let (cpu_id, initial_cpu) = text_node_by_prefix(&scene, "system: CPU ");
    let (mem_id, initial_mem) = text_node_by_prefix(&scene, "system: RAM ");
    assert!(initial_cpu.text.contains("temp 71°C"));
    assert_eq!(initial_cpu.x, 30.0);
    assert_eq!(initial_mem.y, 45.0 + 14.0 * 1.3);

    assert!(stats.update(&mut scene, Duration::ZERO));

    let updated_cpu = text_node(&scene, cpu_id);
    let updated_mem = text_node(&scene, mem_id);
    assert!(updated_cpu.text.contains("temp 73°C"));
    assert!(updated_mem.text.starts_with("system: RAM "));
}

#[test]
fn fps_counter_records_frames_and_updates_display() {
    let mut scene = Scene::new();
    let mut fps = FpsCounterModule::new();
    fps.set_position(12.0, 34.0);

    fps.init(&mut scene);
    let (fps_id, initial) = text_nodes(&scene)
        .into_iter()
        .next()
        .expect("fps module should add one node");
    assert_eq!(initial.x, 12.0);
    assert!(initial.text.ends_with("--"));

    for _ in 0..6 {
        fps.record_frame();
        std::thread::sleep(Duration::from_millis(15));
    }

    std::thread::sleep(Duration::from_millis(550));
    assert!(fps.update(&mut scene, Duration::ZERO));

    let updated = text_node(&scene, fps_id);
    assert!(updated.text.starts_with("overlay-only FPS: "));
    assert_ne!(updated.text, initial.text);

    fps.deinit(&mut scene);
    assert!(scene.is_empty());
}

#[test]
fn module_registry_apply_config_toggles_modules_and_reapplies_clock_format() {
    let mut registry = ModuleRegistry::new();
    registry.register(Box::new(ClockModule::new("%H:%M:%S")));
    registry.register(Box::new(SystemStatsModule::new()));
    registry.register(Box::new(FpsCounterModule::new()));

    let mut scene = Scene::new();
    registry.init_all(&mut scene);
    assert_eq!(scene.len(), 4);

    let mut config = ModulesConfig {
        clock_enabled: false,
        clock_format: "%S".into(),
        system_stats_enabled: true,
        stats_interval_ms: 0,
        fps_enabled: true,
    };
    registry.apply_config(&config, &mut scene);
    assert_eq!(scene.len(), 3, "disabling the clock should remove its node");

    config.clock_enabled = true;
    registry.apply_config(&config, &mut scene);
    assert_eq!(
        scene.len(),
        4,
        "re-enabling the clock should recreate its node"
    );

    std::thread::sleep(Duration::from_millis(1_100));
    let _ = registry.update_all(&mut scene, Duration::ZERO);

    let texts = text_nodes(&scene);
    assert!(texts.iter().any(|(_, props)| {
        props.text.len() == 2 && props.text.chars().all(|ch| ch.is_ascii_digit())
    }));
}

#[test]
fn layout_manager_recalculate_moves_widgets_across_resize() {
    let mut layout = LayoutManager::new(400.0, 300.0);
    layout.add_widget(WidgetWrapper::new(
        TrackingModule::new("alpha", "alpha", 50.0, 20.0),
        Anchor::TopLeft,
        10.0,
        10.0,
    ));
    layout.add_widget(WidgetWrapper::new(
        TrackingModule::new("beta", "beta", 60.0, 30.0),
        Anchor::BottomRight,
        5.0,
        7.0,
    ));
    layout.add_widget(WidgetWrapper::new(
        TrackingModule::new("gamma", "gamma", 100.0, 40.0),
        Anchor::Center,
        -20.0,
        15.0,
    ));

    let mut scene = Scene::new();
    layout.init_all(&mut scene);
    assert_eq!(scene.len(), 3);

    let (_, alpha_before) = text_node_by_prefix(&scene, "alpha#");
    let (_, beta_before) = text_node_by_prefix(&scene, "beta#");
    let (_, gamma_before) = text_node_by_prefix(&scene, "gamma#");
    assert_eq!((alpha_before.x, alpha_before.y), (10.0, 10.0));
    assert_eq!((beta_before.x, beta_before.y), (335.0, 263.0));
    assert_eq!((gamma_before.x, gamma_before.y), (130.0, 145.0));

    layout.recalculate(800.0, 600.0, &mut scene);
    assert_eq!(scene.len(), 3);

    let (_, alpha_after) = text_node_by_prefix(&scene, "alpha#");
    let (_, beta_after) = text_node_by_prefix(&scene, "beta#");
    let (_, gamma_after) = text_node_by_prefix(&scene, "gamma#");
    assert_eq!((alpha_after.x, alpha_after.y), (10.0, 10.0));
    assert_eq!((beta_after.x, beta_after.y), (735.0, 563.0));
    assert_eq!((gamma_after.x, gamma_after.y), (330.0, 295.0));
}

#[test]
fn layout_manager_hit_testing_tracks_enablement_and_resize() {
    let mut layout = LayoutManager::new(100.0, 80.0);
    layout.add_widget(WidgetWrapper::new(
        TrackingModule::new("probe", "probe", 40.0, 20.0),
        Anchor::BottomRight,
        10.0,
        10.0,
    ));

    let mut scene = Scene::new();
    layout.init_all(&mut scene);

    assert_eq!(layout.hit_test(55.0, 55.0), Some("probe"));
    assert_eq!(layout.hit_test(5.0, 5.0), None);

    assert!(layout.set_enabled("probe", false, &mut scene));
    assert_eq!(layout.hit_test(55.0, 55.0), None);

    assert!(layout.set_enabled("probe", true, &mut scene));
    assert_eq!(layout.hit_test(55.0, 55.0), Some("probe"));

    layout.recalculate(200.0, 160.0, &mut scene);
    assert_eq!(layout.hit_test(55.0, 55.0), None);
    assert_eq!(layout.hit_test(155.0, 135.0), Some("probe"));
}

#[test]
fn widget_wrapper_anchor_change_updates_bounding_box_and_hit_testing() {
    let mut wrapper = WidgetWrapper::new(
        TrackingModule::new("wrapper", "wrapper", 30.0, 10.0),
        Anchor::TopLeft,
        10.0,
        15.0,
    );

    wrapper.recalculate(200.0, 100.0);
    let before = wrapper.bounding_box();
    assert_eq!(
        (before.x, before.y, before.width, before.height),
        (10.0, 15.0, 30.0, 10.0)
    );
    assert!(wrapper.contains_point(20.0, 20.0));

    wrapper.set_anchor(Anchor::Center);
    let after = wrapper.bounding_box();
    assert_eq!(
        (after.x, after.y, after.width, after.height),
        (95.0, 60.0, 30.0, 10.0)
    );
    assert!(!wrapper.contains_point(20.0, 20.0));
    assert!(wrapper.contains_point(100.0, 65.0));
}

#[test]
fn multi_module_scene_isolation_keeps_node_ids_unique_and_texts_scoped() {
    let mut scene = Scene::new();
    let mut clock = ClockModule::new("%S");
    clock.set_position(15.0, 20.0);

    let mut temps = vec![Some(80.0), Some(82.0)].into_iter();
    let mut stats = SystemStatsModule::new();
    stats.set_position(20.0, 50.0);
    stats.set_interval(Duration::ZERO);
    stats.set_temp_source(Box::new(move || temps.next().flatten()));

    clock.init(&mut scene);
    stats.init(&mut scene);
    assert_eq!(scene.len(), 3);
    unique_ids(&scene);

    std::thread::sleep(Duration::from_millis(1_100));
    assert!(clock.update(&mut scene, Duration::ZERO));
    assert!(stats.update(&mut scene, Duration::ZERO));

    let texts = text_nodes(&scene);
    assert_eq!(texts.len(), 3);
    assert_eq!(
        texts
            .iter()
            .filter(|(_, props)| props.text.len() == 2
                && props.text.chars().all(|ch| ch.is_ascii_digit()))
            .count(),
        1
    );
    assert_eq!(
        texts
            .iter()
            .filter(|(_, props)| props.text.starts_with("system: CPU ")
                && props.text.contains("temp 82°C"))
            .count(),
        1
    );
    assert_eq!(
        texts
            .iter()
            .filter(|(_, props)| props.text.starts_with("system: RAM "))
            .count(),
        1
    );

    clock.deinit(&mut scene);
    stats.deinit(&mut scene);
    assert!(scene.is_empty());
}
