use futures::{Stream, StreamExt};
use std::pin::Pin;
use std::task::{Context, Poll};
use bytes::Bytes;
use crate::types::openai::ChatCompletionChunk;

/// Transforms raw byte stream from upstream provider into typed SSE events
pub struct SseTransformer<S> {
    inner: S,
    buffer: String,
}

impl<S> SseTransformer<S> {
    pub fn new(stream: S) -> Self {
        Self { inner: stream, buffer: String::new() }
    }
}

impl<S> Stream for SseTransformer<S>
where
    S: Stream<Item = Result<Bytes, crate::provider::ProviderError>> + Unpin,
{
    type Item = Result<ChatCompletionChunk, crate::provider::ProviderError>;

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        loop {
            match self.inner.poll_next_unpin(cx) {
                Poll::Ready(Some(Ok(bytes))) => {
                    let text = String::from_utf8_lossy(&bytes);
                    self.buffer.push_str(&text);

                    // Try to extract complete SSE events
                    while let Some(line_end) = self.buffer.find('\n') {
                        let line = self.buffer[..line_end].trim().to_string();
                        self.buffer = self.buffer[line_end + 1..].to_string();

                        if line.starts_with("data: ") {
                            let data = &line[6..];
                            if data.trim() == "[DONE]" {
                                return Poll::Ready(None);
                            }
                            // Use serde_json::from_str — ChatCompletionChunk now derives Deserialize
                            if let Ok(chunk) = serde_json::from_str::<ChatCompletionChunk>(data) {
                                return Poll::Ready(Some(Ok(chunk)));
                            }
                        }
                    }
                }
                Poll::Ready(Some(Err(e))) => return Poll::Ready(Some(Err(e))),
                Poll::Ready(None) => return Poll::Ready(None),
                Poll::Pending => return Poll::Pending,
            }
        }
    }
}
