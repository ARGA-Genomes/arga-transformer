use std::fs::File;
use std::io::BufReader;

use iref::IriBuf;
use sophia::api::dataset::Dataset as DatasetTrait;
use sophia::api::graph::adapter::PartialUnionGraph;
use sophia::api::ns::Namespace;
use sophia::api::prelude::*;
use sophia::api::term::matcher::GraphNameMatcher;
use sophia::api::term::{GraphName, SimpleTerm};
use sophia::inmem::dataset::FastDataset;
use sophia::turtle::parser::trig;
use tracing::{debug, info};

use crate::errors::TransformError;
use crate::rdf::{IntoIriTerm, Literal};


/// index, field, value, source
pub type Quad = (usize, String, Literal, String);

/// index, field, value
pub type Triple = (usize, String, Literal);


pub type PartialGraph<'a> = PartialUnionGraph<&'a FastDataset, GraphIri<'a>>;


pub struct Dataset {
    // pub store: oxigraph::store::Store,
    pub source: FastDataset,
    pub map: String,
    pub schema: IriBuf,
}


impl Dataset {
    pub fn new(map_iri: &str) -> Result<Dataset, TransformError> {
        let source = FastDataset::new();
        // let store = oxigraph::store::Store::open("./triples.db").unwrap();

        Ok(Dataset {
            // store,
            source,
            map: map_iri.to_string(),
            schema: IriBuf::new(map_iri.to_string())?,
        })
    }

    pub fn scope(&self, models: &[&str]) -> Vec<String> {
        let mut iris: Vec<String> = models.iter().map(|g| format!("{}{}", self.map, g)).collect();

        // also include any source model data based on the model mapping in the schema
        for model in models {
            for iri in self.get_source_models(&model).unwrap() {
                iris.push(format!("{iri}"));
            }
        }

        iris
    }

