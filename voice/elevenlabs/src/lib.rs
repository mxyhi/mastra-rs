pub use mastra_voice_core::{
    ResolvedListenRequest, ResolvedSpeakRequest, SpeakerCatalog, StaticVoiceProvider,
    VoiceCapabilities, VoiceListenRequest, VoiceModelProfile, VoiceProviderAdapter,
    VoiceProviderError, VoiceProviderProfile, VoiceSpeakRequest, VoiceSpeakerProfile,
    VoiceTransport,
};

pub const PROFILE: VoiceProviderProfile = VoiceProviderProfile {
    id: "elevenlabs",
    display_name: "ElevenLabs Voice",
    description: "ElevenLabs speech synthesis and Scribe transcription with dynamic voice ids.",
    env_vars: &["ELEVENLABS_API_KEY"],
    speech_models: &[
        VoiceModelProfile::new("eleven_multilingual_v2", VoiceTransport::Http),
        VoiceModelProfile::new("eleven_flash_v2_5", VoiceTransport::Http),
        VoiceModelProfile::new("eleven_flash_v2", VoiceTransport::Http),
        VoiceModelProfile::new("eleven_multilingual_sts_v2", VoiceTransport::Http),
        VoiceModelProfile::new("eleven_english_sts_v2", VoiceTransport::Http),
    ],
    listening_models: &[VoiceModelProfile::new("scribe_v1", VoiceTransport::Http)],
    speaker_catalog: SpeakerCatalog::Dynamic,
    default_speaker: Some("9BWtsMINqrJLrRacOk9x"),
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
    fn accepts_dynamic_elevenlabs_speakers() {
        let resolved = provider()
            .resolve_speak(VoiceSpeakRequest {
                input: "hello",
                model: None,
                speaker: Some("EXAVITQu4vr4xnSDxMaL"),
                audio_format: Some("mp3_44100_128"),
                stream: true,
            })
            .unwrap();

        assert_eq!(resolved.speaker.as_deref(), Some("EXAVITQu4vr4xnSDxMaL"));
    }

    #[test]
    fn keeps_scribe_as_default_listening_model() {
        let resolved = provider()
            .resolve_listen(VoiceListenRequest {
                model: None,
                mime_type: Some("audio/mpeg"),
                diarize: true,
                realtime: false,
            })
            .unwrap();

        assert_eq!(resolved.model, "scribe_v1");
        assert!(resolved.diarize);
    }
}
