use bytes::{Buf, BufMut, Bytes, BytesMut};

pub struct HttpParser {
    accumulator: BytesMut,
    has_cr: bool,
    last_line: usize,
}

impl Default for HttpParser {
    fn default() -> Self {
        Self {
            accumulator: BytesMut::new(),
            has_cr: false,
            last_line: 0,
        }
    }
}

impl HttpParser {
    /// Consume bytes into the parser.
    ///
    /// Suspends and returns if new parsed data is available, trimming `bytes` of the consumed
    /// data.
    pub fn consume(&mut self, bytes: &mut Bytes) -> Option<ParserEvent> {
        // TODO: We need to unit test data consumption and requests split in weird locations.

        for (i, byte) in bytes.iter().enumerate() {
            match *byte {
                // CRNL is the required newline, so consume CR if it happens
                b'\r' => {
                    self.has_cr = true;
                }
                b'\n' => {
                    // Standalone \n *MAY* be accepted, but I don't think we should, since it's
                    // technically a malformed request.
                    // For now we do handle it the same.
                    let header_done = self.handle_line();
                    self.has_cr = false;

                    // Check if we need to suspend with new data
                    if header_done {
                        bytes.advance(i + 1);

                        let header = self.take_header();
                        return Some(ParserEvent::Header(header));
                    }
                }
                _ => {
                    // CR with no NL needs to be counted as a space
                    if self.has_cr {
                        self.accumulator.put_u8(b' ');
                    }

                    self.accumulator.put_u8(*byte);
                    self.has_cr = false;
                }
            }
        }

        None
    }

    fn handle_line(&mut self) -> bool {
        let line = &self.accumulator[self.last_line..];

        // Empty line signals end of the header
        if line.is_empty() {
            return true;
        }

        // Remember where this line ended in the accumulator
        self.last_line = self.accumulator.len();

        false
    }

    fn take_header(&mut self) -> Bytes {
        let data = std::mem::replace(&mut self.accumulator, BytesMut::new());
        self.last_line = 0;

        data.freeze()
    }
}

pub enum ParserEvent {
    Header(Bytes),
}
