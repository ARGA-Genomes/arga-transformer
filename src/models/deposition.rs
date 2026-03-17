use tracing::{info, instrument};

use crate::dataset::{Dataset, Model};
use crate::errors::TransformError;
use crate::rdf::{self, DepositionField};
use crate::resolver::{ResolvedRecords, Resolver};


#[derive(Debug, Default, serde::Serialize)]
pub struct Deposition {
    pub entity_id: String,
    pub assembly_id: Option<String>,

    pub event_date: Option<String>,
    pub url: Option<String>,
    pub institution: Option<String>,
}


#[instrument(skip_all)]
pub fn get_all(dataset: &Dataset) -> Result<Vec<Deposition>, TransformError> {
    let resolver = Resolver::new(dataset);

    let schemas = dataset.scope(&[Model::Deposition]);
    let schemas: Vec<&iref::Iri> = schemas.iter().map(|s| s.as_iri()).collect();

    info!("Resolving data");
    let data: ResolvedRecords<DepositionField> = resolver.resolve(rdf::Deposition::ALL, &schemas)?;


    let mut depositions = Vec::new();

    for (_idx, fields) in data {
        let mut deposition = Deposition::default();

        for field in fields {
            match field {
                DepositionField::EntityId(val) => deposition.entity_id = val,
                DepositionField::AssemblyId(val) => deposition.assembly_id = Some(val),
                DepositionField::EventDate(val) => deposition.event_date = Some(val),
                DepositionField::Url(val) => deposition.url = Some(val),
                DepositionField::Institution(val) => deposition.institution = Some(val),
            }
        }

        depositions.push(deposition);
    }

    Ok(depositions)
}
