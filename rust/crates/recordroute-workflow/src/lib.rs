#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WorkflowStep {
    Stt,
    Correct,
    Summary,
    Embedding,
    Diarize,
}
