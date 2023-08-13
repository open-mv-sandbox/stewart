use bytes::{Buf, BufMut, Bytes, BytesMut};

use crate::{HttpField, HttpHeader};

pub struct HttpParser {
    data: BytesMut,
    has_cr: bool,
    last_line: usize,
    lines: Vec<usize>,
}

impl Default for HttpParser {
    fn default() -> Self {
        Self {
            data: BytesMut::new(),
            has_cr: false,
            last_line: 0,
            lines: Vec::new(),
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
                        self.data.put_u8(b' ');
                    }

                    self.data.put_u8(*byte);
                    self.has_cr = false;
                }
            }
        }

        None
    }

    fn handle_line(&mut self) -> bool {
        // Extract the current line for convenience
        let line = &self.data[self.last_line..];

        // Empty line signals end of the header
        // TODO: What if an empty line happens before the first line?
        if line.is_empty() {
            return true;
        }

        // Remember that this line is available
        self.lines.push(line.len());

        // Remember where this line ended in the accumulator
        self.last_line = self.data.len();

        false
    }

    /// Take the data and return it as an assembled header.
    ///
    /// Clears all currently pending data.
    ///
    /// TODO: Handle all the cases that *must* result in 400 somehow
    fn take_header(&mut self) -> HttpHeader {
        let data = std::mem::replace(&mut self.data, BytesMut::new());
        let mut data = data.freeze();

        let mut header = HttpHeader::default();

        for (i, length) in self.lines.iter().cloned().enumerate() {
            let line = data.split_to(length);
            if i == 0 {
                continue;
            }

            let field = parse_field(line);
            header.fields.push(field);
        }

        // Clear parser state
        self.has_cr = false;
        self.last_line = 0;
        self.lines.clear();

        header
    }
}

fn parse_field(line: Bytes) -> HttpField {
    // TODO: Parse-as-we-go for this too?

    let result = line.iter().cloned().enumerate().find(|(_, ch)| *ch == b':');
    let Some((split, _)) = result else {
        // TODO: This should be a 400 error
        return HttpField::default();
    };

    let mut value = line.clone();
    let key = value.split_to(split);
    value.advance(1);

    // TODO: Do not include field whitespace

    HttpField { key, value }
}

pub enum ParserEvent {
    Header(HttpHeader),
}
