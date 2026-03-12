pub use mastra_voice_core::{
    ResolvedListenRequest, ResolvedSpeakRequest, SpeakerCatalog, StaticVoiceProvider,
    VoiceCapabilities, VoiceListenRequest, VoiceModelProfile, VoiceProviderAdapter,
    VoiceProviderError, VoiceProviderProfile, VoiceSpeakRequest, VoiceSpeakerProfile,
    VoiceTransport,
};

const SPEAKERS: &[VoiceSpeakerProfile] = &[
    VoiceSpeakerProfile::new("1", "Neutral"),
    VoiceSpeakerProfile::new("2", "Male"),
    VoiceSpeakerProfile::new("3", "Warm"),
    VoiceSpeakerProfile::new("4", "Deep Male"),
    VoiceSpeakerProfile::new("5", "Female"),
    VoiceSpeakerProfile::new("6", "Clear Female"),
];

pub const PROFILE: VoiceProviderProfile = VoiceProviderProfile {
    id: "modelslab",
    display_name: "ModelsLab Voice",
    description: "ModelsLab text-to-speech provider with numeric preset voice ids.",
    env_vars: &["MODELSLAB_API_KEY"],
    speech_models: &[VoiceModelProfile::new("default", VoiceTransport::Http)],
    listening_models: &[],
    speaker_catalog: SpeakerCatalog::Static(SPEAKERS),
    default_speaker: Some("1"),
    capabilities: VoiceCapabilities {
        speech_synthesis: true,
        speech_recognition: false,
        speaker_catalog: true,
        speaker_diarization: false,
        realtime_session: false,
        input_streaming: false,
        output_streaming: false,
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
    fn resolves_default_modelslab_speaker() {
        let resolved = provider()
            .resolve_speak(VoiceSpeakRequest {
                input: "hello",
                model: None,
                speaker: None,
                audio_format: None,
                stream: false,
            })
            .unwrap();

        assert_eq!(resolved.model, "default");
        assert_eq!(resolved.speaker.as_deref(), Some("1"));
    }

    #[test]
    fn rejects_listening_for_modelslab() {
        let error = provider()
            .resolve_listen(VoiceListenRequest {
                model: None,
                mime_type: None,
                diarize: false,
                realtime: false,
            })
            .unwrap_err();

        assert_eq!(
            error,
            VoiceProviderError::CapabilityUnavailable {
                provider: "modelslab",
                capability: "speech recognition",
            }
        );
    }
}
