use std::error::Error;

use crate::domain::{ConversionAggregate, ConversionId};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ConversionReport {
    pub conversion_id: ConversionId,
}

pub trait ConversionReportReader {
    type Error: Error + Send + Sync + 'static;

    /// Retrieves one persisted conversion report when it exists.
    ///
    /// # Errors
    ///
    /// Returns the adapter-specific error when report retrieval fails.
    fn read(&self, id: &ConversionId) -> Result<Option<ConversionAggregate>, Self::Error>;
}

/// Retrieves a conversion report when the underlying adapter has one.
///
/// # Errors
///
/// Propagates the adapter error returned by [`ConversionReportReader`].
pub fn handle<Reader>(
    query: ConversionReport,
    reader: &Reader,
) -> Result<Option<ConversionAggregate>, Reader::Error>
where
    Reader: ConversionReportReader,
{
    let ConversionReport { conversion_id } = query;
    reader.read(&conversion_id)
}
