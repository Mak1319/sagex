//! Bundled fonts: Geist (body), Space Grotesk (headlines), JetBrains Mono (labels/code).
use eframe::egui;

pub fn install(ctx: &egui::Context) {
    let mut fonts = egui::FontDefinitions::default();
    fonts.font_data.insert(
        "Geist".into(),
        egui::FontData::from_static(include_bytes!("../assets/fonts/Geist.ttf")).into(),
    );
    fonts.font_data.insert(
        "SpaceGrotesk".into(),
        egui::FontData::from_static(include_bytes!("../assets/fonts/SpaceGrotesk.ttf"))
            .into(),
    );
    fonts.font_data.insert(
        "JetBrainsMono".into(),
        egui::FontData::from_static(include_bytes!("../assets/fonts/JetBrainsMono.ttf"))
            .into(),
    );
    fonts
        .families
        .insert(egui::FontFamily::Proportional, vec!["Geist".into()]);
    fonts.families.insert(
        egui::FontFamily::Monospace,
        vec!["JetBrainsMono".into()],
    );
    fonts.families.insert(
        egui::FontFamily::Name("headline".into()),
        vec!["SpaceGrotesk".into()],
    );
    ctx.set_fonts(fonts);

    // text styles: headlines -> Space Grotesk, body -> Geist (default proportional), mono stays
    ctx.all_styles_mut(|s| {
        use egui::TextStyle as T;
        let head = egui::FontId::new(15.0, egui::FontFamily::Name("headline".into()));
        s.text_styles.insert(T::Heading, head);
        s.text_styles.insert(
            T::Body,
            egui::FontId::new(13.0, egui::FontFamily::Proportional),
        );
        s.text_styles.insert(
            T::Small,
            egui::FontId::new(11.0, egui::FontFamily::Proportional),
        );
        s.text_styles.insert(
            T::Monospace,
            egui::FontId::new(11.0, egui::FontFamily::Monospace),
        );
        s.text_styles.insert(
            T::Button,
            egui::FontId::new(11.0, egui::FontFamily::Monospace),
        );
    });
}
