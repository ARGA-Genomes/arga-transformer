use std::collections::HashMap;
use std::io::BufReader;

use iref::IriBuf;
use iref::iri::Segment;
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
use crate::rdf::{DataTypes, IntoIriTerm, Literal};


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


#[derive(PartialEq, Eq, Debug)]
pub enum DatasetTerm {
    Iri(IriBuf),
    Literal(crate::rdf::Literal),
}


impl sophia::api::term::Term for DatasetTerm {
    type BorrowTerm<'x> = &'x Self;

    fn kind(&self) -> TermKind {
        match self {
            DatasetTerm::Iri(_iri_buf) => TermKind::Iri,
            DatasetTerm::Literal(_literal) => TermKind::Literal,
        }
    }

    fn borrow_term(&self) -> Self::BorrowTerm<'_> {
        self
    }

    #[inline]
    fn iri(&self) -> Option<IriRef<sophia::api::MownStr>> {
        match self {
            DatasetTerm::Iri(iri_buf) => {
                // it's tempting to return None if the IRI isn't valid according to sophia,
                // but we are kind of expecting the IRI to be valid upon creation so we opt
                // for a panic if it ends up invalid. This should only happen if iref and
                // sophia are implementing a different standard or one has a buggy implementation
                let str_ref = sophia::api::MownStr::from_ref(iri_buf.as_str());
                let iri_ref = IriRef::new(str_ref).expect("IRI is incompatible with sophia IRIs");
                Some(iri_ref)
            }
            _ => None,
        }
    }
}


impl sophia::api::term::matcher::TermMatcher for DatasetTerm {
    type Term = DatasetTerm;

    fn matches<T2: Term + ?Sized>(&self, term: &T2) -> bool {
        term.eq(self)
    }
}


impl DatasetTerm {
    fn into_sophia_term(&self) -> Result<SimpleTerm, TransformError> {
        // this is a litle awkward as SimpleTerm doesn't have a representation for Any which
        // we have conflated with the DatasetTerm for convenience. so we make the return optional
        // and fallback to any if we can't convert into a simple term.
        let term = match self {
            DatasetTerm::Iri(iri_buf) => iri_buf.into_iri_term()?,
            DatasetTerm::Literal(literal) => match literal {
                Literal::String(val) => SimpleTerm::LiteralDatatype(val.clone().into(), DataTypes::String.try_into()?),
                Literal::UInt64(val) => {
                    SimpleTerm::LiteralDatatype(val.to_string().into(), DataTypes::Integer.try_into()?)
                }
            },
        };

        Ok(term)
    }
}


pub enum Model {
    Agent,
    Annotation,
    Assembly,
    Collecting,
    DataProduct,
    Deposition,
    Extraction,
    Library,
    Name,
    Organism,
    ProjectMember,
    Project,
    Publication,
    SequencingRun,
    Subsample,
    Tissue,
}


trait ToIriSegment {
    fn to_iri_segment(&self) -> &iref::iri::Segment;
}

impl ToIriSegment for Model {
    fn to_iri_segment(&self) -> &iref::iri::Segment {
        let segment = match self {
            Model::Agent => "agent",
            Model::Annotation => "annotation",
            Model::Assembly => "assembly",
            Model::Collecting => "collecting",
            Model::DataProduct => "data_products",
            Model::Deposition => "depositions",
            Model::Extraction => "extractions",
            Model::Library => "library",
            Model::Name => "names",
            Model::Organism => "organisms",
            Model::ProjectMember => "project_member",
            Model::Project => "projecct",
            Model::Publication => "publication",
            Model::SequencingRun => "sequencing_runs",
            Model::Subsample => "subsamples",
            Model::Tissue => "tissues",
        };

        // we panic here if the segment isn't valid as these are hardcoded values
        // and should never throw a spanner in the works.
        iref::iri::Segment::new(segment).expect("model segment is not valid")
    }
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

    pub fn model_schema(&self, model: &Model) -> iref::IriBuf {
        let mut iri = self.schema.clone();
        iri.path_mut().push(model.to_iri_segment());
        iri
    }

    /// Finds the schema IRIs for the source dataset.
    ///
    /// Each model represents the target transformation of a dataset. This means we need
    /// to scope our transformations to the source datasets (loaded in as triples), the core
    /// mapping/transform schema (describing the DSL used for transformation), and the final
    /// target schema. This method only returns the IRIs related to the source datasets.
    /// Because the `transforms_into` directive supports transforming multiple sources into
    /// one target, as well as one source into multiple targets, we need to include all possible
    /// schema IRIs that can potentially be used and return it as an array of IRIs.
    fn source_schema(&self, model: &Model) -> Result<Vec<IriBuf>, TransformError> {
        let predicate: &iref::Iri = crate::rdf::Source::TransformsInto.as_ref();
        let object = self.model_schema(model);

        info!(?predicate, ?object, "Getting sources");

        let mut sources = Vec::new();
        for quad in self
            .source
            .quads_matching(Any, [predicate.into_iri_term()?], [DatasetTerm::Iri(object)], Any)
        {
            let (_g, [s, _p, _o]) = quad?;
            match s {
                SimpleTerm::Iri(iri) => sources.push(iref::IriBuf::new(iri.to_string())?),
                _ => {}
            };
        }

        Ok(sources)
    }

