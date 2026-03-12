pub use mastra_voice_core::{
    ResolvedListenRequest, ResolvedSpeakRequest, SpeakerCatalog, StaticVoiceProvider,
    VoiceCapabilities, VoiceListenRequest, VoiceModelProfile, VoiceProviderAdapter,
    VoiceProviderError, VoiceProviderProfile, VoiceSpeakRequest, VoiceSpeakerProfile,
    VoiceTransport,
};

pub const PROFILE: VoiceProviderProfile = VoiceProviderProfile {
    id: "cloudflare",
    display_name: "Cloudflare Workers AI Voice",
    description: "Cloudflare Workers AI transcription provider with REST and Worker binding support.",
    env_vars: &["CLOUDFLARE_AI_API_KEY", "CLOUDFLARE_ACCOUNT_ID"],
    speech_models: &[],
    listening_models: &[
        VoiceModelProfile::new("@cf/openai/whisper-large-v3-turbo", VoiceTransport::Http),
        VoiceModelProfile::new("@cf/openai/whisper", VoiceTransport::Http),
        VoiceModelProfile::new("@cf/openai/whisper-tiny-en", VoiceTransport::Http),
    ],
    speaker_catalog: SpeakerCatalog::None,
    default_speaker: None,
    capabilities: VoiceCapabilities {
        speech_synthesis: false,
        speech_recognition: true,
        speaker_catalog: false,
        speaker_diarization: false,
        realtime_session: false,
        input_streaming: true,
        output_streaming: false,
        tool_calling: false,
        worker_binding: true,
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
    fn exposes_listen_only_worker_binding_profile() {
        let resolved = provider()
            .resolve_listen(VoiceListenRequest {
                model: None,
                mime_type: Some("audio/wav"),
                diarize: false,
                realtime: false,
            })
            .unwrap();

        assert_eq!(resolved.model, "@cf/openai/whisper-large-v3-turbo");
        assert!(profile().capabilities.worker_binding);
    }

    #[test]
    fn rejects_speech_synthesis_for_cloudflare() {
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
                provider: "cloudflare",
                capability: "speech synthesis",
            }
        );
    }
}
