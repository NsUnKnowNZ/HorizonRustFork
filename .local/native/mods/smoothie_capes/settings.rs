fn smoothie_capes_settings(game: &crate::game::Game<'_>, mod_id: &str, buttons: &mut Vec<String>) {
    if mod_id != "smoothie_capes" {
        return;
    }
    use horizon_client::lang;
    let enabled = crate::game::smoothie_elytra_config::enabled(game);
    let code = lang::code();
    let label = match code.as_str() {
        "pt_pt" => "Animação dos élitros",
        "zh_tw" | "zh_hk" => "鞘翅動畫",
        "es_es" => "Animación de los élitros",
        _ => match code.split('_').next().unwrap_or_default() {
            "pt" => "Animação da elytra",
            "es" => "Animación de la elytra",
            "fr" => "Animation des élytres",
            "de" => "Elytren-Animation",
            "it" => "Animazione delle elitre",
            "nl" => "Elytra-animatie",
            "pl" => "Animacja elytry",
            "ru" => "Анимация элитр",
            "uk" => "Анімація елітр",
            "tr" => "Elitra animasyonu",
            "ja" => "エリトラのアニメーション",
            "ko" => "겉날개 애니메이션",
            "zh" => "鞘翅动画",
            "cs" => "Animace krovek",
            "sk" => "Animácia kroviek",
            "hu" => "Elytra animáció",
            "ro" => "Animația elitrelor",
            "sv" => "Elytra-animation",
            "da" => "Elytra-animation",
            "no" | "nb" | "nn" => "Elytra-animasjon",
            "fi" => "Liitosiipien animaatio",
            "el" => "Κίνηση των ελύτρων",
            "bg" => "Анимация на елитрата",
            "id" => "Animasi elytra",
            "vi" => "Hoạt ảnh cánh cứng",
            "th" => "แอนิเมชันปีกเอลีทรา",
            "ar" => "حركة الإليترا",
            "he" => "אנימציית אליטרה",
            _ => "Elytra Animation",
        },
    };
    let status = if enabled {
        lang::get_or("options.on", "On")
    } else {
        lang::get_or("options.off", "Off")
    };
    buttons.push(lang::tr("options.generic_value", &[&label, &status]));
}
