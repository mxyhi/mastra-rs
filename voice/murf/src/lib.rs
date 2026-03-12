pub use mastra_voice_core::{
    ResolvedListenRequest, ResolvedSpeakRequest, SpeakerCatalog, StaticVoiceProvider,
    VoiceCapabilities, VoiceListenRequest, VoiceModelProfile, VoiceProviderAdapter,
    VoiceProviderError, VoiceProviderProfile, VoiceSpeakRequest, VoiceSpeakerProfile,
    VoiceTransport,
};

pub const PROFILE: VoiceProviderProfile = VoiceProviderProfile {
    id: "murf",
    display_name: "Murf Voice",
    description: "Murf text-to-speech provider with provider-managed voice catalogue.",
    env_vars: &["MURF_API_KEY"],
    speech_models: &[
        VoiceModelProfile::new("GEN2", VoiceTransport::Http),
        VoiceModelProfile::new("GEN1", VoiceTransport::Http),
    ],
    listening_models: &[],
    speaker_catalog: SpeakerCatalog::Dynamic,
    default_speaker: Some("en-UK-hazel"),
    capabilities: VoiceCapabilities {
        speech_synthesis: true,
        speech_recognition: false,
        speaker_catalog: true,
        speaker_diarization: false,
        realtime_session: false,
        input_streaming: false,
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
    fn accepts_dynamic_murf_voice_ids() {
        let resolved = provider()
            .resolve_speak(VoiceSpeakRequest {
                input: "hello",
                model: Some("GEN1"),
                speaker: Some("en-US-charlotte"),
                audio_format: Some("MP3"),
                stream: false,
            })
            .unwrap();

        assert_eq!(resolved.model, "GEN1");
        assert_eq!(resolved.speaker.as_deref(), Some("en-US-charlotte"));
    }

    #[test]
    fn rejects_listening_for_murf() {
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
                provider: "murf",
                capability: "speech recognition",
            }
        );
    }
}
