pub use mastra_voice_core::{
    ResolvedListenRequest, ResolvedSpeakRequest, SpeakerCatalog, StaticVoiceProvider,
    VoiceCapabilities, VoiceListenRequest, VoiceModelProfile, VoiceProviderAdapter,
    VoiceProviderError, VoiceProviderProfile, VoiceSpeakRequest, VoiceSpeakerProfile,
    VoiceTransport,
};

const SPEAKERS: &[VoiceSpeakerProfile] = &[
    VoiceSpeakerProfile::new(
        "s3://voice-cloning-zero-shot/baf1ef41-36b6-428c-9bdf-50ba54682bd8/original/manifest.json",
        "Angelo",
    ),
    VoiceSpeakerProfile::new(
        "s3://voice-cloning-zero-shot/65977f5e-a22a-4b36-861b-ecede19bdd65/original/manifest.json",
        "Arsenio",
    ),
    VoiceSpeakerProfile::new(
        "s3://voice-cloning-zero-shot/1591b954-8760-41a9-bc58-9176a68c5726/original/manifest.json",
        "Cillian",
    ),
    VoiceSpeakerProfile::new(
        "s3://voice-cloning-zero-shot/677a4ae3-252f-476e-85ce-eeed68e85951/original/manifest.json",
        "Timo",
    ),
    VoiceSpeakerProfile::new(
        "s3://voice-cloning-zero-shot/b27bc13e-996f-4841-b584-4d35801aea98/original/manifest.json",
        "Dexter",
    ),
    VoiceSpeakerProfile::new(
        "s3://voice-cloning-zero-shot/29dd9a52-bd32-4a6e-bff1-bbb98dcc286a/original/manifest.json",
        "Miles",
    ),
    VoiceSpeakerProfile::new(
        "s3://voice-cloning-zero-shot/71cdb799-1e03-41c6-8a05-f7cd55134b0b/original/manifest.json",
        "Briggs",
    ),
    VoiceSpeakerProfile::new(
        "s3://voice-cloning-zero-shot/e040bd1b-f190-4bdb-83f0-75ef85b18f84/original/manifest.json",
        "Deedee",
    ),
    VoiceSpeakerProfile::new(
        "s3://voice-cloning-zero-shot/831bd330-85c6-4333-b2b4-10c476ea3491/original/manifest.json",
        "Nia",
    ),
    VoiceSpeakerProfile::new(
        "s3://voice-cloning-zero-shot/adb83b67-8d75-48ff-ad4d-a0840d231ef1/original/manifest.json",
        "Inara",
    ),
    VoiceSpeakerProfile::new(
        "s3://voice-cloning-zero-shot/b0aca4d7-1738-4848-a80b-307ac44a7298/original/manifest.json",
        "Constanza",
    ),
    VoiceSpeakerProfile::new(
        "s3://voice-cloning-zero-shot/5a3a1168-7793-4b2c-8f90-aff2b5232131/original/manifest.json",
        "Gideon",
    ),
    VoiceSpeakerProfile::new(
        "s3://voice-cloning-zero-shot/1bbc6986-fadf-4bd8-98aa-b86fed0476e9/original/manifest.json",
        "Casper",
    ),
    VoiceSpeakerProfile::new(
        "s3://voice-cloning-zero-shot/c14e50f2-c5e3-47d1-8c45-fa4b67803d19/original/manifest.json",
        "Mitch",
    ),
    VoiceSpeakerProfile::new(
        "s3://voice-cloning-zero-shot/50381567-ff7b-46d2-bfdc-a9584a85e08d/original/manifest.json",
        "Ava",
    ),
];

pub const PROFILE: VoiceProviderProfile = VoiceProviderProfile {
    id: "playai",
    display_name: "PlayAI Voice",
    description: "PlayAI text-to-speech provider using zero-shot voice manifests.",
    env_vars: &["PLAYAI_API_KEY", "PLAYAI_USER_ID"],
    speech_models: &[
        VoiceModelProfile::new("PlayDialog", VoiceTransport::Http),
        VoiceModelProfile::new("Play3.0-mini", VoiceTransport::Http),
    ],
    listening_models: &[],
    speaker_catalog: SpeakerCatalog::Static(SPEAKERS),
    default_speaker: Some(
        "s3://voice-cloning-zero-shot/baf1ef41-36b6-428c-9bdf-50ba54682bd8/original/manifest.json",
    ),
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

    const CILLIAN: &str =
        "s3://voice-cloning-zero-shot/1591b954-8760-41a9-bc58-9176a68c5726/original/manifest.json";

    #[test]
    fn validates_known_playai_voice_manifests() {
        let resolved = provider()
            .resolve_speak(VoiceSpeakRequest {
                input: "hello",
                model: None,
                speaker: Some(CILLIAN),
                audio_format: None,
                stream: true,
            })
            .unwrap();

        assert_eq!(resolved.speaker.as_deref(), Some(CILLIAN));
    }

    #[test]
    fn rejects_playai_listening() {
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
                provider: "playai",
                capability: "speech recognition",
            }
        );
    }
}
