use tracing::{info, instrument};

use crate::dataset::{Dataset, Model};
use crate::errors::TransformError;
use crate::rdf::{self, NameField};
use crate::resolver::{ResolvedRecords, Resolver};


#[derive(Debug, Default, serde::Serialize, Hash, Eq, PartialEq)]
pub struct Name {
    pub entity_id: String,
    pub canonical_name: String,
    pub scientific_name: String,
    pub scientific_name_authorship: Option<String>,
}


#[instrument(skip_all)]
pub fn get_all(dataset: &Dataset) -> Result<Vec<Name>, TransformError> {
    let resolver = Resolver::new(dataset);

    let schemas = dataset.scope(&[Model::Name]);
    let schemas: Vec<&iref::Iri> = schemas.iter().map(|s| s.as_iri()).collect();

    info!("Resolving data");
    let data: ResolvedRecords<NameField> = resolver.resolve(rdf::Name::ALL, &schemas)?;


    let mut names = Vec::new();

    for (_idx, fields) in data {
        let mut name = Name::default();

        for field in fields {
            match field {
                NameField::EntityId(val) => name.entity_id = val,
                NameField::CanonicalName(val) => name.canonical_name = val,
                NameField::ScientificName(val) => name.scientific_name = val,
                NameField::ScientificNameAuthorship(val) => name.scientific_name_authorship = Some(val),
            }
        }

        names.push(name);
    }

    names.sort_by(|a, b| a.scientific_name.cmp(&b.scientific_name));
    names.dedup();

    Ok(names)
}