    /// Returns a list of IRIs associated with the provided models.
    ///
    /// Models are the link between the transformed common ARGA model and
    /// the source data. A source schema defines a transformation by
    /// using the `transforms_into` directive which associates a source document
    /// (such as `assemblies.csv`) to a final model.
    ///
    /// Transformation happens when data is 'pulled' from the underlying triples
    /// store and as such we need to scope queries to both the common ARGA model
    /// and all source models that contribute to the final output. This function
    /// does exactly that by including all sources and all models relevant to the
    /// list of model names specified.
    pub fn scope(&self, models: &[Model]) -> Vec<iref::IriBuf> {
        let mut scope = Vec::new();

        // include all model schemas as they are the target transformation
        for model in models {
            scope.push(self.model_schema(model));
        }

        // include any source model data based on the model mapping in the schema
        for model in models {
            let schemas = self.source_schema(model).unwrap();
            scope.extend(schemas);
        }

        scope
    }

    pub fn quads_matching(&self, s: DatasetTerm, p: DatasetTerm, o: DatasetTerm, g: &iref::Iri) {
        self.source.quads_matching(s, p, o, GraphIriName(g));
    }

    // pub fn scope(&self, models: &[&str]) -> Vec<String> {
    //     let mut iris: Vec<String> = models.iter().map(|g| format!("{}{}", self.map, g)).collect();

    //     // also include any source model data based on the model mapping in the schema
    //     for model in models {
    //         for iri in self.get_source_models(&model).unwrap() {
    //             iris.push(format!("{iri}"));
    //         }
    //     }

    //     iris
    // }

    // pub fn graph<'a>(&'a self, graphs: &'a Vec<&'a str>) -> PartialGraph<'a> {
    //     let selector = GraphIri(&graphs);
    //     self.source.partial_union_graph(selector)
    // }

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
        let mut base = iref::IriBuf::new("http://arga.org.au/source".to_string())?;
        base.path_mut().push(Segment::new(source).unwrap());

        // instead of recreating the header iri for each record we store it cache
        let mut header_cache = HashMap::new();

        let mut total = 0;
        for triple in triples {
            let (idx, header, literal) = triple.unwrap();

            // get the header iri if it exists. if not create one and store it in the cache
            let header_iri = header_cache.entry(header).or_insert_with_key(|header| {
                let mut iri = self.schema.clone();
                // sanitise the header to make sure it only has valid characters
                let header = header.replace("#", "");
                iri.path_mut().push(Segment::new(&header).unwrap());
                iri
            });

            match literal {
                Literal::String(val) => {
                    self.source
                        .insert(idx, header_iri.into_iri_term()?, val.as_str(), Some(&base.into_iri_term()?))?
                }
                Literal::UInt64(val) => {
                    self.source
                        .insert(idx, header_iri.into_iri_term()?, val as usize, Some(&base.into_iri_term()?))?
                }
            };

            total += 1;
        }

        Ok(total)
    }

    // fn get_source_models(&self, model: &str) -> Result<Vec<Iri<String>>, TransformError> {
    //     let base = Iri::new("http://arga.org.au/schemas/mapping/")?.to_base();
    //     let mapping = Namespace::new(base)?;
    //     let predicate = mapping.get("transforms_into")?;

    //     let prefix = Iri::new(self.map.as_str())?;
    //     let namespace = Namespace::new(prefix)?;
    //     let model = namespace.get(model)?;

    //     info!(?predicate, ?model, "getting sources");

    //     let mut sources = Vec::new();
    //     for quad in self.source.quads_matching(Any, [predicate], [model], Any) {
    //         let (_g, [s, _p, _o]) = quad?;
    //         match s {
    //             SimpleTerm::Iri(iri) => sources.push(Iri::new(iri.to_string())?),
    //             _ => {}
    //         };
    //     }

    //     Ok(sources)
    // }

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

    pub fn dump_triples(&self) {
        for quad in self.source.quads() {
            let (g, [s, p, o]) = quad.unwrap();
            println!("{}  {}  {}  {g:?}", stringify_term(s), stringify_term(p), stringify_term(o));
        }
    }
}


fn stringify_term(term: &SimpleTerm) -> String {
    match term {
        SimpleTerm::Iri(iri_ref) => iri_ref.to_string(),
        SimpleTerm::BlankNode(bnode_id) => bnode_id.to_string(),
        SimpleTerm::LiteralDatatype(mown_str, _iri_ref) => mown_str.to_string(),
        SimpleTerm::LiteralLanguage(mown_str, _language_tag) => mown_str.to_string(),
        SimpleTerm::Triple(triple) => format!("{triple:?}"),
        SimpleTerm::Variable(var_name) => var_name.to_string(),
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


#[derive(Clone, Copy)]
pub struct GraphIriName<'a>(&'a iref::Iri);

impl<'a> GraphNameMatcher for GraphIriName<'a> {
    type Term = SimpleTerm<'static>;

    fn matches<T2: Term + ?Sized>(&self, graph_name: GraphName<&T2>) -> bool {
        match graph_name {
            // only include matching graph names
            Some(t) => match t.as_simple() {
                SimpleTerm::Iri(iri) => self.0.eq(&iri.as_str()),
                _ => false,
            },
            // always include the default graph
            None => false,
        }
    }
}
