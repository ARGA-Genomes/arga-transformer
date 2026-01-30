use crate::dataset::Triple;
use crate::errors::ReaderError;
use crate::rdf::Literal;


/// A CSV triples reader.
///
/// This reader is a convenience wrapper for any stream that implements std::io::Read.
/// Specifically it implements an iterator that can yield `Triple`'s making it
/// compatible with the `Transformer`.
pub struct CsvReader<R: std::io::Read> {
    headers: Vec<String>,
    records: csv::StringRecordsIntoIter<R>,

    // the current line being iterated on
    current_record: Option<csv::StringRecord>,

    // because we need the row and column indices when re-entering
    // the iterator we instead track the index for the _next_ record
    // and column instead. this allows us to maintain zero-indexing
    // and keep the logic simpler in the iterator methods
    next_row: usize,
    next_column: usize,
}

impl<R: std::io::Read> CsvReader<R> {
    pub fn new(reader: R) -> Result<CsvReader<R>, ReaderError> {
        let mut reader = csv::ReaderBuilder::new().from_reader(reader);

        let headers = reader.headers()?.iter().map(|h| h.to_string()).collect();
        let records = reader.into_records();

        Ok(CsvReader {
            headers,
            records,
            next_row: 1,
            next_column: 1,
            current_record: None,
        })
    }

    // get the next column if it exists and increment the count.
    // if there aren't any columns left then reset the column state
    // and return none
    fn next_triple(&mut self) -> Option<Triple> {
        match &self.current_record {
            // no record or reached the end
            None => None,

            Some(record) => {
                // get the current index for the triple
                let current_row = self.next_row - 1;
                let current_column = self.next_column - 1;

                match record.get(current_column) {
                    Some(value) => {
                        self.next_column += 1;
                        Some((current_row, self.headers[current_column].clone(), Literal::String(value.to_string())))
                    }
                    // reached end of line
                    None => {
                        self.next_column = 1;
                        self.current_record = None;
                        None
                    }
                }
            }
        }
    }
}

/// The iterator for the CSV reader.
///
/// Transformer readers need to return triples and for a CSV file a triple
/// is considered the row index, the header, and the value. Because the `csv`
/// reader returns whole lines we implement this iterator to track the current
/// line *and* current column so that we can yield a triple for every value
/// until the document has ended.
impl<R: std::io::Read> std::iter::Iterator for CsvReader<R> {
    /// A parsed header/value combo from a CSV. It's a `Result<>` since
    /// parsing a CSV is fallible.
    type Item = Result<Triple, ReaderError>;

    fn next(&mut self) -> Option<Self::Item> {
        // still have columns left, return the next triple
        if let Some(triple) = self.next_triple() {
            return Some(Ok(triple));
        }

        // no columns left so go to the next line
        match self.records.next() {
            // we've reached the end of the document
            None => None,

            Some(result) => match result {
                // when an error occurs during parsing we want to return the error
                // and carry on to the next row.
                Err(err) => Some(Err(err.into())),

                // we've got a new line so we set it as the current record
                // and return the first triple from it
                Ok(record) => {
                    self.next_row += 1;
                    self.current_record = Some(record);
                    match self.next_triple() {
                        // this would only ever happen if there are no columns in the csv
                        // file, in which case it'd be an empty file so just return EOF
                        None => None,
                        Some(triple) => Some(Ok(triple)),
                    }
                }
            },
        }
    }
}
