pub use mastra_voice_core::{
    ResolvedListenRequest, ResolvedSpeakRequest, SpeakerCatalog, StaticVoiceProvider,
    VoiceCapabilities, VoiceListenRequest, VoiceModelProfile, VoiceProviderAdapter,
    VoiceProviderError, VoiceProviderProfile, VoiceSpeakRequest, VoiceSpeakerProfile,
    VoiceTransport,
};

const SPEAKERS: &[VoiceSpeakerProfile] = &[
    VoiceSpeakerProfile::new("Puck", "Puck"),
    VoiceSpeakerProfile::new("Charon", "Charon"),
    VoiceSpeakerProfile::new("Kore", "Kore"),
    VoiceSpeakerProfile::new("Fenrir", "Fenrir"),
];

pub const PROFILE: VoiceProviderProfile = VoiceProviderProfile {
    id: "google-gemini-live-api",
    display_name: "Google Gemini Live",
    description: "Gemini Live WebSocket voice sessions with multimodal tool-calling.",
    env_vars: &[
        "GOOGLE_API_KEY",
        "GOOGLE_APPLICATION_CREDENTIALS",
        "GOOGLE_CLOUD_PROJECT",
        "GOOGLE_CLOUD_LOCATION",
    ],
    speech_models: &[
        VoiceModelProfile::new("gemini-2.0-flash-exp", VoiceTransport::Websocket),
        VoiceModelProfile::new("gemini-2.0-flash-live-001", VoiceTransport::Websocket),
    ],
    listening_models: &[
        VoiceModelProfile::new("gemini-2.0-flash-exp", VoiceTransport::Websocket),
        VoiceModelProfile::new("gemini-2.0-flash-live-001", VoiceTransport::Websocket),
    ],
    speaker_catalog: SpeakerCatalog::Static(SPEAKERS),
    default_speaker: Some("Puck"),
    capabilities: VoiceCapabilities {
        speech_synthesis: true,
        speech_recognition: true,
        speaker_catalog: true,
        speaker_diarization: false,
        realtime_session: true,
        input_streaming: true,
        output_streaming: true,
        tool_calling: true,
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
    fn exposes_realtime_tool_capable_profile() {
        let provider = provider();
        let speak = provider
            .resolve_speak(VoiceSpeakRequest {
                input: "hello",
                model: None,
                speaker: None,
                audio_format: None,
                stream: true,
            })
            .unwrap();
        let listen = provider
            .resolve_listen(VoiceListenRequest {
                model: None,
                mime_type: Some("audio/pcm"),
                diarize: false,
                realtime: true,
            })
            .unwrap();

        assert_eq!(speak.speaker.as_deref(), Some("Puck"));
        assert_eq!(listen.transport, VoiceTransport::Websocket);
        assert!(profile().capabilities.tool_calling);
    }

    #[test]
    fn validates_known_gemini_live_speakers() {
        let error = provider()
            .resolve_speak(VoiceSpeakRequest {
                input: "hello",
                model: None,
                speaker: Some("Aria"),
                audio_format: None,
                stream: false,
            })
            .unwrap_err();

        assert_eq!(
            error,
            VoiceProviderError::UnknownSpeaker {
                provider: "google-gemini-live-api",
                speaker: "Aria".to_owned(),
            }
        );
    }
}
