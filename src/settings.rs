use godot::{
    engine::ProjectSettings,
    global::PropertyHint,
    prelude::*,
};

const SETTING_THREAD_COUNT: &str = "sorkin_movie_writer/thread_count";
const SETTING_CODEC: &str = "sorkin_movie_writer/codec";
const SETTING_QUALITY: &str = "sorkin_movie_writer/quality";
const SETTING_ALPHA_CHANNEL: &str = "sorkin_movie_writer/alpha_channel";
#[derive(Clone)]
pub struct EncoderConfig {
    pub thread_count: u32,
    pub quality: Quality,
    pub alpha_channel: bool,
    pub codec: Codec,
}

#[derive(Clone)]
pub enum Quality {
    Realtime,
    Good,
    Best,
}

#[derive(Clone)]
pub enum Codec {
    VP9,
    H264,
    AV1,
}

impl Default for EncoderConfig {
    fn default() -> Self {
        Self {
            thread_count: 0, // 0 = auto-detect
            quality: Quality::Realtime,
            alpha_channel: false,
            codec: Codec::VP9,
        }
    }
}

impl EncoderConfig {
    pub fn from_project_settings() -> Self {
        let project_settings = ProjectSettings::singleton();

        let thread_count = project_settings
            .get_setting(SETTING_THREAD_COUNT.into())
            .try_to::<u32>()
            .unwrap_or(0);

        let codec = project_settings
            .get_setting(SETTING_CODEC.into())
            .try_to::<GString>()
            .map(|s| match s.to_string().as_str() {
                "H264" => Codec::H264,
                "AV1" => Codec::AV1,
                _ => Codec::VP9,
            })
            .unwrap_or(Codec::VP9);

        let quality = project_settings
            .get_setting(SETTING_QUALITY.into())
            .try_to::<GString>()
            .ok()
            .map(|s| match s.to_string().as_str() {
                "Good" => Quality::Good,
                "Best" => Quality::Best,
                _ => Quality::Realtime,
            })
            .unwrap_or(Quality::Realtime);

        let alpha_channel = project_settings
            .get_setting(SETTING_ALPHA_CHANNEL.into())
            .try_to::<bool>()
            .ok()
            .unwrap_or(false);

        Self {
            thread_count,
            quality,
            alpha_channel,
            codec,
        }
    }

    pub fn register_project_settings() {
        let mut project_settings = ProjectSettings::singleton();

        let thread_count_name = SETTING_THREAD_COUNT.to_godot();
        if !project_settings.has_setting(thread_count_name.clone()) {
            project_settings.set(thread_count_name.clone().into(), 0i32.to_variant());

            let thread_count_info = dict! {
                "name": thread_count_name.clone(),
                "type": VariantType::INT,
                "hint": PropertyHint::RANGE,
                "hint_string": "0,32,1,or_greater".to_godot()
            };
            project_settings.add_property_info(thread_count_info);
        }

        let quality_name = SETTING_QUALITY.to_godot();
        if !project_settings.has_setting(quality_name.clone()) {
            project_settings.set(quality_name.clone().into(), "Realtime".to_variant());

            let quality_info = dict! {
                "name": quality_name.clone(),
                "type": VariantType::STRING,
                "hint": PropertyHint::ENUM,
                "hint_string": "Realtime,Good,Best".to_godot()
            };
            project_settings.add_property_info(quality_info);
        }

        let alpha_name = SETTING_ALPHA_CHANNEL.to_godot();
        if !project_settings.has_setting(alpha_name.clone()) {
            project_settings.set(alpha_name.clone().into(), false.to_variant());

            let alpha_info = dict! {
                "name": alpha_name.clone(),
                "type": VariantType::BOOL,
                "hint": PropertyHint::NONE,
                "hint_string": "".to_godot()
            };
            project_settings.add_property_info(alpha_info);
        }

        let codec_name = SETTING_CODEC.to_godot();
        if !project_settings.has_setting(codec_name.clone()) {
            project_settings.set((&codec_name).into(), "VP9".to_variant());

            let codec_info = dict! {
                "name": codec_name.clone(),
                "type": VariantType::STRING,
                "hint": PropertyHint::ENUM,
                "hint_string": "VP9,H264,AV1".to_godot()
            };
            project_settings.add_property_info(codec_info);
        }

        godot_print!("Sorkin encoder settings registered in Editor Settings under Sorkin category");
    }
}
