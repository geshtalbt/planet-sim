use bevy_egui::egui;

use crate::history::History;
use crate::localization::{tr, translated};
use crate::rendering::MapLayer;
use crate::simulation::PlanetState;
use crate::ui::{ChartMetric, format_compact, muted};

pub fn draw_legend(ui: &mut egui::Ui, layer: MapLayer, planet: &PlanetState) {
    match layer {
        MapLayer::Natural => {
            ui.label(
                egui::RichText::new(tr("Surface type"))
                    .size(10.0)
                    .color(muted()),
            );
            for (color, label) in [
                ([0.08, 0.30, 0.50], "Ocean / lake"),
                ([0.10, 0.36, 0.13], "Forest"),
                ([0.55, 0.52, 0.25], "Dry land"),
                ([0.92, 0.96, 1.0], "Ice / snow"),
                ([1.0, 0.32, 0.06], "Lava"),
            ] {
                ui.horizontal(|ui| {
                    let (rect, _) =
                        ui.allocate_exact_size(egui::vec2(12.0, 12.0), egui::Sense::hover());
                    ui.painter()
                        .rect_filled(rect, egui::CornerRadius::same(2), rgb(color));
                    ui.label(egui::RichText::new(translated(label)).size(10.0));
                });
            }
        }
        MapLayer::Biome => {
            for (biome, label) in [
                (
                    crate::terrain::Biome::TropicalRainforest,
                    "Tropical rainforest",
                ),
                (crate::terrain::Biome::Savanna, "Savanna"),
                (crate::terrain::Biome::Desert, "Desert"),
                (crate::terrain::Biome::TemperateForest, "Temperate forest"),
                (crate::terrain::Biome::Tundra, "Tundra"),
                (crate::terrain::Biome::IceSheet, "Ice sheet"),
            ] {
                ui.horizontal(|ui| {
                    let (rect, _) =
                        ui.allocate_exact_size(egui::vec2(12.0, 12.0), egui::Sense::hover());
                    ui.painter()
                        .rect_filled(rect, egui::CornerRadius::same(2), rgb(biome.color()));
                    ui.label(egui::RichText::new(translated(label)).size(10.0));
                });
            }
        }
        MapLayer::Tectonics => {
            ui.label(
                egui::RichText::new(
                    tr("Colours identify plates; pale cells lie on a boundary. Boundaries are recomputed from the simulated plate assignment."),
                )
                .size(10.0)
                .color(muted()),
            );
        }
        MapLayer::Volcanism => {
            ui.label(
                egui::RichText::new(tr("Black = quiet · orange = active · pale = recent lava"))
                    .size(10.0)
                    .color(muted()),
            );
            draw_gradient(ui, layer, 0.0, 1.0, tr("activity"));
        }
        MapLayer::Geology => {
            ui.label(
                egui::RichText::new(tr("Elevation tint plus volcanic/seismic heat overlay"))
                    .size(10.0)
                    .color(muted()),
            );
        }
        _ => {
            if let Some((min, max)) = layer.range() {
                draw_gradient(ui, layer, min, max, layer.unit());
            }
        }
    }
    if layer == MapLayer::Co2 {
        ui.label(
            egui::RichText::new(tr(
                "CO₂ is treated as well mixed; this layer is intentionally uniform.",
            ))
            .size(9.5)
            .color(muted()),
        );
    }
    if layer == MapLayer::Temperature {
        ui.label(
            egui::RichText::new(format!(
                "{}: {:.1} °C",
                tr("Current global mean"),
                planet.stats.mean_temp_c
            ))
            .size(10.0)
            .color(muted()),
        );
    }
}

fn draw_gradient(ui: &mut egui::Ui, layer: MapLayer, min: f32, max: f32, unit: &str) {
    let (rect, _) = ui.allocate_exact_size(
        egui::vec2(ui.available_width().max(80.0), 14.0),
        egui::Sense::hover(),
    );
    let painter = ui.painter();
    let segments = 48;
    for i in 0..segments {
        let t = i as f32 / (segments - 1) as f32;
        let color = sample_srgb(layer.stops(), t);
        let x0 = rect.left() + rect.width() * i as f32 / segments as f32;
        let x1 = rect.left() + rect.width() * (i + 1) as f32 / segments as f32 + 0.5;
        painter.rect_filled(
            egui::Rect::from_min_max(egui::pos2(x0, rect.top()), egui::pos2(x1, rect.bottom())),
            egui::CornerRadius::same(0),
            rgb(color),
        );
    }
    ui.horizontal(|ui| {
        ui.label(
            egui::RichText::new(format_value(min, unit))
                .size(9.5)
                .monospace(),
        );
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            ui.label(
                egui::RichText::new(format_value(max, unit))
                    .size(9.5)
                    .monospace(),
            );
        });
    });
}

