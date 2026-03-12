pub use mastra_voice_core::{
    ResolvedListenRequest, ResolvedSpeakRequest, SpeakerCatalog, StaticVoiceProvider,
    VoiceCapabilities, VoiceListenRequest, VoiceModelProfile, VoiceProviderAdapter,
    VoiceProviderError, VoiceProviderProfile, VoiceSpeakRequest, VoiceSpeakerProfile,
    VoiceTransport,
};

pub const PROFILE: VoiceProviderProfile = VoiceProviderProfile {
    id: "azure",
    display_name: "Azure Speech",
    description: "Azure Cognitive Services speech synthesis and speech recognition provider.",
    env_vars: &["AZURE_API_KEY", "AZURE_REGION"],
    speech_models: &[VoiceModelProfile::new(
        "azure-speech-synthesis",
        VoiceTransport::Http,
    )],
    listening_models: &[VoiceModelProfile::new(
        "azure-speech-recognition",
        VoiceTransport::Http,
    )],
    speaker_catalog: SpeakerCatalog::Dynamic,
    default_speaker: Some("en-US-AriaNeural"),
    capabilities: VoiceCapabilities {
        speech_synthesis: true,
        speech_recognition: true,
        speaker_catalog: true,
        speaker_diarization: false,
        realtime_session: false,
        input_streaming: true,
        output_streaming: true,
        tool_calling: false,
        worker_binding: false,
    },
};

pub const PROVIDER: StaticVoiceProvider = StaticVoiceProvider::new(&PROFILE);

pub const fn provider() -> StaticVoiceProvider {
    StaticVoiceProvider::new(&PROFILE)
}

pub fn profile() -> &'static VoiceProviderProfile {
    &PROFILE
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_dynamic_azure_voice_names() {
        let resolved = provider()
            .resolve_speak(VoiceSpeakRequest {
                input: "hello",
                model: None,
                speaker: Some("zh-CN-XiaoxiaoNeural"),
                audio_format: Some("wav"),
                stream: false,
            })
            .unwrap();

        assert_eq!(resolved.speaker.as_deref(), Some("zh-CN-XiaoxiaoNeural"));
        assert_eq!(resolved.model, "azure-speech-synthesis");
    }

    #[test]
    fn resolves_azure_listening_profile() {
        let resolved = provider()
            .resolve_listen(VoiceListenRequest {
                model: None,
                mime_type: Some("audio/wav"),
                diarize: false,
                realtime: false,
            })
            .unwrap();

        assert_eq!(resolved.model, "azure-speech-recognition");
    }
}
