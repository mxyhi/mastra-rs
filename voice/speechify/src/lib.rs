pub use mastra_voice_core::{
    ResolvedListenRequest, ResolvedSpeakRequest, SpeakerCatalog, StaticVoiceProvider,
    VoiceCapabilities, VoiceListenRequest, VoiceModelProfile, VoiceProviderAdapter,
    VoiceProviderError, VoiceProviderProfile, VoiceSpeakRequest, VoiceSpeakerProfile,
    VoiceTransport,
};

pub const PROFILE: VoiceProviderProfile = VoiceProviderProfile {
    id: "speechify",
    display_name: "Speechify Voice",
    description: "Speechify text-to-speech provider with provider-managed voice ids.",
    env_vars: &["SPEECHIFY_API_KEY"],
    speech_models: &[
        VoiceModelProfile::new("simba-english", VoiceTransport::Http),
        VoiceModelProfile::new("simba-multilingual", VoiceTransport::Http),
    ],
    listening_models: &[],
    speaker_catalog: SpeakerCatalog::Dynamic,
    default_speaker: Some("george"),
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
    fn accepts_dynamic_speechify_voices() {
        let resolved = provider()
            .resolve_speak(VoiceSpeakRequest {
                input: "hello",
                model: Some("simba-multilingual"),
                speaker: Some("henry"),
                audio_format: None,
                stream: true,
            })
            .unwrap();

        assert_eq!(resolved.model, "simba-multilingual");
        assert_eq!(resolved.speaker.as_deref(), Some("henry"));
    }

    #[test]
    fn rejects_listening_for_speechify() {
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
                provider: "speechify",
                capability: "speech recognition",
            }
        );
    }
}