pub fn draw_history_chart(
    ui: &mut egui::Ui,
    history: &History,
    metric: ChartMetric,
    time_fraction: f32,
    height: f32,
) {
    draw_multi_chart(
        ui,
        &[(history, metric_color(metric), "current")],
        metric,
        time_fraction,
        height,
    );
}

pub fn draw_multi_chart(
    ui: &mut egui::Ui,
    series: &[(&History, egui::Color32, &str)],
    metric: ChartMetric,
    time_fraction: f32,
    height: f32,
) {
    let width = ui.available_width().max(120.0);
    let (rect, _) = ui.allocate_exact_size(egui::vec2(width, height), egui::Sense::hover());
    let painter = ui.painter_at(rect);
    painter.rect_filled(
        rect,
        egui::CornerRadius::same(4),
        egui::Color32::from_rgb(6, 9, 14),
    );

    let end = series
        .iter()
        .filter_map(|(h, _, _)| h.samples.last().map(|s| s.time_years))
        .fold(0.0f64, f64::max);
    let start_all = series
        .iter()
        .filter_map(|(h, _, _)| h.samples.first().map(|s| s.time_years))
        .fold(end, f64::min);
    let span = (end - start_all).max(1.0e-9);
    let visible = (time_fraction.clamp(0.02, 1.0) as f64) * span;
    let start = end - visible;

    let mut values: Vec<f64> = Vec::new();
    for (history, _, _) in series {
        for sample in &history.samples {
            if sample.time_years >= start {
                values.push(metric.value(sample));
            }
        }
    }
    if values.is_empty() {
        painter.text(
            rect.center(),
            egui::Align2::CENTER_CENTER,
            crate::localization::pick("No history yet", "История пока пуста"),
            egui::FontId::proportional(11.0),
            muted(),
        );
        return;
    }
    let mut y_min = values.iter().copied().fold(f64::INFINITY, f64::min);
    let mut y_max = values.iter().copied().fold(f64::NEG_INFINITY, f64::max);
    if (y_max - y_min).abs() < 1.0e-9 {
        y_min -= 1.0;
        y_max += 1.0;
    }
    let pad = (y_max - y_min) * 0.08;
    y_min -= pad;
    y_max += pad;

    let plot = rect.shrink2(egui::vec2(38.0, 18.0));
    for i in 1..4 {
        let y = plot.top() + plot.height() * i as f32 / 4.0;
        painter.line_segment(
            [egui::pos2(plot.left(), y), egui::pos2(plot.right(), y)],
            egui::Stroke::new(
                1.0,
                egui::Color32::from_rgba_unmultiplied(110, 130, 145, 45),
            ),
        );
    }
    for i in 1..5 {
        let x = plot.left() + plot.width() * i as f32 / 5.0;
        painter.line_segment(
            [egui::pos2(x, plot.top()), egui::pos2(x, plot.bottom())],
            egui::Stroke::new(
                1.0,
                egui::Color32::from_rgba_unmultiplied(110, 130, 145, 28),
            ),
        );
    }

    for (history, color, _) in series {
        let points: Vec<egui::Pos2> = history
            .samples
            .iter()
            .filter(|s| s.time_years >= start && s.time_years <= end)
            .map(|s| {
                let tx = ((s.time_years - start) / (end - start).max(1.0e-9)) as f32;
                let v = metric.value(s);
                let ty = ((v - y_min) / (y_max - y_min).max(1.0e-12)) as f32;
                egui::pos2(
                    plot.left() + plot.width() * tx,
                    plot.bottom() - plot.height() * ty,
                )
            })
            .collect();
        if points.len() >= 2 {
            painter.add(egui::Shape::line(points, egui::Stroke::new(1.7, *color)));
        } else if let Some(point) = points.first() {
            painter.circle_filled(*point, 1.8, *color);
        }
    }

    painter.text(
        egui::pos2(rect.left() + 6.0, rect.top() + 4.0),
        egui::Align2::LEFT_TOP,
        format!("{} ({})", metric.label(), metric.unit()),
        egui::FontId::proportional(10.0),
        egui::Color32::from_rgb(170, 188, 200),
    );
    let latest = series
        .iter()
        .filter_map(|(h, _, _)| h.samples.last())
        .map(|s| metric.value(s))
        .last();
    if let Some(latest) = latest {
        painter.text(
            egui::pos2(rect.right() - 6.0, rect.top() + 4.0),
            egui::Align2::RIGHT_TOP,
            format_axis_value(latest, metric.unit()),
            egui::FontId::monospace(10.0),
            series.last().map(|s| s.1).unwrap_or(egui::Color32::WHITE),
        );
    }
    painter.text(
        egui::pos2(plot.left() - 4.0, plot.top()),
        egui::Align2::RIGHT_TOP,
        format_axis_value(y_max, ""),
        egui::FontId::monospace(8.5),
        muted(),
    );
    painter.text(
        egui::pos2(plot.left() - 4.0, plot.bottom()),
        egui::Align2::RIGHT_BOTTOM,
        format_axis_value(y_min, ""),
        egui::FontId::monospace(8.5),
        muted(),
    );
    painter.text(
        egui::pos2(plot.left(), plot.bottom() + 3.0),
        egui::Align2::LEFT_TOP,
        format_axis_time(start),
        egui::FontId::monospace(8.5),
        muted(),
    );
    painter.text(
        egui::pos2(plot.right(), plot.bottom() + 3.0),
        egui::Align2::RIGHT_TOP,
        format_axis_time(end),
        egui::FontId::monospace(8.5),
        muted(),
    );
}

