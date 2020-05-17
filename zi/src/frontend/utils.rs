use std::io::{self, Write};

pub(crate) struct MeteredWriter<WriterT: Write> {
    writer: WriterT,
    num_bytes_written: usize,
}

impl<WriterT: Write> MeteredWriter<WriterT> {
    pub(crate) fn new(writer: WriterT) -> Self {
        Self {
            writer,
            num_bytes_written: 0,
        }
    }

    pub(crate) fn num_bytes_written(&self) -> usize {
        self.num_bytes_written
    }
}

impl<WriterT: Write> Write for MeteredWriter<WriterT> {
    #[inline]
    fn write(&mut self, buffer: &[u8]) -> io::Result<usize> {
        let write_result = self.writer.write(buffer);
        if let Ok(num_bytes_written) = write_result.as_ref() {
            self.num_bytes_written += num_bytes_written;
        }
        write_result
    }

    #[inline]
    fn flush(&mut self) -> io::Result<()> {
        self.writer.flush()
    }
}
