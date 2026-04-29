//! Alignment model registry for the TTS whisperX runtime.

pub(crate) const DEFAULT_LANGUAGES: &[&str] = &["zh"];

pub(crate) const ALIGN_MODELS: &[AlignModel] = &[
    AlignModel {
        language: "zh",
        repo: "jonatasgrosman/wav2vec2-large-xlsr-53-chinese-zh-cn",
    },
    AlignModel {
        language: "ja",
        repo: "jonatasgrosman/wav2vec2-large-xlsr-53-japanese",
    },
    AlignModel {
        language: "ko",
        repo: "kresnik/wav2vec2-large-xlsr-korean",
    },
    AlignModel {
        language: "en",
        repo: "jonatasgrosman/wav2vec2-large-xlsr-53-english",
    },
    AlignModel {
        language: "fr",
        repo: "jonatasgrosman/wav2vec2-large-xlsr-53-french",
    },
    AlignModel {
        language: "de",
        repo: "jonatasgrosman/wav2vec2-large-xlsr-53-german",
    },
    AlignModel {
        language: "es",
        repo: "jonatasgrosman/wav2vec2-large-xlsr-53-spanish",
    },
    AlignModel {
        language: "it",
        repo: "jonatasgrosman/wav2vec2-large-xlsr-53-italian",
    },
];

#[derive(Debug, Clone, Copy)]
pub(crate) struct AlignModel {
    pub(crate) language: &'static str,
    pub(crate) repo: &'static str,
}
