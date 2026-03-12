pub use mastra_voice_core::{
    ResolvedListenRequest, ResolvedSpeakRequest, SpeakerCatalog, StaticVoiceProvider,
    VoiceCapabilities, VoiceListenRequest, VoiceModelProfile, VoiceProviderAdapter,
    VoiceProviderError, VoiceProviderProfile, VoiceSpeakRequest, VoiceSpeakerProfile,
    VoiceTransport,
};

const SPEAKERS: &[VoiceSpeakerProfile] = &[
    VoiceSpeakerProfile::new("alloy", "Alloy"),
    VoiceSpeakerProfile::new("ash", "Ash"),
    VoiceSpeakerProfile::new("ballad", "Ballad"),
    VoiceSpeakerProfile::new("coral", "Coral"),
    VoiceSpeakerProfile::new("echo", "Echo"),
    VoiceSpeakerProfile::new("sage", "Sage"),
    VoiceSpeakerProfile::new("shimmer", "Shimmer"),
    VoiceSpeakerProfile::new("verse", "Verse"),
];

pub const PROFILE: VoiceProviderProfile = VoiceProviderProfile {
    id: "openai-realtime-api",
    display_name: "OpenAI Realtime Voice",
    description: "OpenAI realtime WebSocket voice sessions with optional Whisper transcription.",
    env_vars: &["OPENAI_API_KEY"],
    speech_models: &[VoiceModelProfile::new(
        "gpt-4o-mini-realtime-preview-2024-12-17",
        VoiceTransport::Websocket,
    )],
    listening_models: &[
        VoiceModelProfile::new(
            "gpt-4o-mini-realtime-preview-2024-12-17",
            VoiceTransport::Websocket,
        ),
        VoiceModelProfile::new("whisper-1", VoiceTransport::Http),
    ],
    speaker_catalog: SpeakerCatalog::Static(SPEAKERS),
    default_speaker: Some("alloy"),
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
    fn resolves_realtime_requests_over_websocket() {
        let provider = provider();
        let speak = provider
            .resolve_speak(VoiceSpeakRequest {
                input: "hello",
                model: None,
                speaker: Some("verse"),
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

        assert_eq!(speak.transport, VoiceTransport::Websocket);
        assert_eq!(listen.transport, VoiceTransport::Websocket);
        assert!(profile().capabilities.tool_calling);
    }

    #[test]
    fn keeps_http_transcriber_as_explicit_option() {
        let listen = provider()
            .resolve_listen(VoiceListenRequest {
                model: Some("whisper-1"),
                mime_type: Some("audio/webm"),
                diarize: false,
                realtime: false,
            })
            .unwrap();

        assert_eq!(listen.model, "whisper-1");
        assert_eq!(listen.transport, VoiceTransport::Http);
    }
}
