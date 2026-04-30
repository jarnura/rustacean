use thiserror::Error;

#[derive(Debug, Error)]
pub enum SseError {
    #[error("SSE channel closed")]
    ChannelClosed,
}
