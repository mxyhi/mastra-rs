pub use mastra_voice_core::{
    ResolvedListenRequest, ResolvedSpeakRequest, SpeakerCatalog, StaticVoiceProvider,
    VoiceCapabilities, VoiceListenRequest, VoiceModelProfile, VoiceProviderAdapter,
    VoiceProviderError, VoiceProviderProfile, VoiceSpeakRequest, VoiceSpeakerProfile,
    VoiceTransport,
};

pub const PROFILE: VoiceProviderProfile = VoiceProviderProfile {
    id: "google",
    display_name: "Google Cloud Voice",
    description: "Google Cloud Text-to-Speech and Speech-to-Text provider with dynamic speaker ids.",
    env_vars: &[
        "GOOGLE_API_KEY",
        "GOOGLE_APPLICATION_CREDENTIALS",
        "GOOGLE_CLOUD_PROJECT",
        "GOOGLE_CLOUD_LOCATION",
    ],
    speech_models: &[VoiceModelProfile::new(
        "google-cloud-text-to-speech",
        VoiceTransport::Http,
    )],
    listening_models: &[VoiceModelProfile::new(
        "google-cloud-speech-to-text",
        VoiceTransport::Http,
    )],
    speaker_catalog: SpeakerCatalog::Dynamic,
    default_speaker: Some("en-US-Studio-O"),
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
    fn accepts_dynamic_google_speakers() {
        let resolved = provider()
            .resolve_speak(VoiceSpeakRequest {
                input: "hello",
                model: None,
                speaker: Some("en-GB-Wavenet-B"),
                audio_format: Some("mp3"),
                stream: false,
            })
            .unwrap();

        assert_eq!(resolved.speaker.as_deref(), Some("en-GB-Wavenet-B"));
        assert_eq!(resolved.model, "google-cloud-text-to-speech");
    }

    #[test]
    fn resolves_google_listening_defaults() {
        let resolved = provider()
            .resolve_listen(VoiceListenRequest {
                model: None,
                mime_type: Some("audio/wav"),
                diarize: false,
                realtime: false,
            })
            .unwrap();

        assert_eq!(resolved.model, "google-cloud-speech-to-text");
        assert_eq!(resolved.transport, VoiceTransport::Http);
    }
}