pub fn metric_color(metric: ChartMetric) -> egui::Color32 {
    metric.color()
}

fn sample_srgb(stops: &[(f32, [f32; 3])], t: f32) -> [f32; 3] {
    let t = t.clamp(0.0, 1.0);
    let mut prev = stops[0];
    for &next in stops.iter().skip(1) {
        if t <= next.0 {
            let f = (t - prev.0) / (next.0 - prev.0).max(1.0e-6);
            return [
                prev.1[0] + (next.1[0] - prev.1[0]) * f,
                prev.1[1] + (next.1[1] - prev.1[1]) * f,
                prev.1[2] + (next.1[2] - prev.1[2]) * f,
            ];
        }
        prev = next;
    }
    prev.1
}

fn rgb(c: [f32; 3]) -> egui::Color32 {
    egui::Color32::from_rgb(
        (c[0].clamp(0.0, 1.0) * 255.0) as u8,
        (c[1].clamp(0.0, 1.0) * 255.0) as u8,
        (c[2].clamp(0.0, 1.0) * 255.0) as u8,
    )
}

pub fn format_value(value: f32, unit: &str) -> String {
    let v = value.abs();
    if v >= 1.0e6 {
        format!("{:.2}M{unit}", value / 1.0e6)
    } else if v >= 1.0e4 {
        format!("{:.1}k{unit}", value / 1.0e3)
    } else if v >= 100.0 {
        format!("{value:.0}{unit}")
    } else if v >= 1.0 {
        format!("{value:.1}{unit}")
    } else {
        format!("{value:.3}{unit}")
    }
}

fn format_axis_value(value: f64, unit: &str) -> String {
    if value.abs() >= 1.0e6 {
        format!("{:.2}M{}", value / 1.0e6, unit)
    } else if value.abs() >= 1.0e4 {
        format!("{:.1}k{}", value / 1.0e3, unit)
    } else if value.abs() >= 100.0 {
        format!("{value:.0}{unit}")
    } else {
        format!("{value:.2}{unit}")
    }
}

fn format_axis_time(years: f64) -> String {
    if crate::localization::is_russian() {
        if years.abs() >= 1.0e9 {
            format!("{:.2} млрд лет", years / 1.0e9)
        } else if years.abs() >= 1.0e6 {
            format!("{:.2} млн лет", years / 1.0e6)
        } else if years.abs() >= 1.0e3 {
            format!("{:.1} тыс. лет", years / 1.0e3)
        } else {
            format!("{years:.1} лет")
        }
    } else if years.abs() >= 1.0e9 {
        format!("{:.2} Gyr", years / 1.0e9)
    } else if years.abs() >= 1.0e6 {
        format!("{:.2} Myr", years / 1.0e6)
    } else if years.abs() >= 1.0e3 {
        format!("{:.1} kyr", years / 1.0e3)
    } else {
        format!("{years:.1} yr")
    }
}

// Used by panels that need a compact value in a tooltip.
pub fn compact(value: f64, unit: &str) -> String {
    format_compact(value, unit)
}
