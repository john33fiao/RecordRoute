//! Prompt templates for summarization

/// Base prompt template for structured summaries
pub const BASE_PROMPT: &str = r#"당신은 전문 요약가입니다. 다음 텍스트를 간결하고 구조화된 한국어 요약으로 작성합니다.

지침:
- 불렛 포인트를 사용합니다.
- 사실에만 근거합니다. 해석/추정/의견 금지.
- 섹션 제목은 다음 순서를 고정합니다:
  1) 주요 주제
  2) 핵심 내용
  3) 결정 사항
  4) 실행 항목
  5) 리스크/이슈
  6) 차기 일정

출력은 반드시 위 6개 섹션만 포함합니다."#;

/// Prompt for chunk summarization
pub fn chunk_prompt(chunk: &str) -> String {
    format!(
        "{}\n\n아래 청크를 요약하세요:\n---\n{}\n---",
        BASE_PROMPT, chunk
    )
}

/// Prompt for reduce phase (combining summaries)
pub fn reduce_prompt(summaries: &str) -> String {
    format!(
        "{}\n\n아래는 여러 청크 요약의 모음입니다. 중복을 제거하고 상충 내용을 조정하여 하나의 최종 요약으로 통합하세요:\n---\n{}\n---",
        BASE_PROMPT, summaries
    )
}

/// Prompt for one-line summary generation
pub fn one_line_prompt(summary: &str) -> String {
    format!(
        "다음 요약을 한 문장으로 압축해주세요. 가장 핵심적인 내용만 포함하세요.\n\n요약:\n{}\n\n한 줄 요약:",
        summary
    )
}
