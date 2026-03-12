pub use mastra_voice_core::{
    ResolvedListenRequest, ResolvedSpeakRequest, SpeakerCatalog, StaticVoiceProvider,
    VoiceCapabilities, VoiceListenRequest, VoiceModelProfile, VoiceProviderAdapter,
    VoiceProviderError, VoiceProviderProfile, VoiceSpeakRequest, VoiceSpeakerProfile,
    VoiceTransport,
};

const SPEAKERS: &[VoiceSpeakerProfile] = &[
    VoiceSpeakerProfile::new("meera", "Meera"),
    VoiceSpeakerProfile::new("pavithra", "Pavithra"),
    VoiceSpeakerProfile::new("maitreyi", "Maitreyi"),
    VoiceSpeakerProfile::new("arvind", "Arvind"),
    VoiceSpeakerProfile::new("amol", "Amol"),
    VoiceSpeakerProfile::new("amartya", "Amartya"),
    VoiceSpeakerProfile::new("diya", "Diya"),
    VoiceSpeakerProfile::new("neel", "Neel"),
    VoiceSpeakerProfile::new("misha", "Misha"),
    VoiceSpeakerProfile::new("vian", "Vian"),
    VoiceSpeakerProfile::new("arjun", "Arjun"),
    VoiceSpeakerProfile::new("maya", "Maya"),
];

pub const PROFILE: VoiceProviderProfile = VoiceProviderProfile {
    id: "sarvam",
    display_name: "Sarvam Voice",
    description: "Sarvam text-to-speech and speech-to-text provider for Indian languages.",
    env_vars: &["SARVAM_API_KEY"],
    speech_models: &[VoiceModelProfile::new("bulbul:v1", VoiceTransport::Http)],
    listening_models: &[
        VoiceModelProfile::new("saarika:v2", VoiceTransport::Http),
        VoiceModelProfile::new("saarika:v1", VoiceTransport::Http),
        VoiceModelProfile::new("saarika:flash", VoiceTransport::Http),
    ],
    speaker_catalog: SpeakerCatalog::Static(SPEAKERS),
    default_speaker: Some("meera"),
    capabilities: VoiceCapabilities {
        speech_synthesis: true,
        speech_recognition: true,
        speaker_catalog: true,
        speaker_diarization: false,
        realtime_session: false,
        input_streaming: true,
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
    fn resolves_sarvam_defaults() {
        let provider = provider();
        let speak = provider
            .resolve_speak(VoiceSpeakRequest {
                input: "hello",
                model: None,
                speaker: None,
                audio_format: None,
                stream: false,
            })
            .unwrap();
        let listen = provider
            .resolve_listen(VoiceListenRequest {
                model: None,
                mime_type: Some("audio/wav"),
                diarize: false,
                realtime: false,
            })
            .unwrap();

        assert_eq!(speak.speaker.as_deref(), Some("meera"));
        assert_eq!(listen.model, "saarika:v2");
    }

    #[test]
    fn rejects_unknown_sarvam_voice() {
        let error = provider()
            .resolve_speak(VoiceSpeakRequest {
                input: "hello",
                model: None,
                speaker: Some("aria"),
                audio_format: None,
                stream: false,
            })
            .unwrap_err();

        assert_eq!(
            error,
            VoiceProviderError::UnknownSpeaker {
                provider: "sarvam",
                speaker: "aria".to_owned(),
            }
        );
    }
}
