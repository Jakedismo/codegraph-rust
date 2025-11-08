use std::sync::Arc;

/// Simple chunker that splits text into token-limited chunks using a provided counter.
pub struct Chunker {
    chunk_size: usize,
    token_counter: Arc<dyn Fn(&str) -> usize + Send + Sync>,
}

impl Clone for Chunker {
    fn clone(&self) -> Self {
        Self {
            chunk_size: self.chunk_size,
            token_counter: Arc::clone(&self.token_counter),
        }
    }
}

impl Chunker {
    /// Create a chunker with the given maximum tokens per chunk.
    /// The token counter should match the tokenizer used by the downstream model.
    pub fn new<F>(chunk_size: usize, token_counter: F) -> Self
    where
        F: Fn(&str) -> usize + Send + Sync + 'static,
    {
        let chunk_size = chunk_size.max(1);
        Self {
            chunk_size,
            token_counter: Arc::new(token_counter),
        }
    }

    /// Split `text` into chunks whose token counts never exceed the configured limit.
    pub fn chunk_text(&self, text: &str) -> Vec<String> {
        if text.is_empty() {
            return Vec::new();
        }

        let mut chunks = Vec::new();
        let mut current = String::new();
        let counter = &self.token_counter;

        for ch in text.chars() {
            current.push(ch);
            if counter(&current) > self.chunk_size {
                // Remove the character that caused the overflow and flush the chunk.
                current.pop();
                if !current.is_empty() {
                    chunks.push(current.clone());
                    current.clear();
                }
                current.push(ch);
            }
        }

        if !current.is_empty() {
            chunks.push(current);
        }

        chunks
    }
}
