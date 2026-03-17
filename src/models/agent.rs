use tracing::{info, instrument};

use crate::dataset::{Dataset, Model};
use crate::errors::TransformError;
use crate::rdf::{self, AnnotationField};
use crate::resolver::{ResolvedRecords, Resolver};


#[derive(Debug, Default, serde::Serialize, Hash, Eq, PartialEq)]
pub struct Agent {
    pub entity_id: String,
    pub full_name: String,
    pub orcid: Option<String>,
}


pub fn get_all(dataset: &Dataset) -> Result<Vec<Agent>, TransformError> {
    let mut agents = get_custodian_agents(dataset)?;
    agents.extend(get_extraction_agents(dataset)?);
    agents.extend(get_material_extraction_agents(dataset)?);
    agents.extend(get_prepared_agents(dataset)?);
    agents.sort_by(|a, b| a.entity_id.cmp(&b.entity_id));
    agents.dedup();
    Ok(agents)
}


pub fn get_custodian_agents(dataset: &Dataset) -> Result<Vec<Agent>, TransformError> {
    let resolver = Resolver::new(dataset);

    let schemas = dataset.scope(&[Model::Agent]);
    let schemas: Vec<&iref::Iri> = schemas.iter().map(|s| s.as_iri()).collect();

    info!("Resolving data");
    let data: ResolvedRecords<AgentField> = resolver.resolve(rdf::Agent::ALL, &schemas)?;


    let mut agents = Vec::new();
    for (_idx, fields) in data {
        let mut agent = Agent::default();

        for field in fields {
            match field {
                DataProductField::Custodian(val) => agent.full_name = val,
                DataProductField::CustodianOrcid(val) => agent.orcid = Some(val),
                DataProductField::CustodianEntityId(val) => agent.entity_id = val,
                _ => {}
            }
        }

        agents.push(agent);
    }

    Ok(agents)
}


pub fn get_extraction_agents(dataset: &Dataset) -> Result<Vec<Agent>, TransformError> {
    let models = dataset.scope(&["extractions"]);
    let mut scope = Vec::new();
    for model in models.iter() {
        scope.push(iref::Iri::new(model).unwrap());
    }

    let resolver = Resolver::new(dataset);

    let data: HashMap<Literal, Vec<ExtractionField>> = resolver.resolve(
        &[
            Extraction::ExtractedBy,
            Extraction::ExtractedByOrcid,
            Extraction::ExtractedByEntityId,
        ],
        &scope,
    )?;

    let mut agents = Vec::new();
    for (_idx, fields) in data {
        let mut agent = Agent::default();

        for field in fields {
            match field {
                ExtractionField::ExtractedBy(val) => agent.full_name = val,
                ExtractionField::ExtractedByOrcid(val) => agent.orcid = Some(val),
                ExtractionField::ExtractedByEntityId(val) => agent.entity_id = val,
                _ => {}
            }
        }

        agents.push(agent);
    }

    Ok(agents)
}


pub fn get_material_extraction_agents(dataset: &Dataset) -> Result<Vec<Agent>, TransformError> {
    let models = dataset.scope(&["extractions"]);
    let mut scope = Vec::new();
    for model in models.iter() {
        scope.push(iref::Iri::new(model).unwrap());
    }

    let resolver = Resolver::new(dataset);

    let data: HashMap<Literal, Vec<ExtractionField>> = resolver.resolve(
        &[
            Extraction::MaterialExtractedBy,
            Extraction::MaterialExtractedByOrcid,
            Extraction::MaterialExtractedByEntityId,
        ],
        &scope,
    )?;

    let mut agents = Vec::new();
    for (_idx, fields) in data {
        let mut agent = Agent::default();

        for field in fields {
            match field {
                ExtractionField::MaterialExtractedBy(val) => agent.full_name = val,
                ExtractionField::MaterialExtractedByOrcid(val) => agent.orcid = Some(val),
                ExtractionField::MaterialExtractedByEntityId(val) => agent.entity_id = val,
                _ => {}
            }
        }

        agents.push(agent);
    }

    Ok(agents)
}


pub fn get_prepared_agents(dataset: &Dataset) -> Result<Vec<Agent>, TransformError> {
    let models = dataset.scope(&["library"]);
    let mut scope = Vec::new();
    for model in models.iter() {
        scope.push(iref::Iri::new(model).unwrap());
    }

    let resolver = Resolver::new(dataset);

    let data: HashMap<Literal, Vec<LibraryField>> =
        resolver.resolve(&[Library::PreparedBy, Library::PreparedByEntityId], &scope)?;

    let mut agents = Vec::new();
    for (_idx, fields) in data {
        let mut agent = Agent::default();

        for field in fields {
            match field {
                LibraryField::PreparedBy(val) => agent.full_name = val,
                LibraryField::PreparedByEntityId(val) => agent.entity_id = val,
                _ => {}
            }
        }

        agents.push(agent);
    }

    Ok(agents)
}
