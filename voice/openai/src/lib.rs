pub use mastra_voice_core::{
    ResolvedListenRequest, ResolvedSpeakRequest, SpeakerCatalog, StaticVoiceProvider,
    VoiceCapabilities, VoiceListenRequest, VoiceModelProfile, VoiceProviderAdapter,
    VoiceProviderError, VoiceProviderProfile, VoiceSpeakRequest, VoiceSpeakerProfile,
    VoiceTransport,
};

const SPEAKERS: &[VoiceSpeakerProfile] = &[
    VoiceSpeakerProfile::new("alloy", "Alloy"),
    VoiceSpeakerProfile::new("echo", "Echo"),
    VoiceSpeakerProfile::new("fable", "Fable"),
    VoiceSpeakerProfile::new("onyx", "Onyx"),
    VoiceSpeakerProfile::new("nova", "Nova"),
    VoiceSpeakerProfile::new("shimmer", "Shimmer"),
    VoiceSpeakerProfile::new("ash", "Ash"),
    VoiceSpeakerProfile::new("coral", "Coral"),
    VoiceSpeakerProfile::new("sage", "Sage"),
];

pub const PROFILE: VoiceProviderProfile = VoiceProviderProfile {
    id: "openai",
    display_name: "OpenAI Voice",
    description: "OpenAI HTTP text-to-speech and Whisper transcription provider.",
    env_vars: &["OPENAI_API_KEY"],
    speech_models: &[
        VoiceModelProfile::new("tts-1", VoiceTransport::Http),
        VoiceModelProfile::new("tts-1-hd", VoiceTransport::Http),
    ],
    listening_models: &[VoiceModelProfile::new("whisper-1", VoiceTransport::Http)],
    speaker_catalog: SpeakerCatalog::Static(SPEAKERS),
    default_speaker: Some("alloy"),
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
    fn resolves_openai_defaults() {
        let provider = provider();
        let speak = provider
            .resolve_speak(VoiceSpeakRequest {
                input: "hello",
                model: None,
                speaker: None,
                audio_format: Some("mp3"),
                stream: true,
            })
            .unwrap();
        let listen = provider
            .resolve_listen(VoiceListenRequest {
                model: None,
                mime_type: Some("audio/mpeg"),
                diarize: false,
                realtime: false,
            })
            .unwrap();

        assert_eq!(speak.model, "tts-1");
        assert_eq!(speak.speaker.as_deref(), Some("alloy"));
        assert_eq!(listen.model, "whisper-1");
        assert_eq!(listen.transport, VoiceTransport::Http);
    }

    #[test]
    fn rejects_unknown_openai_speaker() {
        let error = provider()
            .resolve_speak(VoiceSpeakRequest {
                input: "hello",
                model: None,
                speaker: Some("verse"),
                audio_format: None,
                stream: false,
            })
            .unwrap_err();

        assert_eq!(
            error,
            VoiceProviderError::UnknownSpeaker {
                provider: "openai",
                speaker: "verse".to_owned(),
            }
        );
    }
}
