pub use mastra_voice_core::{
    ResolvedListenRequest, ResolvedSpeakRequest, SpeakerCatalog, StaticVoiceProvider,
    VoiceCapabilities, VoiceListenRequest, VoiceModelProfile, VoiceProviderAdapter,
    VoiceProviderError, VoiceProviderProfile, VoiceSpeakRequest, VoiceSpeakerProfile,
    VoiceTransport,
};

const SPEAKERS: &[VoiceSpeakerProfile] = &[
    VoiceSpeakerProfile::new("asteria-en", "Asteria"),
    VoiceSpeakerProfile::new("luna-en", "Luna"),
    VoiceSpeakerProfile::new("stella-en", "Stella"),
    VoiceSpeakerProfile::new("athena-en", "Athena"),
    VoiceSpeakerProfile::new("hera-en", "Hera"),
    VoiceSpeakerProfile::new("orion-en", "Orion"),
    VoiceSpeakerProfile::new("arcas-en", "Arcas"),
    VoiceSpeakerProfile::new("perseus-en", "Perseus"),
    VoiceSpeakerProfile::new("angus-en", "Angus"),
    VoiceSpeakerProfile::new("orpheus-en", "Orpheus"),
    VoiceSpeakerProfile::new("helios-en", "Helios"),
    VoiceSpeakerProfile::new("zeus-en", "Zeus"),
];

pub const PROFILE: VoiceProviderProfile = VoiceProviderProfile {
    id: "deepgram",
    display_name: "Deepgram Voice",
    description: "Deepgram Aura TTS plus Nova transcription with optional speaker diarization.",
    env_vars: &["DEEPGRAM_API_KEY"],
    speech_models: &[VoiceModelProfile::new("aura", VoiceTransport::Http)],
    listening_models: &[
        VoiceModelProfile::new("nova", VoiceTransport::Http),
        VoiceModelProfile::new("nova-2", VoiceTransport::Http),
        VoiceModelProfile::new("nova-3", VoiceTransport::Http),
        VoiceModelProfile::new("whisper", VoiceTransport::Http),
        VoiceModelProfile::new("base", VoiceTransport::Http),
        VoiceModelProfile::new("enhanced", VoiceTransport::Http),
    ],
    speaker_catalog: SpeakerCatalog::Static(SPEAKERS),
    default_speaker: Some("asteria-en"),
    capabilities: VoiceCapabilities {
        speech_synthesis: true,
        speech_recognition: true,
        speaker_catalog: true,
        speaker_diarization: true,
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
    fn resolves_deepgram_defaults() {
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
                mime_type: Some("audio/wav"),
                diarize: true,
                realtime: false,
            })
            .unwrap();

        assert_eq!(speak.speaker.as_deref(), Some("asteria-en"));
        assert_eq!(listen.model, "nova");
        assert!(listen.diarize);
    }

    #[test]
    fn rejects_unknown_deepgram_voice() {
        let error = provider()
            .resolve_speak(VoiceSpeakRequest {
                input: "hello",
                model: None,
                speaker: Some("custom"),
                audio_format: None,
                stream: false,
            })
            .unwrap_err();

        assert_eq!(
            error,
            VoiceProviderError::UnknownSpeaker {
                provider: "deepgram",
                speaker: "custom".to_owned(),
            }
        );
    }
}