    pub fn graph<'a>(&'a self, graphs: &'a Vec<&'a str>) -> PartialGraph<'a> {
        let selector = GraphIri(&graphs);
        self.source.partial_union_graph(selector)
    }

    /// Load a TriG turtle document.
    pub fn load_trig<R: std::io::Read>(&mut self, buf: BufReader<R>) -> Result<(), TransformError> {
        let quads = trig::parse_bufread(buf);
        self.source
            .insert_all(quads)
            .map_err(|e| TransformError::Insert(e.to_string()))?;
        Ok(())
    }

    /// Load data into the dataset.
    ///
    /// Designed to load any data source that implements a triples iterator into
    /// the dataset as a quad with the graph name derived from the `source` parameter.
    ///
    /// For example, if there was a CSV data source each row could be represented as a
    /// triple with:
    ///   subject = row index
    ///   predicate = column header
    ///   object = column value
    ///
    /// which leads to tuples such as:
    ///   (1, "scientific_name", "Felis catus Linnaeus, 1758")
    ///   (1, "genome_status", "Full")
    ///   (1, "number_of_scaffolds", 104434)
    ///
    /// The transformer will then load the tuples into the dataset after changing the
    /// predicate into an IRI within the dataset schema. It will also associate it
    /// with a graph derived from the source parameter to ensure multiple sources
    /// can be used for the same schema. This leads to quads within the RDF dataset that
    /// ultimately looks like this:
    ///
    ///   (1, http://arga.org.au/schemas/maps/bpa/scientific_name, "Felis catus Linnaeus, 1758", http://arga.org.au/source/assemblies.csv)
    ///   (1, http://arga.org.au/schemas/maps/bpa/genome_status, "Full", http://arga.org.au/source/assemblies.csv)
    ///   (1, http://arga.org.au/schemas/maps/bpa/number_of_scaffolds, 104434, http://arga.org.au/source/assemblies.csv)
    ///
    /// For bevity we omit the XSD types that are associated with the object, but rest assured they
    /// are used when determining the value within the library.
    ///
    /// An important consideration here is that this function does not care what format or structure
    /// the source is. So long as it can stream `Triple`s as an iterable it can be loaded. It is thus
    /// up to the caller to ensure that data is loaded into the RDF dataset appropriately.
    pub fn load<I, E: std::fmt::Debug>(&mut self, triples: I, source: &str) -> Result<usize, TransformError>
    where
        I: IntoIterator<Item = Result<Triple, E>>,
    {
        // get the source data namespace for all loaded data
        let source = format!("http://arga.org.au/source/{source}");
        let source = Iri::new(source).map_err(TransformError::from)?;
        let schema = Namespace::new(self.schema.as_str()).map_err(TransformError::from)?;

        let mut total = 0;
        for triple in triples {
            let (idx, header, literal) = triple.unwrap();
            let header = schema.get(&header)?;

            match literal {
                Literal::String(val) => self.source.insert(idx, header, val.as_str(), Some(&source))?,
                Literal::UInt64(val) => self.source.insert(idx, header, val as usize, Some(&source))?,
            };

            total += 1;
        }

        Ok(total)
    }

    fn get_source_models(&self, model: &str) -> Result<Vec<Iri<String>>, TransformError> {
        let base = Iri::new("http://arga.org.au/schemas/mapping/")?.to_base();
        let mapping = Namespace::new(base)?;
        let predicate = mapping.get("transforms_into")?;

        let prefix = Iri::new(self.map.as_str())?;
        let namespace = Namespace::new(prefix)?;
        let model = namespace.get(model)?;

        info!(?predicate, ?model, "getting sources");

        let mut sources = Vec::new();
        for quad in self.source.quads_matching(Any, [predicate], [model], Any) {
            let (_g, [s, _p, _o]) = quad?;
            match s {
                SimpleTerm::Iri(iri) => sources.push(Iri::new(iri.to_string())?),
                _ => {}
            };
        }

        Ok(sources)
    }

    pub fn get_source_from_model(&self, model: &iref::Iri) -> Result<Vec<iref::IriBuf>, TransformError> {
        debug!(?model, "getting source from model");

        let base = Iri::new("http://arga.org.au/schemas/mapping/")?.to_base();
        let mapping = Namespace::new(base)?;
        let predicate = mapping.get("transforms_into")?;

        let mut sources = Vec::new();
        for quad in self
            .source
            .quads_matching(Any, [predicate], [model.into_iri_term()?], Any)
        {
            let (_g, [s, _p, _o]) = quad?;
            match s {
                SimpleTerm::Iri(iri) => sources.push(iref::IriBuf::new(format!("{0}", iri.to_string()))?),
                _ => {}
            };
        }

        Ok(sources)
    }

    /// Get the triples loaded into the specified source graph.
    pub fn triples(&self, source: &str) -> Result<(), TransformError> {
        let source = format!("http://arga.org.au/source/{source}");

        for quad in self
            .source
            .quads_matching(Any, Any, Any, ExclusiveGraphIri(source.as_str()))
        {
            let (_g, [s, p, o]) = quad?;
            println!("{s:?}  {p:?}  {o:?}");
        }

        Ok(())
    }
}


#[derive(Clone, Copy)]
pub struct GraphIri<'a>(&'a Vec<&'a str>);

impl<'a> GraphNameMatcher for GraphIri<'a> {
    type Term = SimpleTerm<'static>;

    fn matches<T2: Term + ?Sized>(&self, graph_name: GraphName<&T2>) -> bool {
        match graph_name {
            // only include matching graph names
            Some(t) => match t.as_simple() {
                SimpleTerm::Iri(iri) => self.0.contains(&iri.as_str()),
                _ => false,
            },
            // always include the default graph
            None => true,
        }
    }
}


#[derive(Clone, Copy)]
pub struct ExclusiveGraphIri<'a>(&'a str);

impl<'a> GraphNameMatcher for ExclusiveGraphIri<'a> {
    type Term = SimpleTerm<'static>;

    fn matches<T2: Term + ?Sized>(&self, graph_name: GraphName<&T2>) -> bool {
        match graph_name {
            // only include matching graph names
            Some(t) => match t.as_simple() {
                SimpleTerm::Iri(iri) => self.0 == iri.as_str(),
                _ => false,
            },
            None => false,
        }
    }
}
