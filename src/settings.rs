use godot::{engine::ProjectSettings, global::PropertyHint, prelude::*};

const SETTING_THREAD_COUNT: &str = "sorkin_movie_writer/thread_count";
const SETTING_QUALITY: &str = "sorkin_movie_writer/quality";
const SETTING_ALPHA_CHANNEL: &str = "sorkin_movie_writer/alpha_channel";
const SETTING_ENABLE_AUDIO: &str = "sorkin_movie_writer/enable_audio";

#[derive(Clone, Debug)]
pub struct EncoderConfig {
    pub thread_count: u32,
    pub quality: Quality,
    pub alpha_channel: bool,
    pub enable_audio: bool,
}

#[derive(Clone, Debug)]
pub enum Quality {
    Realtime,
    Good,
    Best,
}

impl Default for EncoderConfig {
    fn default() -> Self {
        Self {
            thread_count: 0, // 0 = auto-detect
            quality: Quality::Realtime,
            alpha_channel: false,
            enable_audio: true,
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

        let enable_audio = project_settings
            .get_setting(SETTING_ENABLE_AUDIO.into())
            .try_to::<bool>()
            .ok()
            .unwrap_or(true);

        Self {
            thread_count,
            quality,
            alpha_channel,
            enable_audio,
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
                "hint_string": "0,32,1,or_greater".to_godot(),
                "description": "The number of threads to dedicate to encoding the video - 0 means all available",
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
                "description": "Include alpha channel (transparency) in the file? This will slow down encoding and is only possible if the true."
            };
            project_settings.add_property_info(alpha_info);
        }

        let enable_audio_name = SETTING_ENABLE_AUDIO.to_godot();
        if !project_settings.has_setting(enable_audio_name.clone()) {
            project_settings.set(enable_audio_name.clone().into(), true.to_variant());

            let enable_audio_info = dict! {
                "name": enable_audio_name.clone(),
                "type": VariantType::BOOL,
                "hint": PropertyHint::NONE,
                "description": "Include audio stream in the WebM file. Disable for video-only output."
            };
            project_settings.add_property_info(enable_audio_info);
        }

        godot_print!("Sorkin encoder settings registered in Editor Settings under Sorkin category");
    }
}
