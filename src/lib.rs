pub mod dataset;
pub mod errors;
// pub mod models;
pub mod rdf;
pub mod readers;
// pub mod resolver;


use std::io::BufReader;

use dataset::{Dataset, Quad, Triple};
use errors::TransformError;
use tracing::debug;


mod ttl {
    pub const ARGA_PROJECTS: &[u8] = include_bytes!("../schemas/arga_projects.ttl");
    pub const ARGA_TSI: &[u8] = include_bytes!("../schemas/arga_tsi.ttl");
    pub const BIOPLATFORMS: &[u8] = include_bytes!("../schemas/bioplatforms.ttl");
    pub const DNAZOO: &[u8] = include_bytes!("../schemas/dnazoo.ttl");
    pub const NCBI_TAXONOMY: &[u8] = include_bytes!("../schemas/ncbi_taxonomy.ttl");
    pub const NCBI_BIOSAMPLES: &[u8] = include_bytes!("../schemas/ncbi_biosamples.ttl");
    pub const NCBI_GENBANK: &[u8] = include_bytes!("../schemas/ncbi_genbank.ttl");
    pub const NCBI_REPORTS: &[u8] = include_bytes!("../schemas/ncbi_reports.ttl");
}


/// Transforms datasets by loading them into an RDF store.
///
/// The process of transformation with RDF goes beyond basic mapping. When a dataset
/// is loaded it is broken up into quads usually in the form of `(index, field_name, value, source)`.
/// - `index`: internally generated as a simple record number to associate multiple fields with on record.
/// - `field_name`: an IRI that uniquely identifies the datum within the scope of a single record.
/// - `value`: the datum. Very often a string but could be any primitive.
/// - `source`: an IRI that identifies this particular dataset. Used to determine possible transformations.
///
/// By having this extra level of indirection we allow for complicated transforms by leveraging
/// a kind of DSL to process, sanitize, combine, or even run arbitrary logic to one or more datums in one
/// or more datasets. For example, it's possible to load a dataset along with a 'lookup' dataset and
/// inject a generated field that stores the lookup alongside the dataset.
pub struct Transformer {
    dataset: Dataset,
}

impl Transformer {
    /// Initialise the transformer and it's underlying RDF store.
    ///
    /// This will also load the mapping files defined in the `schemas` subrepo
    /// of which are included in the compiled binary.
    pub fn new(schema: &str) -> Result<Transformer, TransformError> {
        let mut dataset = Dataset::new(schema)?;

        // load the mapping definitions
        dataset.load_trig(BufReader::new(ttl::ARGA_PROJECTS))?;
        // dataset.load_trig(BufReader::new(ttl::ARGA_TSI))?;
        // dataset.load_trig(BufReader::new(ttl::BIOPLATFORMS))?;
        dataset.load_trig(BufReader::new(ttl::DNAZOO))?;
        dataset.load_trig(BufReader::new(ttl::NCBI_TAXONOMY))?;
        // dataset.load_trig(BufReader::new(ttl::NCBI_BIOSAMPLES))?;
        dataset.load_trig(BufReader::new(ttl::NCBI_GENBANK))?;
        dataset.load_trig(BufReader::new(ttl::NCBI_REPORTS))?;

        Ok(Transformer { dataset })
    }

    /// Initialise the transformer and it's underlying RDF store.
    ///
    /// This will also load the mapping files defined in the `schemas` subrepo
    /// of which are included in the compiled binary.
    pub fn load<I, E: std::fmt::Debug>(&mut self, triples: I, source: &str) -> Result<usize, TransformError>
    where
        I: IntoIterator<Item = Result<Triple, E>>,
    {
        debug!(%self.dataset.schema, source, "loading dataset quads");
        self.dataset.load(triples, source)
    }

    pub fn transform() {}

    /// Get the triples loaded into the specified source graph.
    pub fn triples(&self, source: &str) -> Result<(), TransformError> {
        self.dataset.triples(source)
    }
}
