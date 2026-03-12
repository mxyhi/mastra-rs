pub use mastra_voice_core::{
    ResolvedListenRequest, ResolvedSpeakRequest, SpeakerCatalog, StaticVoiceProvider,
    VoiceCapabilities, VoiceListenRequest, VoiceModelProfile, VoiceProviderAdapter,
    VoiceProviderError, VoiceProviderProfile, VoiceSpeakRequest, VoiceSpeakerProfile,
    VoiceTransport,
};

pub const PROFILE: VoiceProviderProfile = VoiceProviderProfile {
    id: "gladia",
    display_name: "Gladia Voice",
    description: "Gladia pre-recorded transcription provider with diarization support.",
    env_vars: &["GLADIA_API_KEY"],
    speech_models: &[],
    listening_models: &[VoiceModelProfile::new("gladia", VoiceTransport::Http)],
    speaker_catalog: SpeakerCatalog::None,
    default_speaker: None,
    capabilities: VoiceCapabilities {
        speech_synthesis: false,
        speech_recognition: true,
        speaker_catalog: false,
        speaker_diarization: true,
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
    fn allows_gladia_diarization() {
        let resolved = provider()
            .resolve_listen(VoiceListenRequest {
                model: None,
                mime_type: Some("audio/mpeg"),
                diarize: true,
                realtime: false,
            })
            .unwrap();

        assert_eq!(resolved.model, "gladia");
        assert!(resolved.diarize);
    }

    #[test]
    fn rejects_gladia_speech_generation() {
        let error = provider()
            .resolve_speak(VoiceSpeakRequest {
                input: "hello",
                model: None,
                speaker: None,
                audio_format: None,
                stream: false,
            })
            .unwrap_err();

        assert_eq!(
            error,
            VoiceProviderError::CapabilityUnavailable {
                provider: "gladia",
                capability: "speech synthesis",
            }
        );
    }
}
