use thiserror::Error;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VoiceTransport {
    Http,
    Websocket,
    WorkerBinding,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct VoiceModelProfile {
    pub name: &'static str,
    pub transport: VoiceTransport,
}

impl VoiceModelProfile {
    pub const fn new(name: &'static str, transport: VoiceTransport) -> Self {
        Self { name, transport }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct VoiceSpeakerProfile {
    pub voice_id: &'static str,
    pub label: &'static str,
}

impl VoiceSpeakerProfile {
    pub const fn new(voice_id: &'static str, label: &'static str) -> Self {
        Self { voice_id, label }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SpeakerCatalog {
    None,
    Static(&'static [VoiceSpeakerProfile]),
    Dynamic,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct VoiceCapabilities {
    pub speech_synthesis: bool,
    pub speech_recognition: bool,
    pub speaker_catalog: bool,
    pub speaker_diarization: bool,
    pub realtime_session: bool,
    pub input_streaming: bool,
    pub output_streaming: bool,
    pub tool_calling: bool,
    pub worker_binding: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct VoiceProviderProfile {
    pub id: &'static str,
    pub display_name: &'static str,
    pub description: &'static str,
    pub env_vars: &'static [&'static str],
    pub speech_models: &'static [VoiceModelProfile],
    pub listening_models: &'static [VoiceModelProfile],
    pub speaker_catalog: SpeakerCatalog,
    pub default_speaker: Option<&'static str>,
    pub capabilities: VoiceCapabilities,
}

impl VoiceProviderProfile {
    pub fn default_speech_model(&self) -> Option<&'static VoiceModelProfile> {
        self.speech_models.first()
    }

    pub fn default_listening_model(&self) -> Option<&'static VoiceModelProfile> {
        self.listening_models.first()
    }

    pub fn static_speakers(&self) -> &'static [VoiceSpeakerProfile] {
        match self.speaker_catalog {
            SpeakerCatalog::Static(speakers) => speakers,
            SpeakerCatalog::None | SpeakerCatalog::Dynamic => &[],
        }
    }

    fn find_speech_model(&self, name: Option<&str>) -> Option<&'static VoiceModelProfile> {
        match name {
            Some(name) => self.speech_models.iter().find(|model| model.name == name),
            None => self.default_speech_model(),
        }
    }

    fn find_listening_model(&self, name: Option<&str>) -> Option<&'static VoiceModelProfile> {
        match name {
            Some(name) => self
                .listening_models
                .iter()
                .find(|model| model.name == name),
            None => self.default_listening_model(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VoiceSpeakRequest<'a> {
    pub input: &'a str,
    pub model: Option<&'a str>,
    pub speaker: Option<&'a str>,
    pub audio_format: Option<&'a str>,
    pub stream: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedSpeakRequest {
    pub provider_id: &'static str,
    pub model: &'static str,
    pub speaker: Option<String>,
    pub audio_format: Option<String>,
    pub input: String,
    pub transport: VoiceTransport,
    pub stream: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VoiceListenRequest<'a> {
    pub model: Option<&'a str>,
    pub mime_type: Option<&'a str>,
    pub diarize: bool,
    pub realtime: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedListenRequest {
    pub provider_id: &'static str,
    pub model: &'static str,
    pub mime_type: Option<String>,
    pub diarize: bool,
    pub realtime: bool,
    pub transport: VoiceTransport,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum VoiceProviderError {
    #[error("{provider} does not support {capability}")]
    CapabilityUnavailable {
        provider: &'static str,
        capability: &'static str,
    },
    #[error("{provider} does not expose a {mode} model named '{name}'")]
    UnknownModel {
        provider: &'static str,
        mode: &'static str,
        name: String,
    },
    #[error("{provider} does not recognize speaker '{speaker}'")]
    UnknownSpeaker {
        provider: &'static str,
        speaker: String,
    },
    #[error("speech input cannot be empty")]
    EmptyInput,
}

pub trait VoiceProviderAdapter {
    fn profile(&self) -> &'static VoiceProviderProfile;

    fn resolve_speak(
        &self,
        request: VoiceSpeakRequest<'_>,
    ) -> Result<ResolvedSpeakRequest, VoiceProviderError> {
        let profile = self.profile();
        if !profile.capabilities.speech_synthesis {
            return Err(VoiceProviderError::CapabilityUnavailable {
                provider: profile.id,
                capability: "speech synthesis",
            });
        }
        if request.input.trim().is_empty() {
            return Err(VoiceProviderError::EmptyInput);
        }

        let model = profile.find_speech_model(request.model).ok_or_else(|| {
            VoiceProviderError::UnknownModel {
                provider: profile.id,
                mode: "speech",
                name: request.model.unwrap_or("<default>").to_owned(),
            }
        })?;

        let speaker = match request.speaker.or(profile.default_speaker) {
            Some(speaker) => match profile.speaker_catalog {
                SpeakerCatalog::None => {
                    return Err(VoiceProviderError::CapabilityUnavailable {
                        provider: profile.id,
                        capability: "speaker selection",
                    });
                }
                SpeakerCatalog::Dynamic => Some(speaker.to_owned()),
                SpeakerCatalog::Static(speakers) => {
                    if speakers
                        .iter()
                        .any(|candidate| candidate.voice_id == speaker)
                    {
                        Some(speaker.to_owned())
                    } else {
                        return Err(VoiceProviderError::UnknownSpeaker {
                            provider: profile.id,
                            speaker: speaker.to_owned(),
                        });
                    }
                }
            },
            None => None,
        };

        Ok(ResolvedSpeakRequest {
            provider_id: profile.id,
            model: model.name,
            speaker,
            audio_format: request.audio_format.map(str::to_owned),
            input: request.input.to_owned(),
            transport: model.transport,
            stream: request.stream,
        })
    }

    fn resolve_listen(
        &self,
        request: VoiceListenRequest<'_>,
    ) -> Result<ResolvedListenRequest, VoiceProviderError> {
        let profile = self.profile();
        if !profile.capabilities.speech_recognition {
            return Err(VoiceProviderError::CapabilityUnavailable {
                provider: profile.id,
                capability: "speech recognition",
            });
        }
        if request.diarize && !profile.capabilities.speaker_diarization {
            return Err(VoiceProviderError::CapabilityUnavailable {
                provider: profile.id,
                capability: "speaker diarization",
            });
        }
        if request.realtime && !profile.capabilities.realtime_session {
            return Err(VoiceProviderError::CapabilityUnavailable {
                provider: profile.id,
                capability: "realtime listening",
            });
        }

        let model = profile.find_listening_model(request.model).ok_or_else(|| {
            VoiceProviderError::UnknownModel {
                provider: profile.id,
                mode: "listening",
                name: request.model.unwrap_or("<default>").to_owned(),
            }
        })?;

        Ok(ResolvedListenRequest {
            provider_id: profile.id,
            model: model.name,
            mime_type: request.mime_type.map(str::to_owned),
            diarize: request.diarize,
            realtime: request.realtime,
            transport: model.transport,
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StaticVoiceProvider {
    profile: &'static VoiceProviderProfile,
}

impl StaticVoiceProvider {
    pub const fn new(profile: &'static VoiceProviderProfile) -> Self {
        Self { profile }
    }
}

impl VoiceProviderAdapter for StaticVoiceProvider {
    fn profile(&self) -> &'static VoiceProviderProfile {
        self.profile
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SPEAKERS: &[VoiceSpeakerProfile] = &[VoiceSpeakerProfile::new("alloy", "Alloy")];
    const PROFILE: VoiceProviderProfile = VoiceProviderProfile {
        id: "openai",
        display_name: "OpenAI",
        description: "Test profile",
        env_vars: &["OPENAI_API_KEY"],
        speech_models: &[VoiceModelProfile::new("tts-1", VoiceTransport::Http)],
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

    const DYNAMIC_PROFILE: VoiceProviderProfile = VoiceProviderProfile {
        id: "elevenlabs",
        display_name: "ElevenLabs",
        description: "Dynamic speaker catalog",
        env_vars: &["ELEVENLABS_API_KEY"],
        speech_models: &[VoiceModelProfile::new(
            "eleven_multilingual_v2",
            VoiceTransport::Http,
        )],
        listening_models: &[VoiceModelProfile::new("scribe_v1", VoiceTransport::Http)],
        speaker_catalog: SpeakerCatalog::Dynamic,
        default_speaker: None,
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

    #[test]
    fn resolves_defaults_from_profile() {
        let provider = StaticVoiceProvider::new(&PROFILE);
        let resolved = provider
            .resolve_speak(VoiceSpeakRequest {
                input: "hello",
                model: None,
                speaker: None,
                audio_format: Some("mp3"),
                stream: true,
            })
            .unwrap();

        assert_eq!(resolved.model, "tts-1");
        assert_eq!(resolved.speaker.as_deref(), Some("alloy"));
        assert_eq!(resolved.transport, VoiceTransport::Http);
    }

    #[test]
    fn rejects_unknown_static_speakers() {
        let provider = StaticVoiceProvider::new(&PROFILE);
        let error = provider
            .resolve_speak(VoiceSpeakRequest {
                input: "hello",
                model: None,
                speaker: Some("echo"),
                audio_format: None,
                stream: false,
            })
            .unwrap_err();

        assert_eq!(
            error,
            VoiceProviderError::UnknownSpeaker {
                provider: "openai",
                speaker: "echo".to_owned(),
            }
        );
    }

    #[test]
    fn allows_dynamic_speakers_and_diarization() {
        let provider = StaticVoiceProvider::new(&DYNAMIC_PROFILE);
        let speak = provider
            .resolve_speak(VoiceSpeakRequest {
                input: "hello",
                model: None,
                speaker: Some("custom"),
                audio_format: None,
                stream: true,
            })
            .unwrap();
        let listen = provider
            .resolve_listen(VoiceListenRequest {
                model: None,
                mime_type: Some("audio/mpeg"),
                diarize: true,
                realtime: false,
            })
            .unwrap();

        assert_eq!(speak.speaker.as_deref(), Some("custom"));
        assert!(listen.diarize);
    }
}
